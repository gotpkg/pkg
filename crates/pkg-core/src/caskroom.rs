//! Homebrew's Caskroom, where pkg reads whether a cask is installed. mise checks a cask against
//! its current definition, which it fetches over the network, so pkg leaves casks out of mise's
//! check and reads their version directories here instead.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};

use crate::error::{Error as Failure, ErrorDetails};
use crate::mise::PackageState;

pub(crate) struct Caskroom {
    directory: PathBuf,
}

impl Caskroom {
    /// The Caskroom under mise's Homebrew prefix: `MISE_SYSTEM_BREW_PREFIX` from `environment`
    /// when it is set and nonempty, else `/opt/homebrew` on macOS and `/home/linuxbrew/.linuxbrew`
    /// on Linux. `None` on any other host, where mise installs no casks.
    pub(crate) fn of(environment: &BTreeMap<OsString, OsString>) -> Option<Caskroom> {
        let default = if cfg!(target_os = "macos") {
            "/opt/homebrew"
        } else if cfg!(target_os = "linux") {
            "/home/linuxbrew/.linuxbrew"
        } else {
            return None;
        };
        let prefix = environment
            .get(OsStr::new("MISE_SYSTEM_BREW_PREFIX"))
            .filter(|prefix| !prefix.is_empty())
            .map_or_else(|| PathBuf::from(default), PathBuf::from);
        Some(Caskroom { directory: prefix.join("Caskroom") })
    }

    #[cfg(test)]
    fn at(directory: PathBuf) -> Caskroom {
        Caskroom { directory }
    }

    /// The installed version of the cask `name`, written `token` or `owner/tap/token`: the one
    /// directory in `<Caskroom>/<token>/` other than `.metadata` and mise's `.mise-*` work
    /// directories. `None` when there is none, or several, as mise counts them.
    ///
    /// # Errors
    ///
    /// `PRECONDITION_FAILED` when the directory exists but cannot be read.
    pub(crate) fn installed_version(&self, name: &str) -> Result<Option<String>, Failure> {
        let token = name.rsplit('/').next().unwrap_or(name);
        let directory = self.directory.join(token);
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(unreadable(&directory, &error)),
        };
        let mut versions = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| unreadable(&directory, &error))?;
            let is_directory =
                entry.file_type().map_err(|error| unreadable(&directory, &error))?.is_dir();
            // mise skips a name that is not UTF-8, so it cannot be a version either.
            let Ok(version) = entry.file_name().into_string() else { continue };
            if is_directory && version != ".metadata" && !version.starts_with(".mise-") {
                versions.push(version);
            }
        }
        Ok(match <[String; 1]>::try_from(versions) {
            Ok([version]) => Some(version),
            Err(_) => None,
        })
    }

    /// Replaces the state mise left out for the `brew-cask:` entry `state` with what the Caskroom
    /// says, keeping its key, requested version and desired state.
    ///
    /// # Errors
    ///
    /// `PRECONDITION_FAILED` when the cask's directory cannot be read.
    pub(crate) fn fill(&self, state: &mut PackageState) -> Result<(), Failure> {
        let name = state.key.split_once(':').map_or(state.key.as_str(), |(_, name)| name);
        let installed = self.installed_version(name)?;
        state.state = cask_state(&state.requested, &state.desired, installed.as_deref()).into();
        state.installed = installed;
        state.available = true;
        Ok(())
    }
}

/// The state mise reports for a cask requested at `requested` (`latest` or a version), desired
/// `present` or `absent`, and installed at `installed`.
fn cask_state(requested: &str, desired: &str, installed: Option<&str>) -> &'static str {
    match (installed, desired == "absent") {
        (Some(_), true) => "unexpectedly_installed",
        (None, true) => "absent",
        (None, false) => "missing",
        (Some(version), false) if requested != "latest" && requested != version => {
            "version_mismatch"
        }
        (Some(_), false) => "installed",
    }
}

