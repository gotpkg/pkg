//! `pkg bootstrap`: restoring a configuration with `mise bootstrap`.

use std::ffi::OsStr;
use std::path::PathBuf;

use serde_json::json;

use crate::support::{Fixture, INSTALLS_PKG, Outcome, assert_matches_output_schema, mise_version};

const DOTFILES: &str = "https://github.com/example/dotfiles.git";
const ZSH_BLOCK: &str = "# >>> pkg >>>\nexport PATH=\"$HOME/.local/bin:$PATH\"\n# <<< pkg <<<\n";
const TEMPLATED_FILE: &str = "[bootstrap.files.\"/etc/motd\"]\ncontent = \"{{ exec(command='hostname') }}\"\ntemplate = true\n";

/// A new Mac with mise, the installer's pkg and PATH block, and nothing else.
fn new_mac() -> Fixture {
    let fixture = Fixture::new();
    fixture.mise(&["version", "--json"]).json(&mise_version("2026.11.9", "2026.11.9"));
    fixture.mise(&["backends", "ls"]).stdout("aqua\ngithub\nnpm\n");
    let config = fixture.home().join(".config/mise");
    fixture
        .mise(&["config", "ls", "--json"])
        .json(&json!([{ "path": config.join("config.toml") }]));
    for setting in ["write_targets.packages", "write_targets.tools"] {
        fixture
            .mise(&["settings", "get", setting])
            .stderr(&format!("mise ERROR Setting [{setting}] is not set\n"))
            .status(1);
    }
    fixture.mise(&["ls", "--global", "--json"]).json(&json!({}));
    fixture.mise(&["use", "-g", "github:gotpkg/pkg@latest"]).runs(INSTALLS_PKG);
    std::fs::write(fixture.home().join(".zshenv"), ZSH_BLOCK).unwrap();
    let pkg = fixture.home().join(".local/bin/pkg");
    std::fs::create_dir_all(pkg.parent().unwrap()).unwrap();
    std::fs::write(&pkg, "#!/bin/sh\necho 'pkg 0.0.0'\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&pkg, std::fs::Permissions::from_mode(0o755)).unwrap();
    fixture
}

fn dotfiles(fixture: &Fixture) -> PathBuf {
    fixture.home().join(".dotfiles")
}

/// The arguments of `mise bootstrap --from` for the example dotfiles.
fn from_args(fixture: &Fixture) -> Vec<String> {
    let directory = dotfiles(fixture).display().to_string();
    ["bootstrap", "--from", DOTFILES, "--from-dir", &directory, "--config-dir", "mise"]
        .map(str::to_string)
        .to_vec()
}

fn words(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}

/// What mise does for `--from … --config-dir mise`: clone, link `~/.config/mise`, deploy a
/// `~/.zshenv` of its own as a link, and list follow-up work.
fn enrolls(fixture: &Fixture) {
    let script = r#"set -e
[ ! -e "$HOME/.zshenv" ] || { echo "pkg's .zshenv is still there" >&2; exit 1; }
mkdir -p "$HOME/.dotfiles/mise" "$HOME/.dotfiles/home" "$HOME/.config"
printf '[tools]\n' > "$HOME/.dotfiles/mise/config.toml"
printf 'export EDITOR=vi\n' > "$HOME/.dotfiles/home/.zshenv"
ln -s "$HOME/.dotfiles/mise" "$HOME/.config/mise"
ln -s "$HOME/.dotfiles/home/.zshenv" "$HOME/.zshenv"
"#;
    fixture
        .mise(&words(&from_args(fixture)))
        .runs(script)
        .stdout("bootstrap: follow-up\n  - sign in to the App Store\n");
}

fn bootstrap(fixture: &Fixture, args: &[&str]) -> Outcome {
    let mut command = vec!["bootstrap"];
    command.extend_from_slice(args);
    command.extend(["--output", "json"]);
    fixture.run_with(&command, &[("SHELL", Some(OsStr::new("/bin/zsh")))])
}

fn from_dotfiles(fixture: &Fixture, extra: &[&str]) -> Outcome {
    let directory = dotfiles(fixture).display().to_string();
    let mut args = vec!["--from", DOTFILES, "--directory", &directory, "--config-dir", "mise"];
    args.extend_from_slice(extra);
    bootstrap(fixture, &args)
}

#[test]
fn bootstrap_enrolls_a_dotfiles_checkout_and_hands_pkg_to_mise() {
    let fixture = new_mac();
    enrolls(&fixture);
    let receipt = from_dotfiles(&fixture, &[]).success();
    assert_matches_output_schema(&["bootstrap"], &receipt);
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["mise"]["action"], "found");
    assert_eq!(receipt["source"]["kind"], "repository");
    assert_eq!(receipt["source"]["url"], DOTFILES);
    assert_eq!(receipt["followUp"], json!(["sign in to the App Store"]));
    assert_eq!(receipt["handoff"]["link"], "linked");

    let calls = fixture.calls();
    assert!(calls.contains(&from_args(&fixture).join(" ")), "{calls:?}");
    assert!(!calls.iter().any(|call| call.contains("--yes")), "never answers for you: {calls:?}");
    assert!(std::fs::symlink_metadata(fixture.home().join(".zshenv")).unwrap().is_symlink());
}

