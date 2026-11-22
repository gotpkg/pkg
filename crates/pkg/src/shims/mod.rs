//! Shims: small executables named for other package managers, which stop a global install and
//! name the `pkg` command that does it, and run the real tool for everything else.
//!
//! What a shim refuses is a pure function ([`decide`]) of a table ([`catalog`]); the files
//! ([`directory`], [`launcher`]) and the processes ([`passthrough`], [`path`]) stay at the edges.

pub(crate) mod catalog;
pub(crate) mod decide;
pub(crate) mod directory;
pub(crate) mod launcher;
pub(crate) mod passthrough;
pub(crate) mod path;

use std::ffi::OsString;
use std::path::PathBuf;

use pkg_core::Settings;

/// A variable of this process, where an empty one counts as unset.
pub(crate) fn process_variable(key: &str) -> Option<OsString> {
    std::env::var_os(key).filter(|value| !value.is_empty())
}

/// Keeps pkg from meeting its own shims: runs the first `mise` outside the shim directory, unless
/// `PKG_MISE` names one, and gives every process pkg starts a `PATH` without that directory.
pub(crate) fn keep_pkg_away_from_shims(settings: &mut Settings, mise_chosen: bool) {
    let Some(directory) = directory::location(catalog::System::current(), |key| {
        settings.child_env.get(std::ffi::OsStr::new(key)).cloned()
    }) else {
        return;
    };
    let Some(key) = path_key(settings) else { return };
    let search_path = settings.child_env[&key].clone();
    if !path::contains(&search_path, &directory) {
        return;
    }
    let without = path::without(&search_path, &directory);
    if !mise_chosen {
        let extensions = path::executable_extensions(
            settings.child_env.get(std::ffi::OsStr::new("PATHEXT")).map(OsString::as_os_str),
        );
        if let Some(mise) =
            path::find_executable(&path::directories(&without), "mise", &extensions, |_| false)
        {
            settings.mise = mise.into_os_string();
        }
    }
    settings.child_env.insert(key, without);
}

/// The name `PATH` goes by in the child environment: `Path` on Windows.
fn path_key(settings: &Settings) -> Option<OsString> {
    settings
        .child_env
        .keys()
        .find(|key| key.to_string_lossy().eq_ignore_ascii_case("path"))
        .cloned()
}

/// The shim directory of this process, or `None` without a home directory.
pub(crate) fn process_directory() -> Option<PathBuf> {
    directory::location(catalog::System::current(), process_variable)
}

#[cfg(all(test, unix))]
mod tests {
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use super::*;

    fn executable(directory: &Path, name: &str) -> PathBuf {
        std::fs::create_dir_all(directory).expect("a directory");
        let file = directory.join(name);
        std::fs::write(&file, "#!/bin/sh\n").expect("a tool");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).expect("mode");
        file
    }

    fn settings(home: &Path, path: &str, mise: &str) -> Settings {
        let mut settings = Settings::new(home.to_path_buf(), mise.into());
        settings.child_env.insert("HOME".into(), home.into());
        settings.child_env.insert("PATH".into(), path.into());
        settings
    }

    #[test]
    fn pkg_runs_the_mise_behind_the_shims_and_hides_the_directory_from_children() {
        let temporary = tempfile::tempdir().expect("a directory");
        let home = temporary.path();
        let shims = home.join(".local/share/pkg/shims");
        executable(&shims, "mise");
        let real = executable(&home.join("bin"), "mise");
        let path = format!("{}:{}", shims.display(), home.join("bin").display());

        let mut settings = settings(home, &path, "mise");
        keep_pkg_away_from_shims(&mut settings, false);
        assert_eq!(settings.mise, real.as_os_str());
        assert_eq!(settings.child_env[OsStr::new("PATH")], home.join("bin").as_os_str());
    }

    #[test]
    fn a_chosen_mise_is_kept() {
        let temporary = tempfile::tempdir().expect("a directory");
        let home = temporary.path();
        let shims = home.join(".local/share/pkg/shims");
        executable(&shims, "mise");
        executable(&home.join("bin"), "mise");
        let path = format!("{}:{}", shims.display(), home.join("bin").display());

        let mut settings = settings(home, &path, "/opt/mise");
        keep_pkg_away_from_shims(&mut settings, true);
        assert_eq!(settings.mise, "/opt/mise");
        assert_eq!(settings.child_env[OsStr::new("PATH")], home.join("bin").as_os_str());
    }

    #[test]
    fn a_path_without_the_shims_is_left_alone() {
        let temporary = tempfile::tempdir().expect("a directory");
        let mut settings = settings(temporary.path(), "/usr/bin:/bin", "mise");
        keep_pkg_away_from_shims(&mut settings, false);
        assert_eq!(settings.mise, "mise");
        assert_eq!(settings.child_env[OsStr::new("PATH")], "/usr/bin:/bin");
    }
}
