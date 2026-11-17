//! `pkg remove`: it edits only its own entries and never prunes more than it was asked to.

use crate::support::{Fixture, MAC_CONFIG, SHARED_CONFIG, assert_matches_output_schema};

#[test]
fn removing_a_formula_deletes_its_line_and_prunes_it_with_its_dependencies() {
    let fixture = Fixture::seeded();
    let receipt = fixture.json(&["remove", "brew:wget"]).success();
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["removed"][0]["package"], "brew:wget");
    assert_eq!(
        receipt["uninstalled"],
        serde_json::json!(["brew:wget@1.24.5", "brew:libidn2@2.3.8"])
    );
    assert_eq!(receipt["leftInstalled"], serde_json::json!([]));
    assert_eq!(
        fixture.read_dotfile("config.macos.toml"),
        MAC_CONFIG.replace("\"brew:wget\" = \"latest\"\n", ""),
        "only the entry's line goes; its comment stays"
    );
    let calls = fixture.calls();
    let prune =
        calls.iter().position(|call| call == "bootstrap packages prune --yes --manager brew");
    assert!(prune.is_some(), "{calls:?}");
    assert!(
        !calls.iter().any(|call| call.contains("brew-cask")),
        "only the managers involved: {calls:?}"
    );
    assert_matches_output_schema(&["remove"], &receipt);
}

#[test]
fn it_refuses_when_a_prune_would_take_undeclared_packages_too() {
    for args in [&["remove", "brew:wget"][..], &["remove", "brew:wget", "--dry-run"][..]] {
        let fixture = Fixture::seeded();
        fixture
            .mise(&["bootstrap", "packages", "prune", "--dry-run", "--manager", "brew"])
            .nth(1)
            .stdout("remove brew:brightness@1.2\nremove brew:pkgconf@3.0.10\n");
        let before = fixture.snapshot();
        let error = fixture.json(args).assert_failed(3, "PRECONDITION_FAILED");
        assert_eq!(
            error["details"]["undeclared"],
            serde_json::json!(["brew:brightness", "brew:pkgconf"])
        );
        assert!(error["suggestions"].to_string().contains("pkg prune --dry-run"), "{error}");
        assert_eq!(fixture.snapshot(), before);
        assert!(!fixture.calls().iter().any(|call| call.contains("--yes")));
    }
}

#[test]
fn a_package_declared_in_another_config_is_not_touched() {
    let fixture = Fixture::seeded();
    for package in ["brew:git", "jq"] {
        let error = fixture.json(&["remove", package]).assert_failed(3, "PRECONDITION_FAILED");
        assert!(
            error["message"].as_str().unwrap().contains(".dotfiles/mise/config.toml"),
            "{error}"
        );
    }
    assert_eq!(fixture.read_dotfile("config.toml"), SHARED_CONFIG);
}

#[test]
fn an_undeclared_package_is_not_found() {
    let fixture = Fixture::seeded();
    fixture.json(&["remove", "brew:nothing"]).assert_failed(3, "NOT_FOUND");
}

#[test]
fn app_store_apps_leave_the_config_but_stay_installed() {
    let fixture = Fixture::seeded();
    let receipt = fixture.json(&["remove", "mas:937984704"]).success();
    assert_eq!(receipt["leftInstalled"][0]["package"], "mas:937984704");
    assert!(!fixture.read_dotfile("config.macos.toml").contains("mas:937984704"));
    assert!(!fixture.calls().iter().any(|call| call.contains("prune")));
}

#[test]
fn a_formula_something_else_needs_stays_installed() {
    let fixture = Fixture::seeded();
    fixture
        .mise(&["bootstrap", "packages", "prune", "--dry-run", "--manager", "brew"])
        .nth(2)
        .stdout("");
    let receipt = fixture.json(&["remove", "brew:coreutils"]).success();
    assert_eq!(receipt["leftInstalled"][0]["package"], "brew:coreutils");
    assert!(!fixture.calls().iter().any(|call| call.contains("--yes")));
}

#[test]
fn tools_are_removed_with_mise_unuse_on_the_write_target() {
    let fixture = Fixture::seeded();
    let path = fixture.mac_config_link();
    fixture.mise(&["unuse", "-p", &path, "npm:prettier"]).stdout("");
    let receipt = fixture.json(&["remove", "npm:prettier"]).success();
    assert_eq!(receipt["removed"][0]["package"], "npm:prettier");
    assert!(fixture.calls().contains(&format!("unuse -p {path} npm:prettier")));
}

#[test]
fn a_dry_run_changes_nothing() {
    let fixture = Fixture::seeded();
    let before = fixture.snapshot();
    let receipt =
        fixture.json(&["remove", "brew:wget", "brew-cask:firefox", "--dry-run"]).success();
    assert_eq!(receipt["status"], "dry-run");
    assert_eq!(receipt["uninstalled"], serde_json::json!(["brew:wget", "brew-cask:firefox"]));
    assert_eq!(fixture.snapshot(), before);
    let text = Fixture::seeded().run(&["remove", "brew:wget", "--dry-run"]);
    assert!(
        text.text().ends_with("Dry run: nothing was uninstalled or written.\n"),
        "{}",
        text.stdout
    );
}