#[test]
fn bootstrap_puts_back_the_installers_block_where_the_config_deploys_nothing() {
    let fixture = new_mac();
    enrolls(&fixture);
    std::fs::write(fixture.home().join(".bashrc"), ZSH_BLOCK).unwrap();
    let receipt = from_dotfiles(&fixture, &[]).success();
    let home = |name: &str| fixture.home().join(name).display().to_string();
    assert_eq!(receipt["startupFiles"]["movedAside"], json!([home(".zshenv"), home(".bashrc")]));
    assert_eq!(receipt["startupFiles"]["restored"], json!([home(".bashrc")]));
    assert_eq!(std::fs::read_to_string(fixture.home().join(".bashrc")).unwrap(), ZSH_BLOCK);
}

#[test]
fn bootstrap_refuses_an_occupied_config_directory_before_changing_anything() {
    let fixture = new_mac();
    let config = fixture.home().join(".config/mise");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("config.toml"), "[tools]\n").unwrap();
    let before = fixture.snapshot();
    let error = from_dotfiles(&fixture, &[]).assert_failed(3, "PRECONDITION_FAILED");
    assert!(error["suggestions"].to_string().contains("aside"), "{error}");
    assert_eq!(fixture.snapshot(), before);
    assert!(!fixture.calls().iter().any(|call| call.starts_with("bootstrap")));
}

#[test]
fn a_failed_bootstrap_puts_the_block_back_and_says_changes_may_remain() {
    let fixture = new_mac();
    fixture
        .mise(&words(&from_args(&fixture)))
        .stderr("mise ERROR sudo: a terminal is required to read the password\n")
        .status(1);
    let error = from_dotfiles(&fixture, &[]).assert_failed(6, "MISE_FAILED");
    assert_eq!(error["details"]["phase"], "bootstrap");
    assert_eq!(error["details"]["changesMayRemain"], true);
    assert!(error["suggestions"].to_string().contains("terminal"), "{error}");
    assert_eq!(std::fs::read_to_string(fixture.home().join(".zshenv")).unwrap(), ZSH_BLOCK);
}

