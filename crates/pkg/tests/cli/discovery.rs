//! Discovery, and the output contract every command shares.

use crate::support::{Fixture, assert_matches_output_schema};

#[test]
fn bare_pkg_and_help_orient_people_and_point_agents_at_discovery() {
    let fixture = Fixture::new();
    for args in [&[][..], &["--help"][..]] {
        let outcome = fixture.run(args);
        assert_eq!(outcome.status, 0);
        for expected in [
            "install",
            "brew-cask:",
            "--dry-run",
            "--output json",
            "cli search",
            "schema",
            "docs search",
            "context show",
        ] {
            assert!(outcome.stdout.contains(expected), "{expected}: {}", outcome.stdout);
        }
    }
}

#[test]
fn help_lists_the_commands_for_people_and_not_the_agent_commands() {
    let outcome = Fixture::new().run(&["--help"]);
    assert!(outcome.stdout.lines().count() < 50, "{}", outcome.stdout);
    assert!(outcome.stdout.contains("[aliases: uninstall, rm]"), "{}", outcome.stdout);
    assert!(!outcome.stdout.contains("Find the command for a task"), "{}", outcome.stdout);
}

#[test]
fn version_names_the_release() {
    let outcome = Fixture::new().run(&["--version"]);
    assert_eq!(outcome.status, 0);
    assert_eq!(outcome.stdout.trim(), format!("{} {}", pkg::NAME, env!("CARGO_PKG_VERSION")));
}

#[test]
fn cli_search_returns_a_bounded_ranked_page_with_exact_commands() {
    let result = Fixture::new().json(&["cli", "search", "uninstall", "a", "cask"]).success();
    let items = result["items"].as_array().unwrap();
    assert!(!items.is_empty() && items.len() <= 5);
    assert_eq!(items[0]["command"], format!("{} remove", pkg::NAME));
    assert!(items[0]["effect"].as_str().unwrap().starts_with("modifies"));
    assert!(result.get("hasMore").is_some() && result.get("nextCursor").is_some());
    assert_matches_output_schema(&["cli", "search"], &result);
}

#[test]
fn cli_search_pages_through_every_match_with_its_cursor() {
    let fixture = Fixture::new();
    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut args = vec!["cli", "search", "package", "config", "--limit", "2"];
        if let Some(cursor) = &cursor {
            args.extend(["--cursor", cursor.as_str()]);
        }
        let page = fixture.json(&args).success();
        for item in page["items"].as_array().unwrap() {
            seen.push(item["id"].as_str().unwrap().to_string());
        }
        if page["hasMore"] == false {
            assert!(page["nextCursor"].is_null());
            break;
        }
        cursor = Some(page["nextCursor"].as_str().unwrap().to_string());
    }
    let unique: std::collections::BTreeSet<&String> = seen.iter().collect();
    assert!(seen.len() > 2 && unique.len() == seen.len(), "{seen:?}");
}

#[test]
fn a_cursor_past_the_last_match_is_an_invalid_argument() {
    let fixture = Fixture::new();
    for search in [&["cli", "search", "package"][..], &["docs", "search", "prune"][..]] {
        let first = fixture.json(&[search, &["--limit", "1"]].concat()).success();
        assert_eq!(first["nextCursor"], "1", "{search:?}");
        let error = fixture
            .json(&[search, &["--cursor", "1000"]].concat())
            .assert_failed(2, "INVALID_ARGUMENT");
        assert_eq!(error["details"]["argument"], "cursor", "{search:?}");
    }
}

