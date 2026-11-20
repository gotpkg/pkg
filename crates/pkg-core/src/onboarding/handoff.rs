//! Handing pkg to mise: declaring `github:gotpkg/pkg` like any tool and replacing the installer's
//! copy in `~/.local/bin` with a link to mise's shim, so mise updates pkg from then on.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use schemars::JsonSchema;
use serde::Serialize;

use super::mise::cannot_write;
use crate::common::Machine;
use crate::outdated::is_newer;
use crate::{Client, Error, InstallAction, InstallRequest, Mode, Progress, Settings, find_on_path};

/// The tool mise installs pkg as.
pub(crate) const PKG_TOOL: &str = "github:gotpkg/pkg";

/// What happened when pkg was handed to mise.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Handoff {
    /// `added` when pkg declared `github:gotpkg/pkg`, `existing` when a config already did.
    pub declaration: Declaration,
    /// The config file that declares it, or for `dry-run` the one it would go to; null when mise
    /// is not installed yet.
    pub config: Option<PathBuf>,
    /// The version of pkg mise runs; null when it is not known.
    pub version: Option<String>,
    /// What happened to `~/.local/bin/pkg`: `linked` (replaced by a link to mise's shim),
    /// `already-linked`, `kept` (see `reason`), or `none` (nothing there).
    pub link: Link,
    /// `~/.local/bin/pkg`.
    pub path: PathBuf,
    /// Why `~/.local/bin/pkg` was kept.
    pub reason: Option<String>,
    /// Other pkg executables on `PATH`, which a shell may run instead.
    pub others: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Declaration {
    Added,
    Existing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Link {
    Linked,
    AlreadyLinked,
    Kept,
    None,
}

/// Who put a pkg executable where it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PkgOwner {
    /// The installer's copy in `~/.local/bin`, which rerunning the installer updates.
    Installer,
    /// mise, which `pkg update` has update it.
    Mise,
    /// Something else, such as Homebrew or a build from a checkout.
    Other,
}

/// Who put the pkg executable at `executable`: mise when it lives in mise's data directory, the
/// installer when it is the file at `~/.local/bin/pkg`, else something else.
pub fn pkg_owner(settings: &Settings, executable: &Path) -> PkgOwner {
    let resolved = std::fs::canonicalize(executable).unwrap_or_else(|_| executable.to_path_buf());
    let data_dir = std::fs::canonicalize(settings.mise_data_dir());
    if data_dir.is_ok_and(|data_dir| resolved.starts_with(data_dir)) {
        return PkgOwner::Mise;
    }
    let installer = settings.local_bin().join("pkg");
    let installers = installer.symlink_metadata().is_ok_and(|metadata| metadata.is_file());
    if installers && std::fs::canonicalize(&installer).is_ok_and(|installer| installer == resolved)
    {
        return PkgOwner::Installer;
    }
    PkgOwner::Other
}

/// What is at `~/.local/bin/pkg`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InstallerCopy {
    Missing,
    LinkToShim,
    OtherLink,
    Pkg(String),
    NotPkg,
}

/// What to do with `~/.local/bin/pkg`, given mise's pkg version (none when it does not run) and
/// the running pkg's.
pub(crate) fn decide(
    copy: &InstallerCopy,
    mise_pkg: Option<&str>,
    running: &str,
) -> (Link, Option<String>) {
    let keep = |reason: String| (Link::Kept, Some(reason));
    match copy {
        InstallerCopy::Missing => (Link::None, None),
        InstallerCopy::LinkToShim => (Link::AlreadyLinked, None),
        InstallerCopy::OtherLink => keep("it is a link pkg did not make".into()),
        InstallerCopy::NotPkg => keep("it is not pkg".into()),
        InstallerCopy::Pkg(_) => match mise_pkg {
            None => keep("mise's pkg did not run".into()),
            Some(version) if is_newer(running, version) => keep(format!(
                "mise has pkg {version}, older than this pkg {running}; `pkg update` links it \
                 once mise has a newer one"
            )),
            Some(_) => (Link::Linked, None),
        },
    }
}

