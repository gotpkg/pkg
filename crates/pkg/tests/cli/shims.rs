//! `pkg shims`: the shim files, what a shim refuses, and what it hands to the real tool.
//!
//! The shims are shell scripts that call `pkg`, so each test builds a `PATH` by hand: the shim
//! directory, a directory of fake real tools, and the directory of the `pkg` under test. The
//! system directories are left out so that no real `npm` or `dotnet` on the machine running the
//! tests can answer.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::support::{Fixture, Outcome, SYSTEM_PATH};

fn pkg_directory() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_pkg")).parent().expect("pkg's directory").to_path_buf()
}

fn shim_directory(fixture: &Fixture) -> PathBuf {
    fixture.home().join(".local/share/pkg/shims")
}

fn install_shims(fixture: &Fixture) {
    fixture.run(&["shims", "install"]).text();
}

fn write_tool(directory: &Path, name: &str, body: &str) -> PathBuf {
    std::fs::create_dir_all(directory).expect("a tool directory");
    let path = directory.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("a fake tool");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("executable");
    path
}

fn joined(directories: &[&Path]) -> String {
    directories
        .iter()
        .map(|directory| directory.display().to_string())
        .collect::<Vec<_>>()
        .join(":")
}

/// Runs the installed shim `name` with `path` as `PATH`.
fn run_shim(fixture: &Fixture, name: &str, args: &[&str], path: &str) -> Outcome {
    let shim = shim_directory(fixture).join(name);
    fixture.finish(&mut fixture.command_for(&shim, args, &[("PATH", Some(OsStr::new(path)))]))
}

/// pkg with `args` and `path` as `PATH`.
fn run_pkg(fixture: &Fixture, args: &[&str], path: &str) -> Outcome {
    fixture.run_with(args, &[("PATH", Some(OsStr::new(path)))])
}

/// A fixture with every shim installed, a directory of fake real tools, and the usual `PATH`:
/// shims, then the fake tools, then pkg.
fn shimmed() -> (Fixture, PathBuf, String) {
    let fixture = Fixture::new();
    install_shims(&fixture);
    let real = fixture.path("real");
    std::fs::create_dir_all(&real).expect("the real tools");
    let path = joined(&[&shim_directory(&fixture), &real, &pkg_directory()]);
    (fixture, real, path)
}

fn changes(receipt: &Value) -> Vec<&str> {
    receipt["shims"]
        .as_array()
        .expect("shims")
        .iter()
        .map(|shim| shim["change"].as_str().expect("a change"))
        .collect()
}

#[test]
fn installing_writes_a_marked_launcher_for_each_manager_of_this_system() {
    let fixture = Fixture::new();
    let receipt = fixture.json(&["shims", "install"]).success();
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["onPath"], false);
    assert_eq!(receipt["pathLine"], "export PATH=\"$HOME/.local/share/pkg/shims:$PATH\"");
    assert!(changes(&receipt).iter().all(|change| *change == "created"), "{receipt}");

    let directory = shim_directory(&fixture);
    for name in ["npm", "mise", "cargo", "pip", "uv", "go", "gem", "dotnet", "brew"] {
        let file = directory.join(name);
        let script = std::fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("no shim for {name}: {error}"));
        assert!(script.contains("pkg-shim"), "{name} has no marker:\n{script}");
        assert!(script.contains(&format!("shims run {name} --")), "{name}:\n{script}");
        let mode = std::fs::metadata(&file).expect("metadata").permissions().mode();
        assert!(mode & 0o111 != 0, "{name} is not executable");
    }
    assert!(!directory.join("winget").exists(), "a Windows manager has a shim on Unix");
    #[cfg(target_os = "macos")]
    {
        assert!(directory.join("mas").exists() && directory.join("port").exists());
        assert!(!directory.join("apt").exists(), "a Linux manager has a shim on macOS");
    }
    #[cfg(target_os = "linux")]
    {
        assert!(directory.join("apt").exists() && directory.join("apt-get").exists());
        assert!(!directory.join("mas").exists(), "a macOS manager has a shim on Linux");
    }
}

#[test]
fn installing_again_changes_nothing_and_repairs_a_stale_shim() {
    let fixture = Fixture::new();
    install_shims(&fixture);
    let again = fixture.json(&["shims", "install"]).success();
    assert!(changes(&again).iter().all(|change| *change == "unchanged"), "{again}");

    std::fs::write(shim_directory(&fixture).join("npm"), "#!/bin/sh\n# pkg-shim\nexit 1\n")
        .expect("a stale shim");
    let repaired = fixture.json(&["shims", "install"]).success();
    let updated: Vec<&str> = repaired["shims"]
        .as_array()
        .expect("shims")
        .iter()
        .filter(|shim| shim["change"] == "updated")
        .map(|shim| shim["name"].as_str().expect("a name"))
        .collect();
    assert_eq!(updated, ["npm"], "{repaired}");
}