#[test]
fn a_bootstrap_dry_run_skips_templated_files_and_changes_nothing() {
    let fixture = new_mac();
    let config = fixture.home().join(".config/mise/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, TEMPLATED_FILE).unwrap();
    fixture.mise(&["bootstrap", "--dry-run", "--skip", "files"]).stdout("tools: nothing to do\n");
    let before = fixture.snapshot();
    let receipt = bootstrap(&fixture, &["--dry-run"]).success();
    assert_eq!(receipt["status"], "dry-run");
    assert!(receipt["notPreviewed"].to_string().contains("files"), "{receipt}");
    assert!(fixture.calls().contains(&"bootstrap --dry-run --skip files".to_string()));
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn a_dry_run_for_an_environment_checks_that_environments_files() {
    let fixture = new_mac();
    let work = fixture.home().join(".config/mise/config.work.toml");
    std::fs::create_dir_all(work.parent().unwrap()).unwrap();
    std::fs::write(&work, TEMPLATED_FILE).unwrap();
    let listed = format!(
        "case \"$MISE_ENV\" in work) printf '[{{\"path\":\"{}\"}}]';; *) printf '[]';; esac\n",
        work.display()
    );
    fixture.mise(&["config", "ls", "--json"]).runs(&listed).stdout("");
    fixture.mise(&["-E", "work", "bootstrap", "--dry-run", "--skip", "files"]).stdout("");
    let receipt = bootstrap(&fixture, &["--env", "work", "--dry-run"]).success();
    assert!(receipt["notPreviewed"].to_string().contains("files"), "{receipt}");
    let preview = "-E work bootstrap --dry-run --skip files".to_string();
    assert!(fixture.calls().contains(&preview), "{:?}", fixture.calls());
}

#[test]
fn a_bootstrap_without_a_terminal_gives_mise_no_terminal_to_ask_at() {
    let fixture = new_mac();
    let probe = r#"[ "$(ps -o pgid= -p "$PPID" | tr -d ' ')" = "$PPID" ] && echo own-session >> "$FAKE_MISE/terminal"
if (exec 3</dev/tty) 2>/dev/null; then echo has-a-terminal >> "$FAKE_MISE/terminal"; fi
"#;
    fixture.mise(&["bootstrap"]).runs(probe);
    bootstrap(&fixture, &[]).success();
    let terminal = std::fs::read_to_string(fixture.path("mise/terminal")).unwrap_or_default();
    assert_eq!(terminal, "own-session\n");
}

#[test]
fn bootstrap_accepts_a_config_directory_that_already_links_to_the_checkout() {
    let fixture = new_mac();
    std::fs::create_dir_all(dotfiles(&fixture).join("mise")).unwrap();
    std::fs::create_dir_all(fixture.home().join(".config")).unwrap();
    std::os::unix::fs::symlink("../.dotfiles/mise", fixture.home().join(".config/mise")).unwrap();
    fixture.mise(&words(&from_args(&fixture))).stdout("");
    from_dotfiles(&fixture, &[]).success();
}

#[test]
fn a_dry_run_without_mise_says_what_pkg_would_install() {
    let fixture = new_mac();
    let receipt = fixture
        .run_with(
            &["bootstrap", "--dry-run", "--output", "json"],
            &[("PKG_MISE", None), ("SHELL", Some(OsStr::new("/bin/zsh")))],
        )
        .success();
    assert_eq!(receipt["mise"]["action"], "installed");
    assert!(receipt["notPreviewed"].as_array().is_some_and(|parts| !parts.is_empty()));
    assert!(fixture.calls().is_empty(), "{:?}", fixture.calls());
}

#[test]
fn a_config_file_is_applied_from_its_own_directory() {
    let fixture = new_mac();
    let directory = fixture.home().join("setup");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("machine.toml"), "[tools]\n").unwrap();
    fixture.mise_in(&directory, &["bootstrap"]).runs(
        "printf '%s|%s' \"$MISE_OVERRIDE_CONFIG_FILENAMES\" \"$MISE_TRUSTED_CONFIG_PATHS\" > \"$FAKE_MISE/env\"\n",
    );
    let file = directory.join("machine.toml");
    let receipt = bootstrap(&fixture, &["--config", &file.display().to_string()]).success();
    assert_eq!(receipt["source"]["kind"], "config");
    let env = std::fs::read_to_string(fixture.path("mise/env")).unwrap();
    assert_eq!(env, format!("machine.toml|{}", file.display()));
}

#[test]
fn bootstrap_checks_its_source_before_doing_anything() {
    let fixture = new_mac();
    for args in [
        &["--from", DOTFILES][..],
        &["--config-dir", "mise"][..],
        &["--save-env"][..],
        &["--config", "/x.toml", "--from", DOTFILES, "--directory", "/d", "--config-dir", "m"][..],
    ] {
        bootstrap(&fixture, args).assert_failed(2, "INVALID_ARGUMENT");
    }
    assert!(fixture.calls().is_empty());
}
