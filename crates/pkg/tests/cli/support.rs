//! Runs the real `pkg` binary against a fake mise and fake catalogs in a temporary home.
//!
//! The fake mise is a shell script: it logs each call and answers from files the test wrote, so
//! a test states exactly what mise says and then checks what pkg did with it. The catalogs are a
//! local HTTP server that serves fixed documents by path.

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde_json::{Value, json};
use tempfile::TempDir;

/// The only directories on `PATH` when pkg runs: enough for the fake mise's shell script.
pub(crate) const SYSTEM_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// What `mise use -g github:markmals/pkg@latest` leaves behind: mise's shim for pkg, which runs a
/// pkg that reports the version pkg itself was built as.
pub(crate) const INSTALLS_PKG: &str = r#"shims="$HOME/.local/share/mise/shims"
mkdir -p "$shims"
printf '#!/bin/sh\necho "pkg 0.0.0"\n' > "$shims/pkg"
chmod +x "$shims/pkg"
"#;

/// A stand-in for mise's installer at https://mise.run: it puts the fake mise where
/// `MISE_INSTALL_PATH` says.
pub(crate) const FAKE_MISE_INSTALLER: &str = r#"#!/bin/sh
set -e
mkdir -p "$(dirname "$MISE_INSTALL_PATH")"
cp "$FAKE_MISE/../bin/mise" "$MISE_INSTALL_PATH"
chmod +x "$MISE_INSTALL_PATH"
echo "mise: installed to $MISE_INSTALL_PATH" >&2
"#;

const FAKE_MISE: &str = r#"#!/bin/sh
# Answers from $FAKE_MISE/answers/<key>[.<n>], where <key> is the arguments with the home
# directory written as `~` and every other byte but letters and digits replaced by `_`, and <n>
# counts calls with those arguments from 1.
dir=$FAKE_MISE
printf '%s\n' "$*" >> "$dir/calls"
key=$(printf '%s' "$*" | sed "s|$HOME|~|g" | LC_ALL=C tr -c 'A-Za-z0-9' '_')
mkdir -p "$dir/counts"
count=$(( $(cat "$dir/counts/$key" 2>/dev/null || echo 0) + 1 ))
echo "$count" > "$dir/counts/$key"
answer="$dir/answers/$key"
[ -d "$answer.$count" ] && answer="$answer.$count"
if [ ! -d "$answer" ]; then
  echo "fake mise has no answer for: $*" >&2
  exit 97
fi
[ -f "$answer/run" ] && sh "$answer/run"
[ -f "$answer/stdout" ] && cat "$answer/stdout"
[ -f "$answer/stderr" ] && cat "$answer/stderr" >&2
exit "$(cat "$answer/status" 2>/dev/null || echo 0)"
"#;

/// The shared config, which pkg must never edit.
pub(crate) const SHARED_CONFIG: &str = r#"[tools]
jq = "latest"

[bootstrap.brew.taps]
"sesh/tap" = "https://github.com/hipstersmoothie/sesh-app.git"

[bootstrap.packages]
"brew:git" = "latest"
"#;

/// The Mac's config, mise's write target.
pub(crate) const MAC_CONFIG: &str = r#"#:schema https://mise.jdx.dev/schema/mise.json

[tools]
"npm:prettier" = "latest"

[bootstrap.brew.taps]
"acme/tools" = "https://github.com/acme/brew-tools.git"

[bootstrap.packages]
"brew:coreutils" = "latest" # GNU tools
# wget fetches things.
"brew:wget" = "latest"
"brew-cask:firefox" = "latest"
"mas:937984704" = "latest" # Amphetamine
"#;

/// A temporary home with a dotfiles checkout, a fake mise, and fake catalogs.
pub(crate) struct Fixture {
    dir: TempDir,
    catalog: Catalog,
}