fn unreadable(path: &Path, problem: &io::Error) -> Failure {
    Failure::precondition(format!(
        "cannot read Homebrew's Caskroom at {}: {problem}",
        path.display()
    ))
    .with_details(ErrorDetails { path: Some(path.to_path_buf()), ..Default::default() })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::ErrorKind;

    fn caskroom(versions: &[(&str, &[&str])]) -> (tempfile::TempDir, Caskroom) {
        let root = tempfile::tempdir().unwrap();
        for (token, entries) in versions {
            for entry in *entries {
                fs::create_dir_all(root.path().join(token).join(entry)).unwrap();
            }
        }
        let caskroom = Caskroom::at(root.path().to_path_buf());
        (root, caskroom)
    }

    fn declared(key: &str, requested: &str, desired: &str) -> PackageState {
        PackageState {
            key: key.into(),
            manager: "brew-cask".into(),
            requested: requested.into(),
            desired: desired.into(),
            state: "skipped".into(),
            installed: None,
            name: None,
            available: false,
        }
    }

    #[test]
    fn the_one_version_directory_is_the_installed_version() {
        let (_root, caskroom) = caskroom(&[("firefox", &["160.0", ".metadata", ".mise-tmp-1a2b"])]);
        assert_eq!(caskroom.installed_version("firefox").unwrap().as_deref(), Some("160.0"));
    }

    #[test]
    fn a_tap_cask_is_found_by_its_token() {
        let (_root, caskroom) = caskroom(&[("codexbar", &["0.18.0"])]);
        assert_eq!(
            caskroom.installed_version("steipete/tap/codexbar").unwrap().as_deref(),
            Some("0.18.0")
        );
    }

    #[test]
    fn no_directory_no_version_or_several_versions_is_not_installed() {
        let (root, caskroom) = caskroom(&[("iina", &["1.4.0", "1.4.1"]), ("vlc", &[".metadata"])]);
        fs::write(root.path().join("vlc/3.0.21"), "").unwrap();
        assert_eq!(caskroom.installed_version("iina").unwrap(), None);
        assert_eq!(caskroom.installed_version("vlc").unwrap(), None, "a file is not a version");
        assert_eq!(caskroom.installed_version("zed").unwrap(), None);
    }

    #[test]
    fn an_unreadable_cask_directory_fails_and_names_it() {
        let (root, caskroom) = caskroom(&[]);
        fs::write(root.path().join("firefox"), "not a directory").unwrap();
        let error = caskroom.installed_version("firefox").unwrap_err();
        assert_eq!(error.kind, ErrorKind::PreconditionFailed);
        assert_eq!(error.details.path, Some(root.path().join("firefox")));
    }

    #[test]
    fn a_cask_reports_the_states_mise_reports() {
        assert_eq!(cask_state("latest", "present", Some("160.0")), "installed");
        assert_eq!(cask_state("latest", "present", None), "missing");
        assert_eq!(cask_state("160.0", "present", Some("160.0")), "installed");
        assert_eq!(cask_state("159.0", "present", Some("160.0")), "version_mismatch");
        assert_eq!(cask_state("latest", "absent", Some("160.0")), "unexpectedly_installed");
        assert_eq!(cask_state("latest", "absent", None), "absent");
    }

    #[test]
    fn filling_a_cask_keeps_what_mise_declared() {
        let (_root, caskroom) = caskroom(&[("firefox", &["160.0"])]);
        let mut state = declared("brew-cask:firefox", "latest", "present");
        caskroom.fill(&mut state).unwrap();
        assert_eq!(state.key, "brew-cask:firefox");
        assert_eq!(state.requested, "latest");
        assert_eq!(state.state, "installed");
        assert_eq!(state.installed.as_deref(), Some("160.0"));
        assert!(state.available);

        let mut missing = declared("brew-cask:iina", "latest", "present");
        caskroom.fill(&mut missing).unwrap();
        assert_eq!((missing.state.as_str(), missing.installed), ("missing", None));
    }

    #[test]
    #[cfg(unix)]
    fn the_prefix_comes_from_the_child_environment() {
        let environment: BTreeMap<OsString, OsString> =
            [("MISE_SYSTEM_BREW_PREFIX".into(), "/tmp/brew".into())].into();
        let caskroom = Caskroom::of(&environment).unwrap();
        assert_eq!(caskroom.directory, Path::new("/tmp/brew/Caskroom"));
        let empty: BTreeMap<OsString, OsString> =
            [("MISE_SYSTEM_BREW_PREFIX".into(), "".into())].into();
        let default = if cfg!(target_os = "macos") {
            "/opt/homebrew/Caskroom"
        } else {
            "/home/linuxbrew/.linuxbrew/Caskroom"
        };
        assert_eq!(Caskroom::of(&empty).unwrap().directory, Path::new(default));
    }
}
