#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;

use pkg_core::{Client, LocateRequest, Settings};
use tempfile::TempDir;

#[test]
fn subprocess_home_matches_the_explicit_context_without_changing_the_host() {
    let sandbox = TempDir::new().unwrap();
    let home = sandbox.path().join("home");
    fs::create_dir(&home).unwrap();
    let executable = sandbox.path().join("mise");
    fs::write(&executable, "#!/bin/sh\nshift 2\ncase \"$*\" in\n'config ls --json') printf '[]';;\n'backends ls') printf 'npm';;\n'where ripgrep') printf '%s' \"$HOME\";;\nesac\n").unwrap();
    let host_home = std::env::var_os("HOME");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let mut client = Client::new(Settings::new(home.clone(), executable.into_os_string())).unwrap();

    let observed = client.locate(LocateRequest { package: "ripgrep".into() }, &mut |_| {}).unwrap();
    assert_eq!(observed.result.path, home);
    assert_eq!(std::env::var_os("HOME"), host_home);
}

#[test]
fn default_configuration_keeps_host_overlay_write_targets() {
    let sandbox = TempDir::new().unwrap();
    let home = sandbox.path().to_path_buf();
    let executable = home.join("mise");
    fs::write(
        &executable,
        r#"#!/bin/sh
shift 2
case "$*" in
    'backends ls') printf 'npm';;
    'config ls --json') printf '[]';;
    'ls --global --json') printf '{}';;
    'version --json') printf '{"version":"2026.11.9 macos-arm64 (2026-11-16)","latest":null}';;
    'settings get write_targets.'*)
        if [ -n "$MISE_GLOBAL_CONFIG_FILE" ]; then
            printf 'is not set' >&2
            exit 1
        fi
        printf '%s/host.toml' "$MISE_CONFIG_DIR";;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let mut client = Client::new(Settings::new(home.clone(), executable.into_os_string())).unwrap();
    let context = client.context(&mut |_| {}).unwrap().result;
    assert_eq!(context.packages_config.path, home.join(".config/mise/host.toml"));
    assert_eq!(context.tools_config.path, home.join(".config/mise/host.toml"));
}
