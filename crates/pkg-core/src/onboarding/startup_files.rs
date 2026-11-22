//! Moving the installer's startup files aside while a configuration deploys its own, and putting
//! back the ones it did not.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Serialize;

use super::mise::cannot_write;
use super::shims::write;
use crate::startup::{candidate_files, is_only_block};
use crate::{Error, Settings};

/// Startup files that held nothing but pkg's `PATH` block.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartupFiles {
    /// The files pkg deleted before mise applied the configuration (for `dry-run`: would delete).
    pub moved_aside: Vec<PathBuf>,
    /// The ones pkg wrote back because the configuration deployed nothing there.
    pub restored: Vec<PathBuf>,
}

/// The files moved aside and what each held.
pub(crate) struct MovedAside(Vec<(PathBuf, String)>);

/// Every candidate startup file that is a plain file holding only pkg's block.
pub(crate) fn installers_files(settings: &Settings) -> Vec<(PathBuf, String)> {
    candidate_files(&settings.home, &settings.child_env)
        .into_iter()
        .filter(|file| file.symlink_metadata().is_ok_and(|metadata| metadata.is_file()))
        .filter_map(|file| {
            let contents = std::fs::read_to_string(&file).ok()?;
            is_only_block(&contents).then_some((file, contents))
        })
        .collect()
}

/// Deletes the installer's startup files, so a configuration can deploy its own there.
pub(crate) fn move_aside(files: Vec<(PathBuf, String)>) -> Result<MovedAside, Error> {
    let mut moved = Vec::new();
    for (file, contents) in files {
        if let Err(error) = std::fs::remove_file(&file) {
            restore(MovedAside(moved))?;
            return Err(cannot_write(&file, &error));
        }
        moved.push((file, contents));
    }
    Ok(MovedAside(moved))
}

/// Writes back each moved file the configuration left absent, and reports both lists.
pub(crate) fn restore(moved: MovedAside) -> Result<StartupFiles, Error> {
    let mut report = StartupFiles::default();
    for (file, contents) in moved.0 {
        if file.symlink_metadata().is_err() {
            write(&file, &contents)?;
            report.restored.push(file.clone());
        }
        report.moved_aside.push(file);
    }
    Ok(report)
}
