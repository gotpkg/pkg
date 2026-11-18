#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;

use pkg_core::{
    Client, ErrorKind, Mode, OutputStream, Progress, RemoveRequest, Settings, TapAddRequest,
};
use tempfile::TempDir;

fn fixture(body: &str) -> (TempDir, Settings) {
    let root = TempDir::new().unwrap();
    let executable = root.path().join("mise");
    fs::write(&executable, format!("#!/bin/sh\nshift 2\n{body}")).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let settings = Settings::new(root.path().to_path_buf(), executable.into_os_string());
    (root, settings)
}

fn tap() -> TapAddRequest {
    TapAddRequest {
        tap: "example/tap".into(),
        url: "https://github.com/example/homebrew-tap".into(),
        mode: Mode::Apply,
    }
}

#[test]
fn tap_targets_the_native_configuration_path() {
    let (root, mut settings) = fixture(
        r#"
case "$*" in
 'backends ls') printf 'npm';;
 'config ls --json') printf '[]';;
 'settings get write_targets.packages') printf 'is not set' >&2; exit 1;;
 'bootstrap packages brew tap '*)
    [ "$6" = "$MISE_GLOBAL_CONFIG_FILE" ] || exit 9;;
esac
"#,
    );
    let config = root.path().join(OsString::from_vec(b"config-\xff.toml".to_vec()));
    settings.global_config_override = Some(config.clone());
    let mut client = Client::new(settings).unwrap();
    let outcome = client.tap_add(tap(), &mut |_| {}).unwrap();
    assert_eq!(outcome.result.config, config);
}

#[test]
fn tap_reports_both_output_streams_to_the_library_caller() {
    let (_root, settings) = fixture(
        r#"
case "$*" in
 'backends ls') printf 'npm';;
 'config ls --json') printf '[]';;
 'settings get write_targets.packages') printf 'is not set' >&2; exit 1;;
 'bootstrap packages brew tap '*) printf 'tapped\n'; printf 'warning\n' >&2;;
esac
"#,
    );
    let mut client = Client::new(settings).unwrap();
    let mut streams = [String::new(), String::new()];
    client
        .tap_add(tap(), &mut |event| {
            if let Progress::Output { stream, text } = event {
                streams[if stream == OutputStream::Stdout { 0 } else { 1 }].push_str(text);
            }
        })
        .unwrap();
    assert_eq!(streams, ["tapped\n", "warning\n"]);
}

#[test]
fn removal_failure_retains_completed_declaration_removals() {
    let (root, settings) = fixture(
        r#"
case "$*" in
 'backends ls') printf 'npm';;
 'config ls --json') printf '[{"path":"%s/config.toml"}]' "$HOME";;
 'settings get write_targets.packages') printf '%s/config.toml' "$HOME";;
 'bootstrap packages prune --dry-run --manager brew')
    if [ -f "$HOME/checked" ]; then
        printf 'mise ERROR cannot query installed packages\n' >&2
        exit 1
    fi
    printf checked > "$HOME/checked";;
esac
"#,
    );
    let config = root.path().join("config.toml");
    fs::write(
        &config,
        "[bootstrap.packages]\n\"brew:wget\" = \"latest\"\n\"brew:tree\" = \"latest\"\n",
    )
    .unwrap();
    let mut client = Client::new(settings).unwrap();
    let error = client
        .remove(
            RemoveRequest {
                packages: vec!["brew:wget".into(), "brew:tree".into()],
                mode: Mode::Apply,
            },
            &mut |_| {},
        )
        .err()
        .unwrap();
    assert_eq!(error.kind, ErrorKind::MiseFailed);
    let changed = fs::read_to_string(config).unwrap();
    assert!(!changed.contains("brew:wget") && !changed.contains("brew:tree"));
    assert_eq!(error.details.removed, Some(vec!["brew:wget".into(), "brew:tree".into()]));
}