/// What one run of `pkg` produced.
#[derive(Debug)]
pub(crate) struct Outcome {
    pub(crate) status: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl Outcome {
    /// The result on stdout, which must be one JSON document.
    pub(crate) fn json(&self) -> Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|error| {
            panic!("stdout is not JSON ({error}):\n{}\nstderr:\n{}", self.stdout, self.stderr)
        })
    }

    /// The `error` object on stderr, after checking the whole document against its schema. The
    /// error is the last line: mise's progress may come before it.
    pub(crate) fn error(&self) -> Value {
        let last = self.stderr.trim().lines().last().unwrap_or_default();
        let document: Value = serde_json::from_str(last).unwrap_or_else(|error| {
            panic!("stderr does not end in one JSON document ({error}):\n{}", self.stderr)
        });
        assert_matches_schema(&error_schema(), &document, "the error document");
        document["error"].clone()
    }

    /// Asserts a failure: the exit status, the code, and nothing on stdout.
    pub(crate) fn assert_failed(&self, status: i32, code: &str) -> Value {
        assert_eq!(self.status, status, "stdout: {}\nstderr: {}", self.stdout, self.stderr);
        assert!(self.stdout.is_empty(), "a failure wrote to stdout: {}", self.stdout);
        let error = self.error();
        assert_eq!(error["code"], code, "{error}");
        error
    }

    /// Asserts success with one JSON document on stdout.
    pub(crate) fn success(&self) -> Value {
        assert_eq!(self.status, 0, "stdout: {}\nstderr: {}", self.stdout, self.stderr);
        self.json()
    }

    /// Asserts success and returns the text on stdout.
    pub(crate) fn text(&self) -> &str {
        assert_eq!(self.status, 0, "stdout: {}\nstderr: {}", self.stdout, self.stderr);
        &self.stdout
    }
}

/// Every file and directory under a root: contents and modification time for files.
type Snapshot = BTreeMap<PathBuf, Option<(Vec<u8>, SystemTime)>>;

