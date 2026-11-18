//! `pkg setup`: installing mise, putting its tools on PATH, and handing pkg to mise.

use std::ffi::OsStr;
use std::path::PathBuf;

use serde_json::json;

use crate::support::{
    FAKE_MISE_INSTALLER, Fixture, INSTALLS_PKG, Outcome, assert_matches_output_schema, mise_version,
};

const PKG_TOOL: [&str; 3] = ["use", "-g", "github:markmals/pkg@latest"];

/// A new Mac: no mise, no mise config, and the installer's pkg in `~/.local/bin`.
fn new_machine() -> Fixture {
    let fixture = Fixture::new();
    fixture.serve_text("/install", FAKE_MISE_INSTALLER);
    fixture.mise(&["version", "--json"]).json(&mise_version("2026.11.9", "2026.11.9"));
    fixture.mise(&["backends", "ls"]).stdout("aqua\ngithub\nnpm\n");
    fixture.mise(&["config", "ls", "--json"]).json(&json!([]));
    for setting in ["write_targets.packages", "write_targets.tools"] {
        fixture
            .mise(&["settings", "get", setting])
            .stderr(&format!("mise ERROR Setting [{setting}] is not set\n"))
            .status(1);
    }
    fixture.mise(&["ls", "--global", "--json"]).json(&json!({}));
    let config = fixture.home().join(".config/mise/config.toml");
    let declared = json!({ "github:markmals/pkg": [{
        "version": "0.0.0", "requested_version": "latest", "installed": true, "active": true,
        "source": { "type": "mise.toml", "path": config },
    }]});
    fixture.mise(&["ls", "--global", "--json"]).nth(2).json(&declared);
    fixture.mise(&PKG_TOOL).runs(&format!(
        "{INSTALLS_PKG}mkdir -p \"$HOME/.config/mise\"\nprintf '[tools]\\n\"github:markmals/pkg\" = \"latest\"\\n' > \"$HOME/.config/mise/config.toml\"\n"
    ));
    installer_copy(&fixture, "echo 'pkg 0.0.0'");
    fixture
}

