use std::process::Command;

#[test]
fn invalid_search_query_precedes_missing_home() {
    let output = Command::new(env!("CARGO_BIN_EXE_pkg"))
        .args(["search", "the and", "--output", "json"])
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let failure: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(failure["error"]["code"], "INVALID_ARGUMENT");
    assert_eq!(failure["error"]["message"], "the query has no words to search for");
    assert!(output.stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn json_receipts_preserve_lossy_display_of_non_utf8_config_paths() {
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::PermissionsExt;

    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    fs::create_dir(&home).unwrap();
    let executable = root.path().join("mise");
    fs::write(&executable, "#!/bin/sh\nshift 2\ncase \"$*\" in\n'backends ls') printf 'npm';;\n'config ls --json') printf '[]';;\n'bootstrap packages status --json') printf '{}';;\n'settings get write_targets.packages') printf 'is not set' >&2; exit 1;;\n'bootstrap packages use --dry-run -g brew:wget') printf 'would install wget';;\nesac\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let config = home.join(OsString::from_vec(b"config-\xff.toml".to_vec()));
    let output = Command::new(env!("CARGO_BIN_EXE_pkg"))
        .args(["install", "brew:wget", "--dry-run", "--output", "json"])
        .env("HOME", &home)
        .env("PKG_MISE", &executable)
        .env("MISE_GLOBAL_CONFIG_FILE", &config)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let receipt: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(receipt["packages"][0]["config"], config.display().to_string());
    assert!(!config.exists());
}