#[test]
fn dry_runs_write_and_delete_nothing() {
    let fixture = Fixture::new();
    let before = fixture.snapshot();
    let preview = fixture.json(&["shims", "install", "--dry-run"]).success();
    assert_eq!(preview["status"], "dry-run");
    assert!(changes(&preview).iter().all(|change| *change == "created"), "{preview}");
    assert_eq!(fixture.snapshot(), before);

    install_shims(&fixture);
    let installed = fixture.snapshot();
    let preview = fixture.json(&["shims", "remove", "--dry-run"]).success();
    assert_eq!(preview["status"], "dry-run");
    assert!(changes(&preview).iter().all(|change| *change == "removed"), "{preview}");
    assert_eq!(fixture.snapshot(), installed);
}

#[test]
fn a_file_that_is_not_a_shim_stops_the_install_before_anything_is_written() {
    let fixture = Fixture::new();
    let directory = shim_directory(&fixture);
    std::fs::create_dir_all(&directory).expect("the shim directory");
    let mine = directory.join("npm");
    std::fs::write(&mine, "#!/bin/sh\necho mine\n").expect("a foreign file");

    let error = fixture.json(&["shims", "install"]).assert_failed(3, "PRECONDITION_FAILED");
    assert_eq!(error["details"]["files"][0], mine.to_string_lossy().as_ref(), "{error}");
    let written: Vec<_> = std::fs::read_dir(&directory).expect("the directory").collect();
    assert_eq!(written.len(), 1, "something else was written: {written:?}");
    assert_eq!(std::fs::read_to_string(&mine).expect("untouched"), "#!/bin/sh\necho mine\n");
}

#[test]
fn removing_deletes_only_shims_and_then_the_empty_directory() {
    let fixture = Fixture::new();
    install_shims(&fixture);
    let directory = shim_directory(&fixture);
    let notes = directory.join("notes.txt");
    std::fs::write(&notes, "mine").expect("a foreign file");

    let receipt = fixture.json(&["shims", "remove"]).success();
    assert!(changes(&receipt).iter().all(|change| *change == "removed"), "{receipt}");
    assert!(!directory.join("npm").exists());
    assert!(notes.exists(), "a file that is not a shim was deleted");

    std::fs::remove_file(&notes).expect("remove the note");
    install_shims(&fixture);
    fixture.run(&["shims", "remove"]).text();
    assert!(!directory.exists(), "the empty directory stayed");
}

#[test]
fn listing_shows_whether_the_directory_is_on_the_path_and_what_each_name_runs() {
    let fixture = Fixture::new();
    let before = fixture.json(&["shims", "list"]).success();
    assert_eq!(before["onPath"], false);
    assert!(
        before["shims"].as_array().expect("shims").iter().all(|shim| shim["installed"] == false)
    );

    install_shims(&fixture);
    let directory = shim_directory(&fixture);
    let real = fixture.path("real");
    let npm = write_tool(&real, "npm", "exit 0");
    let resolves = |listed: &Value, name: &str| -> Value {
        listed["shims"]
            .as_array()
            .expect("shims")
            .iter()
            .find(|shim| shim["name"] == name)
            .unwrap_or_else(|| panic!("no entry for {name}"))["resolvesTo"]
            .clone()
    };

    let shadowed = joined(&[&real, &directory]);
    let listed = run_pkg(&fixture, &["shims", "list", "--output", "json"], &shadowed).success();
    assert_eq!(listed["onPath"], true);
    assert_eq!(resolves(&listed, "npm")["kind"], "other");
    assert_eq!(resolves(&listed, "npm")["path"], npm.to_string_lossy().as_ref());
    assert_eq!(resolves(&listed, "cargo")["kind"], "shim");

    let nothing = run_pkg(&fixture, &["shims", "list", "--output", "json"], &joined(&[&real]));
    let nothing = nothing.success();
    assert_eq!(nothing["onPath"], false);
    assert_eq!(resolves(&nothing, "cargo")["kind"], "none");
}

