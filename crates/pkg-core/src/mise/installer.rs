//! Running mise's own installer script, which puts mise where `MISE_INSTALL_PATH` names.

use std::path::Path;
use std::process::{Command, Stdio};

use super::streams;
use crate::error::{Error, ErrorDetails, ErrorKind};
use crate::{Progress, Settings};

/// Runs the installer script at `script` with `sh`, in the home directory and the caller's child
/// environment, so that it installs mise at `target`. Its output is progress.
///
/// # Errors
///
/// `PRECONDITION_FAILED` when `sh` cannot start, `MISE_FAILED` when the installer fails.
pub(crate) fn run_installer(
    settings: &Settings,
    script: &Path,
    target: &Path,
    progress: &mut dyn FnMut(Progress<'_>),
) -> Result<(), Error> {
    let mut child = Command::new("sh")
        .arg(script)
        .current_dir(&settings.home)
        .env_clear()
        .envs(&settings.child_env)
        .env("HOME", &settings.home)
        .env("MISE_INSTALL_PATH", target)
        // pkg reports where mise went and how to reach it; the installer's own hint would repeat it.
        .env("MISE_INSTALL_HELP", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            Error::precondition(format!("cannot run sh for mise's installer: {error}"))
        })?;
    let collected = streams::collect(child.stdout.take(), child.stderr.take(), progress);
    let status = child
        .wait()
        .map_err(|error| Error::precondition(format!("cannot run mise's installer: {error}")))?;
    let (_, stderr) = collected.map_err(|error| {
        Error::new(ErrorKind::MiseFailed, format!("cannot read mise's installer output: {error}"))
    })?;
    if status.success() {
        return Ok(());
    }
    let last: Vec<String> =
        stderr.lines().filter(|line| !line.trim().is_empty()).map(str::to_string).collect();
    let mut error = Error::new(
        ErrorKind::MiseFailed,
        format!("mise's installer failed: {}", last.last().map_or("no output", String::as_str)),
    )
    .with_details(ErrorDetails {
        url: Some(settings.mise_installer.clone()),
        status: Some(status.code()),
        output: Some(last.iter().rev().take(15).rev().cloned().collect()),
        ..Default::default()
    });
    error.details.output_streamed = true;
    Err(error)
}
