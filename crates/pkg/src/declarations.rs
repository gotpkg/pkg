//! What the global mise config files declare, and taking one package declaration back out.
//!
//! mise has `use` to add a `[bootstrap.packages]` entry but nothing to remove one, so pkg edits
//! the file itself: it deletes only the entry's own lines, keeps every comment, and then checks
//! that the result parses to exactly the original config minus that entry.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use toml_edit::{DocumentMut, Item, Table};

use crate::environment::real_path;
use crate::failure::Failure;

/// The declarations of every global config file mise loads.
pub struct Declarations {
    files: Vec<DeclaredFile>,
}

struct DeclaredFile {
    path: PathBuf,
    packages: Vec<String>,
    tools: Vec<String>,
    taps: Vec<(String, String)>,
}

/// A `[bootstrap.brew.taps]` entry.
pub struct Tap {
    pub name: String,
    pub url: String,
    pub config: PathBuf,
}

impl Declarations {
    /// Reads every file in `paths`; a file that does not exist declares nothing.
    ///
    /// # Errors
    ///
    /// `PRECONDITION_FAILED` when a file cannot be read or is not valid TOML.
    pub fn load(paths: &[PathBuf]) -> Result<Declarations, Failure> {
        let mut files = Vec::new();
        for path in paths {
            let text = match std::fs::read_to_string(path) {
                Ok(text) => text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(unreadable(path, &error.to_string())),
            };
            let document: DocumentMut = text
                .parse()
                .map_err(|error: toml_edit::TomlError| unreadable(path, &error.to_string()))?;
            let keys = |item: Option<&Item>| -> Vec<String> {
                item.and_then(Item::as_table_like)
                    .map(|table| table.iter().map(|(key, _)| key.to_string()).collect())
                    .unwrap_or_default()
            };
            let bootstrap = document.get("bootstrap");
            let taps = bootstrap
                .and_then(|bootstrap| bootstrap.get("brew"))
                .and_then(|brew| brew.get("taps"))
                .and_then(Item::as_table_like)
                .map(|taps| {
                    taps.iter()
                        .map(|(name, url)| {
                            (name.to_string(), url.as_str().unwrap_or_default().to_string())
                        })
                        .collect()
                })
                .unwrap_or_default();
            files.push(DeclaredFile {
                path: path.clone(),
                packages: keys(bootstrap.and_then(|bootstrap| bootstrap.get("packages"))),
                tools: keys(document.get("tools")),
                taps,
            });
        }
        Ok(Declarations { files })
    }

    /// The first file that declares the bootstrap package `key`.
    pub fn package_file(&self, key: &str) -> Option<&Path> {
        self.files
            .iter()
            .find(|file| file.packages.iter().any(|package| package == key))
            .map(|file| file.path.as_path())
    }

    /// The first file that declares the tool `key`.
    pub fn tool_file(&self, key: &str) -> Option<&Path> {
        self.files
            .iter()
            .find(|file| file.tools.iter().any(|tool| tool == key))
            .map(|file| file.path.as_path())
    }

    pub fn taps(&self) -> Vec<Tap> {
        self.files
            .iter()
            .flat_map(|file| {
                file.taps.iter().map(|(name, url)| Tap {
                    name: name.clone(),
                    url: url.clone(),
                    config: file.path.clone(),
                })
            })
            .collect()
    }
}

/// Whether two paths name the same file once symlinks are resolved.
pub fn same_file(left: &Path, right: &Path) -> bool {
    real_path(left) == real_path(right)
}

fn unreadable(path: &Path, problem: &str) -> Failure {
    Failure::precondition(format!("cannot read {}: {problem}", path.display()))
        .with_details(json!({ "config": path.display().to_string() }))
}

/// Whether the file at `path` declares the `[bootstrap.packages]` entry `key`; `false` when it
/// cannot be read.
pub fn declares_package(path: &Path, key: &str) -> bool {
    Declarations::load(&[path.to_path_buf()])
        .is_ok_and(|declarations| declarations.package_file(key).is_some())
}

