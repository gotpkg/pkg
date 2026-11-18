//! What a bootstrap preview must leave out, and what a bootstrap leaves for the person to do.

use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item};

/// Whether any of `files` declares a `[bootstrap.files]` entry with `template = true`: mise
/// renders those even in a dry run, and a template's `exec()` would run.
pub(crate) fn declares_templated_files(files: &[PathBuf]) -> bool {
    files.iter().any(|file| {
        let Ok(text) = std::fs::read_to_string(file) else { return false };
        let Ok(document) = text.parse::<DocumentMut>() else { return false };
        let Some(entries) = document
            .get("bootstrap")
            .and_then(|bootstrap| bootstrap.get("files"))
            .and_then(Item::as_table_like)
        else {
            return false;
        };
        entries
            .iter()
            .any(|(_, entry)| entry.get("template").and_then(Item::as_bool).unwrap_or(false))
    })
}

/// The global config files in a checkout's config directory, as mise would load them once it
/// links that directory: `config.toml`, `config.<env>.toml`, and `conf.d/*.toml`.
pub(crate) fn config_directory_files(directory: &Path) -> Vec<PathBuf> {
    let toml_in = |directory: &Path, keep: &dyn Fn(&str) -> bool| -> Vec<PathBuf> {
        std::fs::read_dir(directory)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.file_name().and_then(|name| name.to_str()).is_some_and(keep)
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut files =
        toml_in(directory, &|name| name.starts_with("config") && name.ends_with(".toml"));
    files.extend(toml_in(&directory.join("conf.d"), &|name| name.ends_with(".toml")));
    files.sort();
    files
}

/// The items after the `bootstrap: follow-up` heading mise prints at the end of a run.
pub(crate) fn follow_up(stdout: &str) -> Vec<String> {
    let mut lines = stdout.lines().skip_while(|line| !line.starts_with("bootstrap: follow-up"));
    lines.next();
    lines.map_while(|line| line.strip_prefix("  - ")).map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_templated_bootstrap_file_holds_back_the_preview() {
        let directory = tempfile::TempDir::new().unwrap();
        let plain = directory.path().join("config.toml");
        std::fs::write(&plain, "[bootstrap.files.\"/etc/motd\"]\ncontent = \"hi\"\n").unwrap();
        assert!(!declares_templated_files(std::slice::from_ref(&plain)));
        let templated = directory.path().join("config.work.toml");
        std::fs::write(
            &templated,
            "[bootstrap.files]\n\"/etc/motd\" = { content = \"{{ exec(command='id') }}\", template = true }\n",
        )
        .unwrap();
        assert!(declares_templated_files(&[plain, templated]));
    }

    #[test]
    fn the_follow_up_is_the_list_after_its_heading() {
        let stdout = "tools: done\nbootstrap: follow-up\n  - sign in to the App Store\n  - start a new login session\n";
        assert_eq!(follow_up(stdout), ["sign in to the App Store", "start a new login session"]);
        assert!(follow_up("tools: done\n").is_empty());
    }
}