#[test]
fn a_shim_refuses_a_global_install_and_names_the_pkg_command() {
    let (fixture, real, path) = shimmed();
    let log = fixture.path("ran");
    let record = format!("echo \"$0 $*\" >> {}", log.display());
    write_tool(&real, "npm", &record);
    write_tool(&real, "brew", &record);

    let npm = run_shim(&fixture, "npm", &["install", "-g", "prettier"], &path);
    assert_eq!(npm.status, 2, "{npm:?}");
    assert!(npm.stdout.is_empty(), "{npm:?}");
    assert!(npm.stderr.contains("pkg install npm:prettier"), "{}", npm.stderr);
    assert!(npm.stderr.contains("pkg search"), "{}", npm.stderr);

    let brew = run_shim(&fixture, "brew", &["install", "--cask", "iina"], &path);
    assert_eq!(brew.status, 2, "{brew:?}");
    assert!(brew.stderr.contains("pkg install brew-cask:iina"), "{}", brew.stderr);

    let removal = run_shim(&fixture, "brew", &["uninstall", "wget"], &path);
    assert_eq!(removal.status, 2, "{removal:?}");
    assert!(removal.stderr.contains("pkg remove brew:wget"), "{}", removal.stderr);

    assert!(
        !log.exists(),
        "a refused command ran the real tool: {:?}",
        std::fs::read_to_string(&log)
    );
}

#[test]
fn a_shim_hands_everything_else_to_the_real_tool_with_its_status() {
    let (fixture, real, path) = shimmed();
    write_tool(&real, "npm", "echo \"real npm: $*\"; echo \"warning\" >&2; exit 7");

    let version = run_shim(&fixture, "npm", &["--version"], &path);
    assert_eq!(version.status, 7, "{version:?}");
    assert_eq!(version.stdout, "real npm: --version\n");
    assert_eq!(version.stderr, "warning\n");

    let project = run_shim(&fixture, "npm", &["install", "prettier"], &path);
    assert_eq!(project.status, 7, "a project install was refused: {project:?}");
    assert_eq!(project.stdout, "real npm: install prettier\n");

    let local = run_shim(&fixture, "npm", &["install", "-g", "./tool.tgz"], &path);
    assert_eq!(local.status, 7, "an install from a file was refused: {local:?}");
}

#[test]
fn a_shim_without_a_real_tool_says_so_and_exits_127() {
    let (fixture, _real, path) = shimmed();
    let outcome = run_shim(&fixture, "gem", &["list"], &path);
    assert_eq!(outcome.status, 127, "{outcome:?}");
    assert!(outcome.stderr.contains("gem"), "{}", outcome.stderr);
    assert!(outcome.stderr.contains("not installed"), "{}", outcome.stderr);
}

#[test]
fn a_shim_runs_the_real_tool_when_pkg_is_not_on_the_path() {
    let (fixture, real, _) = shimmed();
    write_tool(&real, "npm", "echo \"real npm: $*\"");
    let without_pkg = joined(&[&shim_directory(&fixture), &real]);

    let outcome = run_shim(&fixture, "npm", &["install", "-g", "prettier"], &without_pkg);
    assert_eq!(outcome.status, 0, "{outcome:?}");
    assert_eq!(outcome.stdout, "real npm: install -g prettier\n");
    assert!(outcome.stderr.is_empty(), "{}", outcome.stderr);
}

#[test]
fn the_mise_shim_keeps_the_shims_from_the_package_managers_mise_runs() {
    let (fixture, real, path) = shimmed();
    write_tool(&real, "mise", "printf '%s\\n' \"$PATH\"");
    let shims = shim_directory(&fixture).display().to_string();

    let bootstrap = run_shim(&fixture, "mise", &["bootstrap", "packages", "apply"], &path);
    assert_eq!(bootstrap.status, 0, "{bootstrap:?}");
    assert!(!bootstrap.stdout.contains(&shims), "mise saw the shims: {}", bootstrap.stdout);
    assert!(bootstrap.stdout.contains(&real.display().to_string()), "{}", bootstrap.stdout);

    let project_use = run_shim(&fixture, "mise", &["use", "ripgrep"], &path);
    assert!(!project_use.stdout.contains(&shims), "{}", project_use.stdout);

    for args in [&["exec", "--", "env"][..], &["-C", "/somewhere", "run", "build"][..]] {
        let kept = run_shim(&fixture, "mise", args, &path);
        assert!(
            kept.stdout.contains(&shims),
            "`mise {}` lost the shims: {}",
            args.join(" "),
            kept.stdout
        );
    }

    let global = run_shim(&fixture, "mise", &["use", "-g", "ripgrep"], &path);
    assert_eq!(global.status, 2, "{global:?}");
    assert!(global.stderr.contains("pkg install ripgrep"), "{}", global.stderr);
}