#[test]
fn schema_lists_every_command_and_describes_one_by_any_spelling() {
    let fixture = Fixture::new();
    let list = fixture.json(&["schema"]).success();
    let commands: Vec<&str> =
        list["items"].as_array().unwrap().iter().map(|item| item["id"].as_str().unwrap()).collect();
    assert!(
        commands.contains(&"install")
            && commands.contains(&"tap.add")
            && commands.contains(&"cli.search")
    );

    let entry = fixture.json(&["schema", "install"]).success();
    assert_eq!(entry["id"], "install");
    assert_eq!(entry["dryRun"], true);
    assert_eq!(entry["effect"]["kind"], "modifies");
    assert_eq!(entry["aliases"], serde_json::json!(["i", "add"]));
    let packages = entry["arguments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|argument| argument["name"] == "packages")
        .unwrap();
    assert_eq!(packages["required"], true);
    assert_eq!(packages["pattern"], pkg::registry::PACKAGE_PATTERN);
    assert!(entry["errors"].as_array().unwrap().iter().any(|error| error["code"] == "MISE_FAILED"));
    assert_eq!(entry["arguments"].as_array().unwrap().last().unwrap()["default"], "text");
    assert_eq!(fixture.json(&["schema", "tap.add"]).success()["id"], "tap.add");
    assert_eq!(fixture.json(&["schema", "rm"]).success()["id"], "remove");
}

#[test]
fn an_unknown_command_in_schema_is_not_found_with_suggestions() {
    let error = Fixture::new().json(&["schema", "tap", "rename"]).assert_failed(3, "NOT_FOUND");
    assert!(error["suggestions"].to_string().contains("schema tap"), "{error}");
}

#[test]
fn missing_input_fails_at_once_naming_the_missing_argument() {
    let error = Fixture::new().json(&["install"]).assert_failed(2, "INVALID_ARGUMENT");
    assert_eq!(error["retryable"], false);
    assert!(error["details"]["missing"].to_string().contains("PACKAGES"), "{error}");
    assert!(error["details"]["arguments"].is_array());
    assert!(error["suggestions"].to_string().contains("schema install"));
}

#[test]
fn a_malformed_package_is_an_invalid_argument() {
    let fixture = Fixture::new();
    for package in ["brew wget", "Brew:wget", "brew:"] {
        let error = fixture.json(&["install", package]).assert_failed(2, "INVALID_ARGUMENT");
        assert!(error["message"].as_str().unwrap().contains("brew:wget"), "{error}");
    }
}

#[test]
fn an_unaccepted_value_lists_what_is_accepted() {
    let error = Fixture::new()
        .json(&["search", "x", "--manager", "apt"])
        .assert_failed(2, "INVALID_ARGUMENT");
    assert!(error["details"]["acceptedValues"].to_string().contains("brew-cask"), "{error}");
}

#[test]
fn a_resource_without_an_operation_lists_its_operations() {
    let error = Fixture::new().json(&["tap"]).assert_failed(2, "INVALID_ARGUMENT");
    assert!(error["details"]["commands"].to_string().contains("tap add"), "{error}");
}

#[test]
fn an_unknown_command_points_at_search() {
    let error = Fixture::new().json(&["frobnicate"]).assert_failed(2, "INVALID_ARGUMENT");
    assert!(error["suggestions"].to_string().contains("cli search"), "{error}");
}

#[test]
fn a_positional_passed_as_a_flag_gets_the_right_form() {
    let error = Fixture::new()
        .json(&["info", "--package", "brew:wget"])
        .assert_failed(2, "INVALID_ARGUMENT");
    let first = error["suggestions"][0].as_str().unwrap();
    assert_eq!(first, format!("Pass the package without a flag: {} info <package>", pkg::NAME));
}

#[test]
fn errors_list_their_fields_in_contract_order() {
    let outcome = Fixture::new().json(&["schema", "nothing"]);
    let keys = ["\"code\"", "\"message\"", "\"retryable\"", "\"details\"", "\"suggestions\""];
    let positions: Vec<usize> = keys.iter().map(|key| outcome.stderr.find(key).unwrap()).collect();
    assert!(positions.is_sorted(), "{}", outcome.stderr);
}

#[test]
fn text_is_the_default_for_results_and_errors() {
    let fixture = Fixture::new();
    let text = fixture.run(&["schema", "install"]);
    assert!(text.text().starts_with(&format!("{} install", pkg::NAME)));
    assert!(serde_json::from_str::<serde_json::Value>(&text.stdout).is_err());

    let unknown = fixture.run(&["schema", "no", "such"]);
    assert_eq!(unknown.status, 3);
    assert!(unknown.stdout.is_empty());
    assert!(unknown.stderr.starts_with("error: no pkg command `no such`\n"), "{}", unknown.stderr);
    assert!(unknown.stderr.contains("hint: pkg schema"), "{}", unknown.stderr);

    let missing = fixture.run(&["install"]);
    assert_eq!(missing.status, 2);
    assert!(missing.stderr.contains("<PACKAGES>"), "{}", missing.stderr);
}

#[test]
fn docs_search_returns_sections_to_read() {
    let fixture = Fixture::new();
    let result = fixture.json(&["docs", "search", "removing", "packages"]).success();
    let first = &result["items"][0];
    assert_eq!(first["path"], "docs/packages.md");
    assert_eq!(first["section"], "Removing");
    assert_matches_output_schema(&["docs", "search"], &result);

    let document = fixture.json(&["docs", "show", "docs/packages.md"]).success();
    assert!(document["markdown"].as_str().unwrap().contains("## Naming packages"));
    let text = fixture.run(&["docs", "show", "docs/catalogs.md"]);
    assert!(text.text().starts_with("# Catalogs"));
}

#[test]
fn docs_show_of_an_unknown_path_lists_the_documents() {
    let error =
        Fixture::new().json(&["docs", "show", "docs/nothing.md"]).assert_failed(3, "NOT_FOUND");
    assert!(error["details"]["documents"].to_string().contains("docs/catalogs.md"), "{error}");
}

#[cfg(unix)]
#[test]
fn context_show_resolves_the_config_link_to_the_dotfiles_checkout() {
    let fixture = Fixture::seeded();
    let context = fixture.json(&["context", "show"]).success();
    assert_eq!(context["packagesConfig"]["setting"], "~/.config/mise/config.macos.toml");
    assert_eq!(context["packagesConfig"]["path"], fixture.mac_config_link());
    let real = fixture.dotfile("config.macos.toml").canonicalize().unwrap();
    assert_eq!(context["packagesConfig"]["realPath"], real.display().to_string());
    assert_eq!(context["mise"]["version"], "2026.11.9 macos-arm64 (2026-11-16)");
    assert!(context["catalogs"]["homebrewIndexFetchedAt"].is_null());
    assert_matches_output_schema(&["context", "show"], &context);

    let text = fixture.run(&["context", "show"]);
    assert!(text.text().contains("packages go to"), "{}", text.stdout);
    assert!(text.stdout.contains(".dotfiles/mise/config.macos.toml"), "{}", text.stdout);
}

#[cfg(unix)]
#[test]
fn without_a_write_target_setting_declarations_go_to_the_global_config() {
    let fixture = Fixture::seeded();
    fixture
        .mise(&["settings", "get", "write_targets.packages"])
        .stderr("mise ERROR Setting [write_targets.packages] is not set\n")
        .status(1);
    let context = fixture.json(&["context", "show"]).success();
    assert!(context["packagesConfig"]["setting"].is_null());
    assert_eq!(
        context["packagesConfig"]["path"],
        fixture.home().join(".config/mise/config.toml").display().to_string()
    );
}

#[cfg(unix)]
#[test]
fn a_missing_mise_is_a_precondition_failure_that_says_how_to_install_it() {
    let fixture = Fixture::seeded();
    std::fs::remove_file(fixture.path("bin/mise")).unwrap();
    let error = fixture.json(&["list"]).assert_failed(3, "PRECONDITION_FAILED");
    assert!(error["suggestions"].to_string().contains("mise.run"), "{error}");
}
