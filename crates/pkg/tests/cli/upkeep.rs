//! The commands that keep a machine current: update, sync, prune, and the tap changes.

use serde_json::json;

use crate::support::{Fixture, MAC_CONFIG, mise_version, status};

fn ready_to_update(fixture: &Fixture) {
    fixture.mise(&["bootstrap", "packages", "upgrade", "--yes"]).stdout("");
    fixture.mise(&["upgrade", "--global"]).stdout("");
}

#[test]
fn update_updates_mise_before_anything_else() {
    let fixture = Fixture::seeded();
    ready_to_update(&fixture);
    fixture.mise(&["version", "--json"]).nth(2).json(&mise_version("2026.11.10", "2026.11.10"));
    let receipt = fixture.json(&["update"]).success();
    assert_eq!(
        receipt["updated"][0],
        json!({ "package": "mise", "from": "2026.11.9", "to": "2026.11.10" })
    );
    let calls = fixture.calls();
    let position = |call: &str| {
        calls.iter().position(|made| made == call).unwrap_or_else(|| panic!("{call}: {calls:?}"))
    };
    assert!(position("self-update --yes") < position("bootstrap packages upgrade --yes"));
}

#[test]
fn update_leaves_a_mise_from_a_package_manager_to_it() {
    let fixture = Fixture::seeded();
    ready_to_update(&fixture);
    fixture.disable_mise_self_update();
    fixture.mise(&["version", "--json"]).json(&mise_version("2026.11.9", "2026.11.10"));
    let receipt = fixture.json(&["update"]).success();
    assert_eq!(receipt["skipped"][0]["package"], "mise", "{receipt}");
    assert!(!fixture.calls().iter().any(|call| call.starts_with("self-update")));
}

#[test]
fn a_failed_mise_update_changes_nothing_else() {
    let fixture = Fixture::seeded();
    ready_to_update(&fixture);
    fixture.mise(&["self-update", "--yes"]).stderr("mise ERROR cannot reach GitHub\n").status(1);
    let error = fixture.json(&["update"]).assert_failed(6, "MISE_FAILED");
    assert!(error["message"].as_str().unwrap().contains("cannot reach GitHub"), "{error}");
    let calls = fixture.calls();
    assert!(!calls.iter().any(|call| call.starts_with("bootstrap packages upgrade")), "{calls:?}");
}

#[test]
fn update_reports_the_versions_that_changed() {
    let fixture = Fixture::seeded();
    fixture.mise(&["bootstrap", "packages", "upgrade", "--yes"]).stdout("");
    fixture.mise(&["upgrade", "--global"]).stdout("");
    let mut updated = status(&[]);
    updated["brew"]["packages"][2]["installed_version"] = "1.25.0_2".into();
    fixture.mise(&["bootstrap", "packages", "status", "--json"]).nth(2).json(&updated);
    let receipt = fixture.json(&["update"]).success();
    assert_eq!(
        receipt["updated"],
        serde_json::json!([{ "package": "brew:wget", "from": "1.24.5", "to": "1.25.0_2" }])
    );
    let calls = fixture.calls();
    assert!(calls.contains(&"bootstrap packages upgrade --yes".to_string()), "{calls:?}");
    assert!(calls.contains(&"upgrade --global".to_string()), "{calls:?}");
}

#[test]
fn update_reads_a_casks_new_version_from_the_caskroom() {
    let fixture = Fixture::seeded();
    fixture.mise(&["bootstrap", "packages", "upgrade", "--yes"]).runs(
        "mv \"$MISE_SYSTEM_BREW_PREFIX/Caskroom/firefox/160.0\" \"$MISE_SYSTEM_BREW_PREFIX/Caskroom/firefox/161.0\"\n",
    );
    fixture.mise(&["upgrade", "--global"]).stdout("");
    let receipt = fixture.json(&["update"]).success();
    assert_eq!(
        receipt["updated"],
        serde_json::json!([{ "package": "brew-cask:firefox", "from": "160.0", "to": "161.0" }])
    );
}

#[test]
fn updating_named_packages_runs_only_their_manager() {
    let fixture = Fixture::seeded();
    fixture.json(&["update", "brew:wget"]).success();
    let calls = fixture.calls();
    assert!(calls.contains(&"bootstrap packages upgrade --yes brew:wget".to_string()), "{calls:?}");
    assert!(!calls.iter().any(|call| call.starts_with("upgrade")), "{calls:?}");
    assert!(!calls.iter().any(|call| call.starts_with("self-update")), "{calls:?}");
    fixture.json(&["update", "brew:nothing"]).assert_failed(3, "NOT_FOUND");
}