impl Fixture {
    /// A home with no mise config, a mise that answers nothing, and empty catalogs.
    pub(crate) fn new() -> Fixture {
        let dir = tempfile::Builder::new().prefix("pkg-test-").tempdir().expect("a temp dir");
        let fixture = Fixture { dir, catalog: Catalog::start() };
        for directory in ["home", "mise/answers", "bin", "cache"] {
            std::fs::create_dir_all(fixture.path(directory)).expect("a fixture directory");
        }
        let script = fixture.path("bin/mise");
        std::fs::write(&script, FAKE_MISE).expect("the fake mise");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .expect("an executable fake mise");
        }
        fixture
    }

    /// A Mac whose `~/.config/mise` links into `~/.dotfiles/mise`, with the packages in
    /// [`MAC_CONFIG`] and [`SHARED_CONFIG`] installed, and catalogs that know them.
    pub(crate) fn seeded() -> Fixture {
        let fixture = Fixture::new();
        let mise = fixture.home().join(".dotfiles/mise");
        std::fs::create_dir_all(&mise).expect("the dotfiles checkout");
        std::fs::write(mise.join("config.toml"), SHARED_CONFIG).expect("the shared config");
        std::fs::write(mise.join("config.macos.toml"), MAC_CONFIG).expect("the Mac config");
        std::fs::create_dir_all(fixture.home().join(".config")).expect("~/.config");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&mise, fixture.home().join(".config/mise")).expect("the link");

        let target = "~/.config/mise/config.macos.toml\n";
        fixture.mise(&["settings", "get", "write_targets.packages"]).stdout(target);
        fixture.mise(&["settings", "get", "write_targets.tools"]).stdout(target);
        fixture.mise(&["backends", "ls"]).stdout("aqua\ncargo\ncore\ngithub\nnpm\n");
        fixture.mise(&["version"]).stdout("2026.11.9 macos-arm64 (2026-11-16)\n");
        fixture.mise(&["version", "--json"]).json(&mise_version("2026.11.9", "2026.11.9"));
        fixture.mise(&["self-update", "--yes"]).stdout("");
        let config = fixture.config_dir();
        fixture.mise(&["config", "ls", "--json"]).json(&json!([
            { "path": config.join("config.macos.toml"), "tools": ["npm:prettier"] },
            { "path": config.join("config.toml"), "tools": ["jq"] },
        ]));
        fixture.mise(&["bootstrap", "packages", "status", "--json"]).json(&status(&[]));
        fixture.mise(&["ls", "--global", "--json"]).json(&json!({
            "jq": [tool("1.8.2", &config.join("config.toml"))],
            "npm:prettier": [tool("3.6.1", &config.join("config.macos.toml"))],
        }));
        fixture.mise(&["registry", "--json"]).json(&json!([
            { "short": "jq", "backends": ["aqua:jqlang/jq"], "description": "Command-line JSON processor", "aliases": [] },
            { "short": "ripgrep", "backends": ["aqua:BurntSushi/ripgrep"], "description": "Recursively search directories for a regex pattern", "aliases": ["rg"] },
            { "short": "ffmpeg", "backends": ["conda:ffmpeg"], "description": "A complete solution to record, convert and stream audio and video", "aliases": [] },
        ]));
        fixture.mise(&["tool", "jq", "--json"]).json(&json!({
            "backend": "aqua:jqlang/jq", "description": "Command-line JSON processor", "url": null,
            "installed_versions": ["1.8.2"], "requested_versions": ["latest"],
        }));
        fixture.mise(&["outdated", "--global", "--json"]).json(&json!({
            "npm:prettier": { "name": "npm:prettier", "requested": "latest", "current": "3.6.1", "latest": "3.6.2" },
        }));
        fixture.mise(&["bootstrap", "packages", "upgrade", "--dry-run"]).stdout(
            "pour libidn2/2.3.8 (dependency)\npour wget/1.25.0_2 (requested)\nmise brew-cask:firefox: skipped: installed app is running and updates itself\nmas upgrade 937984704\n",
        );
        fixture
            .mise(&["bootstrap", "packages", "prune", "--dry-run", "--manager", "brew"])
            .stdout("");
        fixture
            .mise(&["bootstrap", "packages", "prune", "--dry-run", "--manager", "brew-cask"])
            .stdout("mise brew-cask: nothing to prune\n");
        fixture
            .mise(&[
                "bootstrap",
                "packages",
                "prune",
                "--dry-run",
                "--manager",
                "brew-cask",
                "--force",
            ])
            .stdout("mise brew-cask: nothing to prune\n");
        fixture
            .mise(&["bootstrap", "packages", "where", "brew:wget"])
            .stdout("/opt/homebrew/opt/wget\n");
        fixture.mise(&["where", "jq"]).stdout("/mise/installs/jq/1.8.2\n");

        // What the registered examples change.
        fixture
            .mise(&["bootstrap", "packages", "use", "--dry-run", "-g", "brew:hello"])
            .stdout("~/.config/mise/config.macos.toml: \"brew:hello\" = \"latest\"\npour hello/2.12.3 (requested)\n");
        for package in ["brew:hello", "brew-cask:iina"] {
            fixture
                .mise(&["bootstrap", "packages", "use", "--yes", "-g", package])
                .declares(package);
        }
        fixture.mise(&["use", "-g", "ripgrep@latest"]).stdout("mise ripgrep@15.2.0 installed\n");
        fixture.mise(&["use", "-g", "github:markmals/pkg@latest"]).runs(INSTALLS_PKG);
        fixture
            .mise(&["bootstrap", "packages", "prune", "--dry-run", "--manager", "brew"])
            .nth(2)
            .stdout("remove brew:wget@1.24.5\nremove brew:libidn2@2.3.8\n");
        fixture.mise(&["bootstrap", "packages", "prune", "--yes", "--manager", "brew"]).stdout("");
        fixture.mise(&["bootstrap", "packages", "upgrade", "--yes", "brew:wget"]).stdout("");
        let path = fixture.mac_config_link();
        for args in [
            &["--dry-run", "acme/tools", "https://github.com/acme/brew-tools.git"][..],
            &["acme/tools", "https://github.com/acme/brew-tools.git"][..],
        ] {
            let mut tap = vec!["bootstrap", "packages", "brew", "tap", "-p", &path];
            tap.extend_from_slice(args);
            fixture.mise(&tap).stdout("");
        }
        for args in [&["--dry-run", "acme/tools"][..], &["acme/tools"][..]] {
            let mut untap = vec!["bootstrap", "packages", "brew", "untap", "-p", &path];
            untap.extend_from_slice(args);
            fixture.mise(&untap).stdout("");
        }

        fixture.serve("/api/formula.json", &json!([
            { "name": "wget", "desc": "Internet file retriever", "versions": { "stable": "1.25.0" }, "revision": 2 },
            { "name": "hello", "desc": "Program providing model for GNU coding standards", "versions": { "stable": "2.12.3" } },
            { "name": "ffmpeg", "desc": "Play, record, convert, and stream audio and video", "versions": { "stable": "8.0.1" } },
            { "name": "ffmpegthumbnailer", "desc": "Create thumbnails for your video files", "versions": { "stable": "2.2.3" } },
            { "name": "old", "desc": "Disabled", "versions": { "stable": "1" }, "disabled": true },
        ]));
        fixture.serve("/api/cask.json", &json!([
            { "token": "firefox", "name": ["Mozilla Firefox"], "desc": "Web browser", "version": "160.0.1" },
            { "token": "iina", "name": ["IINA"], "desc": "Free and open-source media player", "version": "1.4.1" },
            { "token": "vlc", "name": ["VLC media player"], "desc": "Multimedia player", "version": "3.0.21" },
        ]));
        fixture.serve("/api/formula/wget.json", &json!({
            "name": "wget", "desc": "Internet file retriever", "homepage": "https://www.gnu.org/software/wget/",
            "license": "GPL-3.0-or-later", "versions": { "stable": "1.25.0" }, "revision": 2,
            "dependencies": ["libidn2", "openssl@3"], "caveats": null, "deprecated": false, "disabled": false,
        }));
        fixture.serve("/api/cask/firefox.json", &json!({
            "token": "firefox", "name": ["Mozilla Firefox"], "desc": "Web browser",
            "homepage": "https://www.mozilla.org/firefox/", "version": "160.0.1", "auto_updates": true,
            "artifacts": [{ "uninstall": [] }, { "app": ["Firefox.app"] }], "caveats": null,
        }));
        let amphetamine = json!({
            "trackId": 937984704, "trackName": "Amphetamine", "version": "5.3.2",
            "description": "Keep your Mac awake. Triggers do it for you.",
            "trackViewUrl": "https://apps.apple.com/app/id937984704",
        });
        fixture.serve("/lookup", &json!({ "resultCount": 1, "results": [amphetamine] }));
        fixture.serve("/search", &json!({ "resultCount": 2, "results": [
            amphetamine,
            { "trackId": 6444012595u64, "trackName": "FFmpeg Studio", "version": "2.2", "description": "Run ffmpeg without a terminal." },
        ]}));
        fixture
    }

    pub(crate) fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }

    pub(crate) fn home(&self) -> PathBuf {
        self.path("home")
    }

    /// `~/.config/mise`, as mise names it.
    pub(crate) fn config_dir(&self) -> PathBuf {
        self.home().join(".config/mise")
    }

    /// The Mac config as mise names it, through the link.
    pub(crate) fn mac_config_link(&self) -> String {
        self.config_dir().join("config.macos.toml").display().to_string()
    }

    /// A config file in the dotfiles checkout: `config.macos.toml` or `config.toml`.
    pub(crate) fn dotfile(&self, name: &str) -> PathBuf {
        self.home().join(".dotfiles/mise").join(name)
    }

    pub(crate) fn read_dotfile(&self, name: &str) -> String {
        std::fs::read_to_string(self.dotfile(name)).expect("a config file")
    }

    /// The answer the fake mise gives to `mise -C <home> <args>`.
    pub(crate) fn mise(&self, args: &[&str]) -> Answer {
        let home = self.home().display().to_string();
        let mut words = vec!["-C", home.as_str()];
        words.extend_from_slice(args);
        let key: String = words
            .join(" ")
            .replace(&home, "~")
            .bytes()
            .map(|byte| if byte.is_ascii_alphanumeric() { char::from(byte) } else { '_' })
            .collect();
        let answer =
            Answer { directory: self.path("mise/answers").join(key), fixture_home: self.home() };
        std::fs::create_dir_all(&answer.directory).expect("an answer directory");
        answer
    }

    /// The mise commands pkg ran, without the `-C <home>` every one starts with.
    pub(crate) fn calls(&self) -> Vec<String> {
        let prefix = format!("-C {} ", self.home().display());
        std::fs::read_to_string(self.path("mise/calls"))
            .unwrap_or_default()
            .lines()
            .map(|line| line.strip_prefix(&prefix).unwrap_or(line).to_string())
            .collect()
    }

    /// Serves `body` as JSON at `path` on the fake catalogs.
    pub(crate) fn serve(&self, path: &str, body: &Value) {
        self.catalog
            .routes
            .lock()
            .expect("the routes")
            .insert(path.to_string(), (200, body.to_string()));
    }

    /// Serves `body` as plain text at `path` on the fake catalogs.
    pub(crate) fn serve_text(&self, path: &str, body: &str) {
        self.catalog
            .routes
            .lock()
            .expect("the routes")
            .insert(path.to_string(), (200, body.to_string()));
    }

    /// The fake catalogs' base URL.
    pub(crate) fn catalog_url(&self) -> &str {
        &self.catalog.url
    }

    /// Answers `path` with an error status.
    pub(crate) fn fail(&self, path: &str, status: u16) {
        self.catalog
            .routes
            .lock()
            .expect("the routes")
            .insert(path.to_string(), (status, "{}".into()));
    }

    pub(crate) fn run(&self, args: &[&str]) -> Outcome {
        self.run_with(args, &[])
    }

    /// `run` with `env` applied on top of the fixture's environment; `None` unsets a variable.
    /// `PATH` holds only the system directories, so no mise or pkg of the host's is found.
    pub(crate) fn run_with(&self, args: &[&str], env: &[(&str, Option<&OsStr>)]) -> Outcome {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pkg"));
        command
            .args(args)
            .current_dir(self.dir.path())
            .env_clear()
            .env("PATH", SYSTEM_PATH)
            .env("HOME", self.home())
            .env("PKG_MISE", self.path("bin/mise"))
            .env("FAKE_MISE", self.path("mise"))
            .env("PKG_HOMEBREW_API", format!("{}/api", self.catalog.url))
            .env("PKG_APP_STORE_API", &self.catalog.url)
            .env("XDG_CACHE_HOME", self.path("cache"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in env {
            match value {
                Some(value) => command.env(key, value),
                None => command.env_remove(key),
            };
        }
        let output = command.output().expect("pkg runs");
        Outcome {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// Marks the fake mise as one a package manager installed, which mise's packaging contract
    /// signals with a file under its install prefix (the directory above `bin/`).
    pub(crate) fn disable_mise_self_update(&self) {
        std::fs::create_dir_all(self.path("lib")).expect("the prefix's lib directory");
        std::fs::write(self.path("lib/.disable-self-update"), "").expect("the marker");
    }

    /// `run` with `--output json`.
    pub(crate) fn json(&self, args: &[&str]) -> Outcome {
        self.run(&[args, &["--output", "json"]].concat())
    }

    /// Every file and directory in the home directory, to prove a command changed nothing.
    pub(crate) fn snapshot(&self) -> Snapshot {
        fn walk(directory: &Path, entries: &mut Snapshot) {
            for entry in std::fs::read_dir(directory).expect("a readable directory") {
                let path = entry.expect("an entry").path();
                let metadata = std::fs::symlink_metadata(&path).expect("metadata");
                if metadata.is_dir() {
                    entries.insert(path.clone(), None);
                    walk(&path, entries);
                } else if metadata.is_file() {
                    let contents = std::fs::read(&path).expect("a readable file");
                    entries.insert(path, Some((contents, metadata.modified().expect("a time"))));
                } else {
                    entries.insert(path, None);
                }
            }
        }
        let mut entries = BTreeMap::new();
        walk(&self.home(), &mut entries);
        entries
    }
}

/// One fake mise answer, written as it is configured.
pub(crate) struct Answer {
    directory: PathBuf,
    fixture_home: PathBuf,
}

impl Answer {
    /// The answer for the `n`th call with the same arguments, counting from 1; calls without a
    /// numbered answer get the plain one.
    pub(crate) fn nth(self, n: u32) -> Answer {
        let name =
            format!("{}.{n}", self.directory.file_name().unwrap_or_default().to_string_lossy());
        let directory = self.directory.with_file_name(name);
        std::fs::create_dir_all(&directory).expect("an answer directory");
        Answer { directory, fixture_home: self.fixture_home }
    }

    pub(crate) fn stdout(self, text: &str) -> Answer {
        std::fs::write(self.directory.join("stdout"), text).expect("stdout");
        self
    }

    pub(crate) fn json(self, value: &Value) -> Answer {
        self.stdout(&value.to_string())
    }

    pub(crate) fn stderr(self, text: &str) -> Answer {
        std::fs::write(self.directory.join("stderr"), text).expect("stderr");
        self
    }

    pub(crate) fn status(self, status: i32) -> Answer {
        std::fs::write(self.directory.join("status"), status.to_string()).expect("status");
        self
    }

    /// Runs `script` with `sh` before answering, in mise's environment.
    pub(crate) fn runs(self, script: &str) -> Answer {
        std::fs::write(self.directory.join("run"), script).expect("run");
        self
    }

    /// Appends `"<package>" = "latest"` to the Mac config, as `mise bootstrap packages use`
    /// does; the config ends in `[bootstrap.packages]`.
    pub(crate) fn declares(self, package: &str) -> Answer {
        let config = self.fixture_home.join(".dotfiles/mise/config.macos.toml");
        let script =
            format!("printf '%s\\n' '\"{package}\" = \"latest\"' >> '{}'\n", config.display());
        std::fs::write(self.directory.join("run"), script).expect("run");
        self
    }
}

/// `mise bootstrap packages status --json` for the seeded packages plus `extra`.
pub(crate) fn status(extra: &[(&str, &str, Option<&str>)]) -> Value {
    let mut managers: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
    let seeded = [
        ("brew", "git", Some("2.56.1")),
        ("brew", "coreutils", Some("9.12_1")),
        ("brew", "wget", Some("1.24.5")),
        ("brew-cask", "firefox", Some("160.0")),
        ("mas", "937984704", Some("5.3.1")),
    ];
    for (manager, package, installed) in seeded.iter().chain(extra) {
        let mut entry = json!({
            "package": package,
            "requested_version": "latest",
            "desired_state": "present",
            "state": if installed.is_some() { "installed" } else { "missing" },
            "installed_version": installed,
        });
        if *manager == "mas" {
            entry["name"] = json!("Amphetamine");
        }
        managers.entry(manager).or_default().push(entry);
    }
    let managers: serde_json::Map<String, Value> = managers
        .into_iter()
        .map(|(manager, packages)| {
            (manager.to_string(), json!({ "available": true, "packages": packages }))
        })
        .collect();
    Value::Object(managers)
}

/// `mise version --json` for a mise at `current` that knows of `latest`.
pub(crate) fn mise_version(current: &str, latest: &str) -> Value {
    json!({
        "version": format!("{current} macos-arm64 (2026-11-16)"),
        "latest": latest,
        "os": "macos",
        "arch": "arm64",
        "build_time": "2026-11-16 10:40:41 +00:00",
    })
}

fn tool(version: &str, config: &Path) -> Value {
    json!({
        "version": version, "requested_version": "latest", "installed": true, "active": true,
        "source": { "type": "mise.toml", "path": config },
    })
}

/// A local HTTP server that answers each path with a fixed status and JSON body.
struct Catalog {
    url: String,
    routes: Arc<Mutex<HashMap<String, (u16, String)>>>,
}

impl Catalog {
    fn start() -> Catalog {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let url = format!("http://{}", listener.local_addr().expect("an address"));
        let routes: Arc<Mutex<HashMap<String, (u16, String)>>> = Arc::default();
        let served = Arc::clone(&routes);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut reader = BufReader::new(stream.try_clone().expect("a stream"));
                let mut request = String::new();
                if reader.read_line(&mut request).is_err() {
                    continue;
                }
                let mut header = String::new();
                while reader.read_line(&mut header).is_ok_and(|read| read > 2) {
                    header.clear();
                }
                let target = request.split_whitespace().nth(1).unwrap_or("/");
                let path = target.split('?').next().unwrap_or(target);
                let (status, body) = served
                    .lock()
                    .expect("the routes")
                    .get(path)
                    .cloned()
                    .unwrap_or((404, "{}".into()));
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Catalog { url, routes }
    }
}

/// Fails the test when `result` does not match the output schema `pkg schema` declares for the
/// command at `path`.
pub(crate) fn assert_matches_output_schema(path: &[&str], result: &Value) {
    let mut args = vec!["schema"];
    args.extend_from_slice(path);
    let description = Fixture::new().json(&args).success();
    assert_matches_schema(&description["output"], result, &format!("pkg {}", path.join(" ")));
}

fn error_schema() -> Value {
    schemars::schema_for!(pkg::failure::ErrorDocument).to_value()
}

fn assert_matches_schema(schema: &Value, instance: &Value, what: &str) {
    let validator = jsonschema::validator_for(schema)
        .unwrap_or_else(|error| panic!("{what} has an invalid schema: {error}"));
    let problems: Vec<String> = validator
        .iter_errors(instance)
        .map(|error| format!("{error} at {}", error.instance_path()))
        .collect();
    assert!(
        problems.is_empty(),
        "{what} does not match its schema:\n{}\n{}",
        problems.join("\n"),
        serde_json::to_string_pretty(instance).unwrap_or_default()
    );
}