fn installer_copy(fixture: &Fixture, body: &str) {
    let path = local_pkg(fixture);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn local_pkg(fixture: &Fixture) -> PathBuf {
    fixture.home().join(".local/bin/pkg")
}

fn setup(fixture: &Fixture, args: &[&str]) -> Outcome {
    let installer = format!("{}/install", fixture.catalog_url());
    let mut command = vec!["setup"];
    command.extend_from_slice(args);
    command.extend(["--output", "json"]);
    fixture.run_with(
        &command,
        &[
            ("PKG_MISE", None),
            ("SHELL", Some(OsStr::new("/bin/zsh"))),
            ("PKG_MISE_INSTALLER", Some(OsStr::new(&installer))),
        ],
    )
}

#[test]
fn setup_installs_mise_puts_its_tools_on_path_and_hands_pkg_to_mise() {
    let fixture = new_machine();
    let receipt = setup(&fixture, &[]).success();
    assert_matches_output_schema(&["setup"], &receipt);

    let mise = fixture.home().join(".local/bin/mise");
    assert_eq!(receipt["mise"]["action"], "installed");
    assert_eq!(receipt["mise"]["program"], mise.display().to_string());
    assert!(mise.is_file(), "mise's installer put mise in ~/.local/bin");

    let zshenv = std::fs::read_to_string(fixture.home().join(".zshenv")).unwrap();
    assert_eq!(
        zshenv,
        "# >>> pkg >>>\nexport PATH=\"$HOME/.local/bin:$HOME/.local/share/mise/shims:$PATH\"\n# <<< pkg <<<\n"
    );
    assert_eq!(receipt["path"]["action"], "added");
    assert_eq!(receipt["path"]["load"], "source ~/.zshenv");

    assert_eq!(receipt["handoff"]["declaration"], "added");
    assert_eq!(receipt["handoff"]["link"], "linked");
    let shim = fixture.home().join(".local/share/mise/shims/pkg");
    assert_eq!(std::fs::read_link(local_pkg(&fixture)).unwrap(), shim);
    assert!(fixture.calls().contains(&PKG_TOOL.join(" ")));
}

#[test]
fn setup_again_changes_nothing() {
    let fixture = new_machine();
    setup(&fixture, &[]).success();
    let config = fixture.home().join(".config/mise/config.toml");
    fixture.mise(&["config", "ls", "--json"]).json(&json!([{ "path": config, "tools": [] }]));
    fixture.mise(&["ls", "--global", "--json"]).nth(3).json(&json!({ "github:markmals/pkg": [{
        "version": "0.0.0", "requested_version": "latest", "installed": true, "active": true,
        "source": { "type": "mise.toml", "path": config },
    }]}));
    let before = fixture.snapshot();
    let receipt = setup(&fixture, &[]).success();
    assert_eq!(receipt["mise"]["action"], "found");
    assert!(receipt["path"].is_null(), "a configuration exists now: {receipt}");
    assert_eq!(receipt["handoff"]["declaration"], "existing");
    assert_eq!(receipt["handoff"]["link"], "already-linked");
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn a_setup_dry_run_says_what_it_would_do_and_changes_nothing() {
    let fixture = new_machine();
    let before = fixture.snapshot();
    let receipt = setup(&fixture, &["--dry-run"]).success();
    assert_eq!(receipt["status"], "dry-run");
    assert_eq!(receipt["mise"]["action"], "installed");
    assert_eq!(receipt["path"]["action"], "added");
    assert_eq!(receipt["handoff"]["declaration"], "added");
    assert_eq!(receipt["handoff"]["link"], "linked");
    assert_eq!(fixture.snapshot(), before);
    assert!(fixture.calls().is_empty(), "no mise ran: {:?}", fixture.calls());
}

#[test]
fn no_modify_path_prints_the_line_instead() {
    let fixture = new_machine();
    let receipt = setup(&fixture, &["--no-modify-path"]).success();
    assert_eq!(receipt["path"]["action"], "printed");
    assert_eq!(
        receipt["path"]["line"],
        "export PATH=\"$HOME/.local/bin:$HOME/.local/share/mise/shims:$PATH\""
    );
    assert!(!fixture.home().join(".zshenv").exists());
}

#[test]
fn setup_reuses_the_mise_it_finds_and_leaves_path_to_an_existing_config() {
    let fixture = Fixture::seeded();
    let receipt = fixture.json(&["setup"]).success();
    assert_eq!(receipt["mise"]["action"], "found");
    assert!(receipt["path"].is_null(), "{receipt}");
    assert_eq!(receipt["handoff"]["declaration"], "added");
    assert_eq!(receipt["handoff"]["link"], "none", "no installer copy to replace");
}

#[test]
fn setup_refuses_a_mise_older_than_pkg_supports() {
    let fixture = Fixture::seeded();
    fixture.mise(&["version", "--json"]).json(&mise_version("2026.1.1", "2026.11.9"));
    let error = fixture.json(&["setup"]).assert_failed(3, "PRECONDITION_FAILED");
    assert!(error["message"].as_str().unwrap().contains("2026.11.9"), "{error}");
    assert!(!fixture.calls().iter().any(|call| call.starts_with("use")));
}

#[test]
fn setup_never_installs_over_a_pkg_mise_that_is_missing() {
    let fixture = new_machine();
    let missing = fixture.path("nowhere/mise");
    let outcome = fixture
        .run_with(&["setup", "--output", "json"], &[("PKG_MISE", Some(missing.as_os_str()))]);
    outcome.assert_failed(3, "PRECONDITION_FAILED");
    assert!(!fixture.home().join(".local/bin/mise").exists());
}

#[test]
fn setup_keeps_a_local_bin_pkg_that_is_not_pkg() {
    let fixture = new_machine();
    installer_copy(&fixture, "echo 'something else 1.0'");
    let receipt = setup(&fixture, &[]).success();
    assert_eq!(receipt["handoff"]["link"], "kept");
    assert!(receipt["handoff"]["reason"].as_str().is_some(), "{receipt}");
    assert!(std::fs::symlink_metadata(local_pkg(&fixture)).unwrap().is_file());
}
