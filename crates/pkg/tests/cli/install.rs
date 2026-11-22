//! `pkg install`: what it asks mise to do, what it records, and how it recovers.

use crate::support::{Fixture, MAC_CONFIG, assert_matches_output_schema, status};

#[test]
fn a_new_formula_is_declared_in_the_write_target_and_installed() {
    let fixture = Fixture::seeded();
    fixture.mise(&["bootstrap", "packages", "status", "--json"]).json(&status(&[(
        "brew",
        "hello",
        Some("2.12.3"),
    )]));
    let receipt = fixture.json(&["install", "brew:hello"]).success();
    assert_eq!(receipt["status"], "completed");
    let item = &receipt["packages"][0];
    assert_eq!(item["action"], "added");
    assert_eq!(item["kind"], "package");
    assert_eq!(item["version"], "2.12.3");
    let real = fixture.dotfile("config.macos.toml").canonicalize().unwrap();
    assert_eq!(item["config"], real.display().to_string());
    assert!(fixture.calls().contains(&"bootstrap packages use --yes -g brew:hello".to_string()));
    assert!(fixture.read_dotfile("config.macos.toml").contains("\"brew:hello\" = \"latest\""));
    assert_matches_output_schema(&["install"], &receipt);
}

#[test]
fn text_names_the_dotfiles_file_it_was_declared_in() {
    let fixture = Fixture::seeded();
    let outcome = fixture.run(&["install", "brew:hello"]);
    assert_eq!(
        outcome.text(),
        "Installed brew:hello, declared in ~/.dotfiles/mise/config.macos.toml\n"
    );
}

#[test]
fn state_is_read_only_after_installing_a_package_nothing_declares_yet() {
    let fixture = Fixture::seeded();
    fixture.json(&["install", "brew:hello", "--dry-run"]).success();
    assert!(
        !fixture.calls().iter().any(|call| call == "bootstrap packages status --json"),
        "{:?}",
        fixture.calls()
    );

    fixture.json(&["install", "brew:hello"]).success();
    let calls = fixture.calls();
    let installed =
        calls.iter().position(|call| call == "bootstrap packages use --yes -g brew:hello");
    let reads: Vec<usize> = calls
        .iter()
        .enumerate()
        .filter(|(_, call)| *call == "bootstrap packages status --json")
        .map(|(index, _)| index)
        .collect();
    assert_eq!(reads.len(), 1, "{calls:?}");
    assert!(installed.is_some_and(|installed| installed < reads[0]), "{calls:?}");
}

#[test]
fn an_installed_package_is_left_alone() {
    let fixture = Fixture::seeded();
    let receipt = fixture.json(&["install", "brew:wget", "jq"]).success();
    assert_eq!(receipt["packages"][0]["action"], "unchanged");
    assert_eq!(receipt["packages"][0]["version"], "1.24.5");
    assert_eq!(receipt["packages"][1]["action"], "unchanged");
    assert_eq!(receipt["packages"][1]["kind"], "tool");
    let changes: Vec<String> = fixture
        .calls()
        .into_iter()
        .filter(|call| {
            call.contains(" use ")
                || call.starts_with("use")
                || call.contains("apply")
                || call.starts_with("install")
        })
        .collect();
    assert!(changes.is_empty(), "{changes:?}");
    assert_eq!(fixture.read_dotfile("config.macos.toml"), MAC_CONFIG);
}

#[test]
fn a_declared_but_missing_package_is_installed_where_it_is_declared() {
    let fixture = Fixture::seeded();
    fixture.mise(&["bootstrap", "packages", "status", "--json"]).nth(1).json(&{
        let mut missing = status(&[]);
        missing["brew"]["packages"][0]["state"] = "missing".into();
        missing["brew"]["packages"][0]["installed_version"] = serde_json::Value::Null;
        missing
    });
    fixture.mise(&["bootstrap", "packages", "apply", "--yes", "brew:git"]).stdout("");
    let receipt = fixture.json(&["install", "brew:git"]).success();
    assert_eq!(receipt["packages"][0]["action"], "installed");
    let shared = fixture.dotfile("config.toml").canonicalize().unwrap();
    assert_eq!(receipt["packages"][0]["config"], shared.display().to_string());
    assert!(fixture.calls().contains(&"bootstrap packages apply --yes brew:git".to_string()));
}