/// Declares and installs pkg with mise unless a config already declares it, then links
/// `~/.local/bin/pkg` to mise's shim. `mise_ready` is false in a dry run that would install mise,
/// when nothing can be asked of it.
pub(crate) fn hand_off(
    settings: &Settings,
    mode: Mode,
    running: &str,
    mise_ready: bool,
    progress: &mut dyn FnMut(Progress<'_>),
) -> Result<Handoff, Error> {
    let path = settings.local_bin().join("pkg");
    let shim = settings.mise_data_dir().join("shims/pkg");
    let copy = inspect(&path, &shim, settings);
    let (declared, config) = if mise_ready {
        let machine = Machine::load(settings)?;
        match machine.declared_in(&machine.package(PKG_TOOL)) {
            Some(file) => (true, Some(file.to_path_buf())),
            None => (false, Some(machine.target(true)?.path)),
        }
    } else {
        (false, None)
    };
    let mut handoff = Handoff {
        declaration: if declared { Declaration::Existing } else { Declaration::Added },
        config,
        version: None,
        link: Link::None,
        path: path.clone(),
        reason: None,
        others: others(settings, &shim),
    };
    if mode.is_dry_run() {
        (handoff.link, handoff.reason) = decide(&copy, Some(running), running);
        return Ok(handoff);
    }

    let mut client = Client::new(settings.clone())?;
    let installed =
        client.install(InstallRequest { packages: vec![PKG_TOOL.into()], mode }, progress)?;
    if let Some(item) = installed.result.packages.first() {
        handoff.config = Some(item.config.clone());
        if matches!(item.action, InstallAction::Added | InstallAction::Declared) {
            handoff.declaration = Declaration::Added;
        }
    }
    handoff.version = pkg_version(&shim, settings);
    (handoff.link, handoff.reason) = decide(&copy, handoff.version.as_deref(), running);
    if handoff.link == Link::Linked {
        link(&path, &shim)?;
    }
    Ok(handoff)
}

fn inspect(path: &Path, shim: &Path, settings: &Settings) -> InstallerCopy {
    let Ok(metadata) = path.symlink_metadata() else {
        return InstallerCopy::Missing;
    };
    if metadata.is_symlink() {
        return match std::fs::read_link(path) {
            Ok(target) if target == shim => InstallerCopy::LinkToShim,
            _ => InstallerCopy::OtherLink,
        };
    }
    pkg_version(path, settings).map_or(InstallerCopy::NotPkg, InstallerCopy::Pkg)
}

/// The version `<executable> --version` reports when it is pkg.
fn pkg_version(executable: &Path, settings: &Settings) -> Option<String> {
    let output = Command::new(executable)
        .arg("--version")
        .current_dir(&settings.home)
        .env_clear()
        .envs(&settings.child_env)
        .env("HOME", &settings.home)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let version = text.trim().strip_prefix("pkg ")?;
    (output.status.success() && !version.is_empty()).then(|| version.to_string())
}

/// Replaces `path` with a link to `shim`, atomically, through a link staged beside it under this
/// process's own name. It fails rather than remove anything already there.
fn link(path: &Path, shim: &Path) -> Result<(), Error> {
    let staged = path.with_file_name(format!(".pkg-link-{}", std::process::id()));
    #[cfg(unix)]
    std::os::unix::fs::symlink(shim, &staged).map_err(|error| cannot_write(&staged, &error))?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(shim, &staged)
        .map_err(|error| cannot_write(&staged, &error))?;
    std::fs::rename(&staged, path).map_err(|error| {
        let _ = std::fs::remove_file(&staged);
        cannot_write(path, &error)
    })
}

/// pkg executables on `PATH` other than `~/.local/bin/pkg` and mise's.
fn others(settings: &Settings, shim: &Path) -> Vec<PathBuf> {
    let Some(path) = settings.child_env.get(std::ffi::OsStr::new("PATH")) else {
        return Vec::new();
    };
    let local_bin = settings.local_bin();
    let data_dir = std::fs::canonicalize(settings.mise_data_dir()).ok();
    std::env::split_paths(path)
        .filter(|directory| *directory != local_bin && Some(directory.as_path()) != shim.parent())
        .filter_map(|directory| find_on_path(std::ffi::OsStr::new("pkg"), directory.as_os_str()))
        .filter(|found| {
            let resolved = std::fs::canonicalize(found).ok();
            !matches!((&resolved, &data_dir), (Some(found), Some(data)) if found.starts_with(data))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkg_never_replaces_the_installers_copy_with_an_older_release() {
        let copy = InstallerCopy::Pkg("0.4.0".into());
        assert_eq!(decide(&copy, Some("0.4.0"), "0.4.0"), (Link::Linked, None));
        assert_eq!(decide(&copy, Some("0.5.0"), "0.4.0"), (Link::Linked, None));
        let (link, reason) = decide(&copy, Some("0.3.9"), "0.4.0");
        assert_eq!(link, Link::Kept);
        assert!(reason.unwrap().contains("0.3.9"));
        assert_eq!(decide(&copy, None, "0.4.0").0, Link::Kept);
    }

    #[test]
    fn only_pkg_itself_is_replaced() {
        assert_eq!(decide(&InstallerCopy::NotPkg, Some("1.0.0"), "1.0.0").0, Link::Kept);
        assert_eq!(decide(&InstallerCopy::OtherLink, Some("1.0.0"), "1.0.0").0, Link::Kept);
        assert_eq!(decide(&InstallerCopy::LinkToShim, None, "1.0.0").0, Link::AlreadyLinked);
        assert_eq!(decide(&InstallerCopy::Missing, Some("1.0.0"), "1.0.0").0, Link::None);
    }

    #[test]
    fn the_owner_follows_where_the_executable_lives() {
        let home = tempfile::TempDir::new().unwrap();
        let settings = Settings::new(home.path().to_path_buf(), "mise".into());
        let installed = settings.mise_data_dir().join("installs/github-gotpkg-pkg/1.0.0/pkg");
        let copy = settings.local_bin().join("pkg");
        for file in [&installed, &copy] {
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, "").unwrap();
        }
        assert_eq!(pkg_owner(&settings, &installed), PkgOwner::Mise);
        assert_eq!(pkg_owner(&settings, &copy), PkgOwner::Installer);
        assert_eq!(pkg_owner(&settings, &home.path().join("elsewhere/pkg")), PkgOwner::Other);
    }
}
