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

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

/// A variable of this process, where an empty one counts as unset.
pub(crate) fn process_variable(key: &str) -> Option<OsString> {
    std::env::var_os(key).filter(|value| !value.is_empty())
}

/// The shim directory of this process, or `None` without a home directory.
pub(crate) fn process_directory() -> Option<PathBuf> {
    directory::location(catalog::System::current(), process_variable)
}

/// Keeps pkg from meeting its own shims: takes the shim directory off the `PATH` in `environment`,
/// which pkg then looks mise up in and hands to every process it starts.
pub(crate) fn take_shims_off_path(environment: &mut BTreeMap<OsString, OsString>) {
    let Some(directory) = directory::location(catalog::System::current(), |key| {
        environment.get(OsStr::new(key)).filter(|value| !value.is_empty()).cloned()
    }) else {
        return;
    };
    let Some(key) =
        environment.keys().find(|key| key.to_string_lossy().eq_ignore_ascii_case("path")).cloned()
    else {
        return;
    };
    let search_path = &environment[&key];
    if path::contains(search_path, &directory) {
        let without = path::without(search_path, &directory);
        environment.insert(key, without);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::path::Path;

    use super::*;

    fn environment(pairs: &[(&str, &Path)]) -> BTreeMap<OsString, OsString> {
        pairs.iter().map(|(key, value)| (OsString::from(key), value.into())).collect()
    }

    #[test]
    fn the_shim_directory_comes_off_the_path_and_the_rest_stays() {
        let temporary = tempfile::tempdir().expect("a directory");
        let home = temporary.path();
        let shims = home.join(".local/share/pkg/shims");
        std::fs::create_dir_all(&shims).expect("a directory");
        let path =
            std::env::join_paths([shims.as_path(), Path::new("/opt/bin"), Path::new("/bin")])
                .expect("a path");
        let mut variables = environment(&[("HOME", home)]);
        variables.insert("PATH".into(), path);

        take_shims_off_path(&mut variables);
        assert_eq!(variables[OsStr::new("PATH")], OsString::from("/opt/bin:/bin"));
        assert_eq!(variables[OsStr::new("HOME")], home.as_os_str());
    }

    #[test]
    fn a_moved_shim_directory_is_the_one_taken_off() {
        let temporary = tempfile::tempdir().expect("a directory");
        let data = temporary.path().join("data");
        let shims = data.join("pkg/shims");
        std::fs::create_dir_all(&shims).expect("a directory");
        let path = std::env::join_paths([shims.as_path(), Path::new("/bin")]).expect("a path");
        let mut variables =
            environment(&[("HOME", Path::new("/home/mark")), ("XDG_DATA_HOME", data.as_path())]);
        variables.insert("PATH".into(), path);

        take_shims_off_path(&mut variables);
        assert_eq!(variables[OsStr::new("PATH")], OsString::from("/bin"));
    }

    #[test]
    fn a_path_without_the_shims_is_left_alone() {
        let mut variables = environment(&[("HOME", Path::new("/home/mark"))]);
        variables.insert("PATH".into(), "/usr/bin:/bin".into());
        take_shims_off_path(&mut variables);
        assert_eq!(variables[OsStr::new("PATH")], OsString::from("/usr/bin:/bin"));
    }

    #[test]
    fn an_environment_without_a_path_or_a_home_is_left_alone() {
        let mut variables = BTreeMap::new();
        take_shims_off_path(&mut variables);
        assert!(variables.is_empty());
    }
}