/// Removes the `[bootstrap.packages]` entry `key` from the file at `path`.
///
/// # Errors
///
/// `PRECONDITION_FAILED` when the file cannot be read or written, or the entry is not in a form
/// pkg can remove without touching anything else.
pub fn remove_package(path: &Path, key: &str) -> Result<(), Failure> {
    let text =
        std::fs::read_to_string(path).map_err(|error| unreadable(path, &error.to_string()))?;
    let edited = without_package(&text, key).map_err(|problem| {
        Failure::precondition(format!("cannot take {key} out of {}: {problem}", path.display()))
            .with_details(json!({ "package": key, "config": path.display().to_string() }))
            .suggest(format!("Delete the `\"{key}\"` entry from {} by hand", path.display()))
    })?;
    std::fs::write(path, edited).map_err(|error| {
        Failure::precondition(format!("cannot write {}: {error}", path.display()))
            .with_details(json!({ "config": path.display().to_string() }))
    })
}

/// `text` without the `[bootstrap.packages]` entry `key`: its line in `[bootstrap.packages]`, or
/// its own `[bootstrap.packages."<key>"]` table. Comments above and below stay.
///
/// # Errors
///
/// A sentence saying why, when the entry is missing, written in another form, or the edit would
/// change anything besides the entry.
pub fn without_package(text: &str, key: &str) -> Result<String, String> {
    let original: DocumentMut = text
        .parse()
        .map_err(|error: toml_edit::TomlError| format!("it is not valid TOML: {error}"))?;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let range = entry_lines(&lines, key).ok_or_else(|| {
        "its entry is not in [bootstrap.packages] in a form pkg can edit".to_string()
    })?;
    let mut kept: Vec<&str> = lines[..range.start].to_vec();
    let rest = &lines[range.end..];
    // Removing an entry between two blank lines would leave a double blank line.
    let seam_doubles = range.start > 0
        && lines[range.start - 1].trim().is_empty()
        && rest.first().is_some_and(|line| line.trim().is_empty());
    kept.extend_from_slice(if seam_doubles { &rest[1..] } else { rest });
    let edited = kept.concat();

    let mut expected = original;
    let removed = expected
        .get_mut("bootstrap")
        .and_then(|bootstrap| bootstrap.get_mut("packages"))
        .and_then(Item::as_table_like_mut)
        .and_then(|packages| packages.remove(key));
    let parsed: DocumentMut = edited
        .parse()
        .map_err(|error: toml_edit::TomlError| format!("the edit would break the file: {error}"))?;
    if removed.is_none()
        || canonical_table(expected.as_table()) != canonical_table(parsed.as_table())
    {
        return Err("the edit would change more than its entry".to_string());
    }
    Ok(edited)
}

/// The lines of `key`'s entry: one assignment in `[bootstrap.packages]`, or a whole
/// `[bootstrap.packages."<key>"]` table without the blank lines and comments that end it.
fn entry_lines(lines: &[&str], key: &str) -> Option<std::ops::Range<usize>> {
    let mut section: Vec<String> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if let Some(path) = header_path(line) {
            if path == ["bootstrap", "packages", key] {
                let next = lines[index + 1..]
                    .iter()
                    .position(|line| header_path(line).is_some())
                    .map_or(lines.len(), |offset| index + 1 + offset);
                let mut end = next;
                while end > index + 1 && is_blank_or_comment(lines[end - 1]) {
                    end -= 1;
                }
                return Some(index..end);
            }
            section = path;
        } else if section == ["bootstrap", "packages"] && assigns(line, key) {
            return Some(index..index + 1);
        }
    }
    None
}

fn is_blank_or_comment(line: &str) -> bool {
    let line = line.trim();
    line.is_empty() || line.starts_with('#')
}

/// The dotted path of a `[table]` header line, such as `["bootstrap", "packages"]`.
fn header_path(line: &str) -> Option<Vec<String>> {
    let line = line.trim();
    if !line.starts_with('[') || line.starts_with("[[") {
        return None;
    }
    let document: DocumentMut = line.parse().ok()?;
    let mut path = Vec::new();
    let mut table: &Table = document.as_table();
    while table.len() == 1 {
        let (key, item) = table.iter().next()?;
        path.push(key.to_string());
        match item.as_table() {
            Some(inner) if inner.is_implicit() => table = inner,
            _ => break,
        }
    }
    (!path.is_empty()).then_some(path)
}

/// Whether `line` is the single assignment `key = ...`.
fn assigns(line: &str, key: &str) -> bool {
    line.trim()
        .parse::<DocumentMut>()
        .is_ok_and(|document| document.len() == 1 && document.get(key).is_some_and(Item::is_value))
}

fn canonical_table(table: &Table) -> Value {
    Value::Object(table.iter().map(|(key, item)| (key.to_string(), canonical_item(item))).collect())
}