#[test]
fn tools_go_through_mise_use_at_latest_unless_a_version_is_given() {
    let fixture = Fixture::seeded();
    fixture.mise(&["use", "-g", "npm:cowsay@1.5.0"]).stdout("");
    let receipt = fixture.json(&["install", "ripgrep", "npm:cowsay@1.5.0"]).success();
    assert_eq!(receipt["packages"][0]["kind"], "tool");
    assert_eq!(receipt["packages"][0]["package"], "ripgrep");
    assert_eq!(receipt["packages"][1]["package"], "npm:cowsay");
    let calls = fixture.calls();
    assert!(calls.contains(&"use -g ripgrep@latest".to_string()), "{calls:?}");
    assert!(calls.contains(&"use -g npm:cowsay@1.5.0".to_string()), "{calls:?}");
}

#[test]
fn a_failed_install_takes_its_new_declaration_back_out() {
    let fixture = Fixture::seeded();
    fixture
        .mise(&["bootstrap", "packages", "use", "--yes", "-g", "brew:hello"])
        .declares("brew:hello")
        .stderr("mise ERROR no bottle for hello on this macOS\nmise ERROR Version: 2026.11.9\n")
        .status(1);
    let outcome = fixture.json(&["install", "brew:wget", "brew:hello", "brew-cask:iina"]);
    let error = outcome.assert_failed(6, "MISE_FAILED");
    assert!(error["message"].as_str().unwrap().contains("no bottle for hello"), "{error}");
    assert_eq!(error["details"]["package"], "brew:hello");
    assert_eq!(error["details"]["installed"], serde_json::json!(["brew:wget"]));
    assert_eq!(fixture.read_dotfile("config.macos.toml"), MAC_CONFIG);
    assert!(
        !fixture.calls().iter().any(|call| call.contains("brew-cask:iina")),
        "stopped at the failure"
    );

    let text = fixture.run(&["install", "brew:hello"]);
    assert_eq!(text.status, 6);
    assert!(
        text.stderr.contains("mise ERROR no bottle"),
        "mise's own output passes through: {}",
        text.stderr
    );
    assert!(text.stderr.contains("error: mise could not install brew:hello"), "{}", text.stderr);
    assert_eq!(fixture.read_dotfile("config.macos.toml"), MAC_CONFIG);
}

#[test]
fn a_dry_run_shows_mises_plan_and_changes_nothing() {
    let fixture = Fixture::seeded();
    let before = fixture.snapshot();
    let receipt = fixture.json(&["install", "brew:hello", "--dry-run"]).success();
    assert_eq!(receipt["status"], "dry-run");
    assert_eq!(receipt["packages"][0]["action"], "added");
    assert!(receipt["plan"].to_string().contains("pour hello/2.12.3"), "{receipt}");
    assert_eq!(fixture.snapshot(), before);
    assert!(!fixture.calls().iter().any(|call| call.contains("--yes")));
}

#[test]
fn an_unknown_manager_is_mises_error() {
    let fixture = Fixture::seeded();
    fixture
        .mise(&["bootstrap", "packages", "use", "--yes", "-g", "nope:thing"])
        .stderr("mise ERROR unknown bootstrap package manager 'nope'\n")
        .status(1);
    let error = fixture.json(&["install", "nope:thing"]).assert_failed(6, "MISE_FAILED");
    assert!(
        error["message"].as_str().unwrap().contains("unknown bootstrap package manager"),
        "{error}"
    );
    assert_eq!(fixture.read_dotfile("config.macos.toml"), MAC_CONFIG);
}
