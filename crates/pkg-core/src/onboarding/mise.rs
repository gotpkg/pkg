//! Finding a usable mise, or installing one with mise's own installer.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;

use crate::mise::{Mise, MiseRelease, run_installer};
use crate::outdated::is_newer;
use crate::{Error, ErrorDetails, Mode, Progress, Recovery, Settings, catalog};

/// The oldest mise pkg supports.
pub(crate) const MINIMUM_MISE: &str = "2026.11.9";

/// The mise `setup` and `bootstrap` use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct MiseSetup {
    /// The executable.
    pub program: PathBuf,
    /// Its version; null in a dry run that would install it.
    pub version: Option<String>,
    /// `found` when pkg used the mise it found, `installed` when it ran mise's installer (for
    /// `dry-run`: would run).
    pub action: MiseAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MiseAction {
    Found,
    Installed,
}

/// The settings to run the rest of the operation with, and what pkg did about mise. When mise is
/// missing and `settings` names it only by name, mise's installer puts it in `~/.local/bin`; a
/// missing executable named by path is an error, never a reason to install another.
pub(crate) fn ensure(
    settings: &Settings,
    mode: Mode,
    progress: &mut dyn FnMut(Progress<'_>),
) -> Result<(Settings, MiseSetup), Error> {
    let mise = Mise::new(settings);
    match mise.release() {
        Ok(release) => {
            let program = mise.executable().unwrap_or_else(|| PathBuf::from(&settings.mise));
            supported(&release, &program, mise.updates_itself())?;
            let found =
                MiseSetup { program, version: Some(release.version), action: MiseAction::Found };
            Ok((settings.clone(), found))
        }
        Err(error) if is_missing(&error) && is_bare(&settings.mise) => {
            install(settings, mode, progress)
        }
        Err(error) => Err(error),
    }
}

/// Whether mise could not be started because no such executable exists.
pub(crate) fn is_missing(error: &Error) -> bool {
    error.recovery.contains(&Recovery::InstallMise)
}

fn is_bare(program: &std::ffi::OsStr) -> bool {
    Path::new(program).components().count() == 1
}

fn install(
    settings: &Settings,
    mode: Mode,
    progress: &mut dyn FnMut(Progress<'_>),
) -> Result<(Settings, MiseSetup), Error> {
    let target = settings.local_bin().join("mise");
    let mut installed = settings.clone();
    installed.mise = target.clone().into_os_string();
    if mode.is_dry_run() {
        let plan = MiseSetup { program: target, version: None, action: MiseAction::Installed };
        return Ok((installed, plan));
    }
    let script = catalog::Client::new(&settings.child_env)?.get_text(&settings.mise_installer)?;
    std::fs::create_dir_all(&settings.cache_dir)
        .map_err(|error| cannot_write(&settings.cache_dir, &error))?;
    let path = settings.cache_dir.join("mise-installer.sh");
    std::fs::write(&path, script).map_err(|error| cannot_write(&path, &error))?;
    let ran = run_installer(settings, &path, &target, progress);
    let _ = std::fs::remove_file(&path);
    ran?;
    let mise = Mise::new(&installed);
    let release = mise.release()?;
    supported(&release, &target, true)?;
    Ok((
        installed,
        MiseSetup {
            program: target,
            version: Some(release.version),
            action: MiseAction::Installed,
        },
    ))
}

fn supported(release: &MiseRelease, program: &Path, updates_itself: bool) -> Result<(), Error> {
    if !is_newer(MINIMUM_MISE, &release.version) {
        return Ok(());
    }
    Err(Error::precondition(format!(
        "mise {} at {} is older than {MINIMUM_MISE}, the oldest pkg supports",
        release.version,
        program.display()
    ))
    .with_details(ErrorDetails { program: Some(program.to_path_buf()), ..Default::default() })
    .recover(if updates_itself {
        Recovery::UpdateMise
    } else {
        Recovery::UpdateMiseWithItsPackageManager
    }))
}

pub(crate) fn cannot_write(path: &Path, error: &std::io::Error) -> Error {
    Error::precondition(format!("cannot write {}: {error}", path.display()))
        .with_details(ErrorDetails { path: Some(path.to_path_buf()), ..Default::default() })
}