fn canonical_item(item: &Item) -> Value {
    match item {
        Item::None => Value::Null,
        Item::Value(value) => canonical_value(value),
        Item::Table(table) => canonical_table(table),
        Item::ArrayOfTables(tables) => Value::Array(tables.iter().map(canonical_table).collect()),
    }
}

fn canonical_value(value: &toml_edit::Value) -> Value {
    use toml_edit::Value as Toml;
    match value {
        Toml::String(text) => json!(text.value()),
        Toml::Integer(number) => json!(number.value()),
        Toml::Float(number) => json!(number.value().to_string()),
        Toml::Boolean(flag) => json!(flag.value()),
        Toml::Datetime(datetime) => json!(datetime.value().to_string()),
        Toml::Array(array) => Value::Array(array.iter().map(canonical_value).collect()),
        Toml::InlineTable(table) => Value::Object(
            table
                .iter()
                .map(|(key, value)| (key.to_string(), canonical_value(value)))
                .collect::<Map<String, Value>>(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"#:schema https://mise.jdx.dev/schema/mise.json

[tools]
jq = "latest"

[bootstrap.packages]
"brew:coreutils" = "latest" # g-prefixed GNU tools
"brew:wget" = "latest"
# mpv needs Homebrew's yt-dlp.
"brew:yt-dlp" = "latest"

"mas:1365531024" = "latest" # 1Blocker

# Leaf has no cask.
[bootstrap.packages."macos-app:leaf"]
version = "0.7.0"
artifact = "Leaf.app"

# Touch ID for sudo.
[bootstrap.files."/etc/pam.d/sudo_local"]
mode = "0644"
"#;

    #[test]
    fn removes_only_the_entry_line_and_keeps_comments() {
        let edited = without_package(CONFIG, "brew:yt-dlp").unwrap();
        assert_eq!(edited, CONFIG.replace("\"brew:yt-dlp\" = \"latest\"\n", ""));
        let edited = without_package(CONFIG, "brew:coreutils").unwrap();
        assert!(!edited.contains("coreutils") && edited.contains("\"brew:wget\""));
    }

    #[test]
    fn removes_a_package_table_up_to_the_comments_of_the_next_table() {
        let edited = without_package(CONFIG, "macos-app:leaf").unwrap();
        assert!(!edited.contains("[bootstrap.packages.\"macos-app:leaf\"]"));
        assert!(!edited.contains("Leaf.app"));
        assert!(edited.contains("# Touch ID for sudo.\n[bootstrap.files"));
        assert!(edited.contains("# Leaf has no cask.\n"));
    }

    #[test]
    fn a_lone_entry_between_blank_lines_leaves_one_blank_line() {
        let edited = without_package(CONFIG, "mas:1365531024").unwrap();
        assert!(edited.contains("\"brew:yt-dlp\" = \"latest\"\n\n# Leaf has no cask."), "{edited}");
    }

    #[test]
    fn refuses_what_it_cannot_find_or_cannot_remove_cleanly() {
        assert!(without_package(CONFIG, "brew:nothing").is_err());
        // A key of the same name in another table is not a package.
        assert!(without_package(CONFIG, "jq").is_err());
        let dotted =
            "[bootstrap.packages]\n\"brew:x\".version = \"1\"\n\"brew:x\".state = \"present\"\n";
        assert!(without_package(dotted, "brew:x").is_err());
    }

    #[test]
    fn reads_packages_tools_and_taps_from_each_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        std::fs::write(
            &path,
            format!(
                "{CONFIG}\n[bootstrap.brew.taps]\n\"sesh/tap\" = \"https://github.com/a/b.git\"\n"
            ),
        )
        .unwrap();
        let missing = directory.path().join("missing.toml");
        let declarations = Declarations::load(&[missing, path.clone()]).unwrap();
        assert_eq!(declarations.package_file("brew:wget"), Some(path.as_path()));
        assert_eq!(declarations.package_file("macos-app:leaf"), Some(path.as_path()));
        assert_eq!(declarations.tool_file("jq"), Some(path.as_path()));
        assert_eq!(declarations.package_file("jq"), None);
        let taps = declarations.taps();
        assert_eq!(
            (taps[0].name.as_str(), taps[0].url.as_str()),
            ("sesh/tap", "https://github.com/a/b.git")
        );
    }
}