#[test]
fn pkg_runs_the_mise_behind_the_shims_and_gives_it_a_path_without_them() {
    let fixture = Fixture::seeded();
    install_shims(&fixture);
    let shims = shim_directory(&fixture);
    let hit = fixture.path("shim-mise-ran");
    std::fs::write(
        shims.join("mise"),
        format!("#!/bin/sh\n# pkg-shim\necho ran >> {}\nexit 99\n", hit.display()),
    )
    .expect("a mise shim that records its use");
    let seen = fixture.path("mise-path");
    let record = format!("printf '%s' \"$PATH\" > {}", seen.display());
    for args in [
        &["settings", "get", "write_targets.packages"][..],
        &["config", "ls", "--json"][..],
        &["ls", "--global", "--json"][..],
    ] {
        fixture.mise(args).runs(&record);
    }

    let path = format!("{}:{}:{SYSTEM_PATH}", shims.display(), fixture.path("bin").display());
    let listed = fixture.run_with(
        &["list", "--output", "json"],
        &[("PKG_MISE", None), ("PATH", Some(OsStr::new(&path)))],
    );
    listed.success();
    assert!(!hit.exists(), "pkg ran mise through its shim");
    let mise_path = std::fs::read_to_string(&seen).expect("the fake mise recorded its PATH");
    assert!(!mise_path.contains(&shims.display().to_string()), "mise saw the shims: {mise_path}");
}

/// Runs `command` and fails the test when it has not finished within `seconds`: a shim that
/// loops would otherwise hang the suite.
fn finish_within(mut command: std::process::Command, seconds: u64) -> Outcome {
    let mut child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the shim starts");
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while child.try_wait().expect("a status").is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("the shim had not finished after {seconds} seconds");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let output = child.wait_with_output().expect("the output");
    Outcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[test]
fn shims_in_two_directories_without_pkg_end_in_127_instead_of_calling_each_other() {
    let (fixture, _real, _path) = shimmed();
    let first = shim_directory(&fixture);
    let second = fixture.path("more-shims");
    std::fs::create_dir_all(&second).expect("a second directory");
    for entry in std::fs::read_dir(&first).expect("the shims") {
        let entry = entry.expect("an entry").path();
        std::fs::copy(&entry, second.join(entry.file_name().expect("a name"))).expect("a copy");
    }
    let path = joined(&[&first, &second]);
    let shim = first.join("cargo");
    let command = fixture.command_for(&shim, &["--version"], &[("PATH", Some(OsStr::new(&path)))]);
    let outcome = finish_within(command, 15);
    assert_eq!(outcome.status, 127, "{outcome:?}");
    assert!(outcome.stderr.contains("not installed"), "{}", outcome.stderr);
}

#[test]
fn arguments_that_are_not_utf8_reach_the_real_tool_byte_for_byte() {
    let (fixture, real, path) = shimmed();
    write_tool(&real, "npm", "printf '%s' \"$2\" | od -An -tx1");
    let shim = shim_directory(&fixture).join("npm");
    let path = format!("{path}:{SYSTEM_PATH}");
    let mut command = fixture.command_for(&shim, &["run"], &[("PATH", Some(OsStr::new(&path)))]);
    command.arg(OsStr::from_bytes(b"\xff\xfe"));
    let outcome = fixture.finish(&mut command);
    assert_eq!(outcome.status, 0, "{outcome:?}");
    assert_eq!(outcome.stdout.split_whitespace().collect::<Vec<_>>(), ["ff", "fe"], "{outcome:?}");
}

#[test]
fn the_managers_own_output_option_does_not_change_how_a_refusal_is_written() {
    let (fixture, _real, path) = shimmed();
    let outcome = run_shim(&fixture, "mise", &["use", "-g", "--output=json", "ripgrep"], &path);
    assert_eq!(outcome.status, 2, "{outcome:?}");
    assert!(outcome.stderr.starts_with("error: mise did not run"), "{}", outcome.stderr);
    assert!(outcome.stderr.contains("pkg install ripgrep"), "{}", outcome.stderr);
}

#[test]
fn listing_does_not_call_a_file_the_shim_directory_holds_a_shim_unless_it_is_one() {
    let fixture = Fixture::new();
    install_shims(&fixture);
    let directory = shim_directory(&fixture);
    write_tool(&directory, "uv", "exit 0");

    let listed = run_pkg(&fixture, &["shims", "list", "--output", "json"], &joined(&[&directory]));
    let listed = listed.success();
    let uv = listed["shims"]
        .as_array()
        .expect("shims")
        .iter()
        .find(|shim| shim["name"] == "uv")
        .expect("uv");
    assert_eq!(uv["installed"], false, "{uv}");
    assert_eq!(uv["resolvesTo"]["kind"], "other", "{uv}");
}