#[test]
fn an_update_dry_run_lists_what_outdated_lists() {
    let fixture = Fixture::seeded();
    let receipt = fixture.json(&["update", "--dry-run"]).success();
    assert_eq!(receipt["status"], "dry-run");
    assert_eq!(
        receipt["updated"][0],
        serde_json::json!({ "package": "brew:wget", "from": "1.24.5", "to": "1.25.0_2" })
    );
    assert!(!fixture.calls().iter().any(|call| call.contains("--yes")));
}

#[test]
fn the_upgrade_command_is_gone() {
    Fixture::seeded().json(&["upgrade"]).assert_failed(2, "INVALID_ARGUMENT");
}

#[test]
fn sync_installs_only_when_something_is_missing() {
    let fixture = Fixture::seeded();
    let receipt = fixture.json(&["sync"]).success();
    assert_eq!(receipt["installed"], serde_json::json!([]));
    assert!(!fixture.calls().iter().any(|call| call.contains("apply") || call == "install"));

    let fixture = Fixture::seeded();
    let missing = status(&[("brew", "tree", None)]);
    fixture.mise(&["bootstrap", "packages", "status", "--json"]).nth(1).json(&missing);
    fixture.mise(&["bootstrap", "packages", "status", "--json"]).nth(2).json(&missing);
    fixture.mise(&["bootstrap", "packages", "apply", "--yes"]).stdout("");
    let dry = fixture.json(&["sync", "--dry-run"]).success();
    assert_eq!(dry["installed"], serde_json::json!(["brew:tree"]));
    let receipt = fixture.json(&["sync"]).success();
    assert_eq!(receipt["installed"], serde_json::json!(["brew:tree"]));
    assert!(fixture.calls().contains(&"bootstrap packages apply --yes".to_string()));
}

#[test]
fn sync_installs_a_cask_the_caskroom_does_not_have() {
    let fixture = Fixture::seeded();
    std::fs::remove_dir(fixture.caskroom().join("firefox/160.0")).unwrap();
    fixture.mise(&["bootstrap", "packages", "apply", "--yes"]).stdout("");
    let dry = fixture.json(&["sync", "--dry-run"]).success();
    assert_eq!(dry["installed"], serde_json::json!(["brew-cask:firefox"]));
    let receipt = fixture.json(&["sync"]).success();
    assert_eq!(receipt["missing"], serde_json::json!(["brew-cask:firefox"]));
    assert!(fixture.calls().contains(&"bootstrap packages apply --yes".to_string()));
}

#[test]
fn prune_previews_then_removes_what_no_config_declares() {
    let fixture = Fixture::seeded();
    fixture
        .mise(&["bootstrap", "packages", "prune", "--dry-run", "--manager", "brew"])
        .stdout("remove brew:brightness@1.2\n");
    let preview = fixture.json(&["prune", "--dry-run"]).success();
    assert_eq!(preview["uninstalled"], serde_json::json!(["brew:brightness@1.2"]));
    assert!(!fixture.calls().iter().any(|call| call.contains("--yes")));

    let receipt = fixture.json(&["prune"]).success();
    assert_eq!(receipt["status"], "completed");
    let calls = fixture.calls();
    assert!(
        calls.contains(&"bootstrap packages prune --yes --manager brew".to_string()),
        "{calls:?}"
    );
    assert!(
        !calls.iter().any(|call| call.contains("--yes --manager brew-cask")),
        "nothing to prune there: {calls:?}"
    );
}

#[test]
fn tap_add_and_remove_edit_only_the_write_target() {
    let fixture = Fixture::seeded();
    let added = fixture
        .json(&["tap", "add", "acme/tools", "https://github.com/acme/brew-tools.git"])
        .success();
    assert_eq!(added["action"], "added");
    let path = fixture.mac_config_link();
    assert!(fixture.calls().contains(&format!(
        "bootstrap packages brew tap -p {path} acme/tools https://github.com/acme/brew-tools.git"
    )));
    let removed = fixture.json(&["tap", "remove", "acme/tools"]).success();
    assert_eq!(removed["url"], "https://github.com/acme/brew-tools.git");
    let error =
        fixture.json(&["tap", "remove", "sesh/tap"]).assert_failed(3, "PRECONDITION_FAILED");
    assert!(error["message"].as_str().unwrap().contains("config.toml"), "{error}");
    fixture.json(&["tap", "remove", "nobody/tap"]).assert_failed(3, "NOT_FOUND");
    assert_eq!(
        fixture.read_dotfile("config.macos.toml"),
        MAC_CONFIG,
        "the fake mise wrote nothing"
    );
}
