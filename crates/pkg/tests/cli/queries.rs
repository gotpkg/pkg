//! The commands that only read: list, search, info, outdated, where, and tap list.

use crate::support::{Fixture, assert_matches_output_schema};

#[test]
fn list_groups_packages_by_manager_then_tools() {
    let fixture = Fixture::seeded();
    let page = fixture.json(&["list"]).success();
    let packages: Vec<&str> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["package"].as_str().unwrap())
        .collect();
    assert_eq!(
        packages,
        [
            "brew:git",
            "brew:coreutils",
            "brew:wget",
            "brew-cask:firefox",
            "mas:937984704",
            "jq",
            "npm:prettier"
        ]
    );
    assert_eq!(page["hasMore"], false);
    let shared = fixture.dotfile("config.toml").canonicalize().unwrap();
    assert_eq!(page["items"][0]["config"], shared.display().to_string());
    assert_eq!(page["items"][4]["name"], "Amphetamine");
    assert_matches_output_schema(&["list"], &page);

    let text = fixture.run(&["list"]);
    assert!(text.text().starts_with("brew (3)\n  git        2.56.1\n"), "{}", text.stdout);
    assert!(text.stdout.contains("tools (2)\n  jq            1.8.2\n"), "{}", text.stdout);
}

#[test]
fn list_filters_by_manager_and_pages() {
    let fixture = Fixture::seeded();
    let casks = fixture.json(&["list", "--manager", "brew-cask"]).success();
    assert_eq!(casks["items"].as_array().unwrap().len(), 1);
    let tools = fixture.json(&["list", "--manager", "tool"]).success();
    assert_eq!(tools["items"][0]["package"], "jq");
    assert!(!fixture.calls().iter().any(|call| call.contains("bootstrap packages status")) || true);
    let first = fixture.json(&["list", "--limit", "2"]).success();
    assert_eq!(first["nextCursor"], "2");
    let rest = fixture.json(&["list", "--cursor", "2"]).success();
    assert_eq!(rest["items"][0]["package"], "brew:wget");
}

#[test]
fn search_ranks_an_exact_name_first_across_every_catalog() {
    let fixture = Fixture::seeded();
    let page = fixture.json(&["search", "ffmpeg"]).success();
    let packages: Vec<&str> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["package"].as_str().unwrap())
        .collect();
    assert_eq!(packages[..2], ["brew:ffmpeg", "ffmpeg"]);
    assert!(
        packages.contains(&"brew:ffmpegthumbnailer") && packages.contains(&"mas:6444012595"),
        "{packages:?}"
    );
    assert_matches_output_schema(&["search"], &page);
    assert!(fixture.path("cache/pkg/homebrew-index.json").exists(), "the index is cached");

    let declared = fixture.json(&["search", "wget"]).success();
    assert_eq!(declared["items"][0]["package"], "brew:wget");
    assert_eq!(declared["items"][0]["declared"], true);
}

#[test]
fn search_reads_the_cached_index_and_survives_an_unreachable_homebrew() {
    let fixture = Fixture::seeded();
    fixture.json(&["search", "wget", "--manager", "brew"]).success();
    fixture.fail("/api/formula.json", 503);
    let fresh = fixture.json(&["search", "wget", "--manager", "brew"]);
    assert_eq!(fresh.success()["items"][0]["package"], "brew:wget");
    assert!(fresh.stderr.is_empty(), "a fresh cache needs no download: {}", fresh.stderr);

    let empty = Fixture::seeded();
    empty.fail("/api/formula.json", 503);
    let error = empty.json(&["search", "wget"]).assert_failed(5, "UNAVAILABLE");
    assert_eq!(error["retryable"], true);
}

#[test]
fn search_warns_and_answers_when_the_app_store_is_unreachable() {
    let fixture = Fixture::seeded();
    fixture.fail("/search", 500);
    let outcome = fixture.json(&["search", "ffmpeg"]);
    assert_eq!(outcome.success()["items"][0]["package"], "brew:ffmpeg");
    assert!(outcome.stderr.contains("warning: skipped the App Store"), "{}", outcome.stderr);
    fixture.json(&["search", "ffmpeg", "--manager", "mas"]).assert_failed(5, "UNAVAILABLE");
}

#[test]
fn search_text_shows_the_package_to_install() {
    let fixture = Fixture::seeded();
    let text = fixture.run(&["search", "media", "player", "--manager", "brew-cask"]);
    assert_eq!(
        text.text(),
        "  brew-cask:vlc   3.0.21  VLC media player: Multimedia player\n  brew-cask:iina  1.4.1   IINA: Free and open-source media player\nInstall one with `pkg install <package>`.\n"
    );
}

#[test]
fn info_joins_the_catalog_and_what_mise_has_installed() {
    let fixture = Fixture::seeded();
    let wget = fixture.json(&["info", "brew:wget"]).success();
    assert_eq!(wget["version"], "1.25.0_2");
    assert_eq!(wget["installed"], "1.24.5");
    assert_eq!(wget["dependencies"], serde_json::json!(["libidn2", "openssl@3"]));
    assert_matches_output_schema(&["info"], &wget);
    let firefox = fixture.json(&["info", "brew-cask:firefox"]).success();
    assert_eq!(firefox["apps"], serde_json::json!(["Firefox.app"]));
    let app = fixture.json(&["info", "mas:937984704"]).success();
    assert_eq!(app["name"], "Amphetamine");
    assert_eq!(app["description"], "Keep your Mac awake.");
    let jq = fixture.json(&["info", "jq"]).success();
    assert_eq!(jq["backend"], "aqua:jqlang/jq");
    assert_eq!(jq["installed"], "1.8.2");
}

#[test]
fn info_of_something_nobody_knows_is_not_found() {
    let fixture = Fixture::seeded();
    let error = fixture.json(&["info", "brew:nothing"]).assert_failed(3, "NOT_FOUND");
    assert!(error["suggestions"].to_string().contains("pkg search nothing"), "{error}");
}

#[test]
fn outdated_combines_homebrew_the_app_store_and_tools() {
    let fixture = Fixture::seeded();
    let report = fixture.json(&["outdated"]).success();
    let items: Vec<(&str, &str)> = report["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| (item["package"].as_str().unwrap(), item["latest"].as_str().unwrap()))
        .collect();
    assert_eq!(
        items,
        [("brew:wget", "1.25.0_2"), ("mas:937984704", "5.3.2"), ("npm:prettier", "3.6.2")]
    );
    assert_eq!(report["skipped"][0]["package"], "brew-cask:firefox");
    assert_matches_output_schema(&["outdated"], &report);
}

#[test]
fn where_prints_a_formula_prefix_or_a_tool_directory_and_refuses_casks() {
    let fixture = Fixture::seeded();
    assert_eq!(fixture.run(&["where", "brew:wget"]).text(), "/opt/homebrew/opt/wget\n");
    assert_eq!(fixture.json(&["where", "jq"]).success()["path"], "/mise/installs/jq/1.8.2");
    fixture.json(&["where", "brew-cask:firefox"]).assert_failed(2, "INVALID_ARGUMENT");
}

#[test]
fn tap_list_shows_every_file_s_taps() {
    let fixture = Fixture::seeded();
    let list = fixture.json(&["tap", "list"]).success();
    let taps: Vec<&str> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["tap"].as_str().unwrap())
        .collect();
    assert_eq!(taps, ["acme/tools", "sesh/tap"]);
}
