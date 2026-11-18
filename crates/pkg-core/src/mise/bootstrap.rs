//! Running `mise bootstrap`, which may need the person at the terminal.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, Stdio};

use super::{Mise, Output};
use crate::{Error, Progress};

/// How `mise bootstrap` runs: from which directory, with which extra environment, and whether it
/// may use the caller's terminal.
pub(crate) struct BootstrapRun<'a> {
    pub directory: &'a Path,
    pub env: Vec<(&'static str, OsString)>,
    pub terminal: bool,
}

impl Mise<'_> {
    /// Runs `mise -C <directory> <args>`. With `terminal`, mise reads the caller's stdin and writes
    /// to its stderr, so it and the programs it runs can ask for passwords and answers; nothing is
    /// captured and the result is `None`. Otherwise its output goes to `progress` and is returned.
    ///
    /// # Errors
    ///
    /// `PRECONDITION_FAILED` when mise cannot start, `MISE_FAILED` when it fails.
    pub(crate) fn bootstrap(
        &self,
        args: &[OsString],
        run: &BootstrapRun<'_>,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Option<Output>, Error> {
        let mut command = self.command_in(run.directory, args);
        command.envs(run.env.iter().map(|(key, value)| (OsStr::new(key), value)));
        if !run.terminal {
            without_terminal(&mut command);
            return self.run_command(command, args, progress).map(Some);
        }
        command.stdin(Stdio::inherit()).stdout(terminal_stdout()?).stderr(Stdio::inherit());
        let status = command.status().map_err(|error| self.spawn_failure(&error))?;
        if status.success() {
            return Ok(None);
        }
        let output = Output {
            success: false,
            status: status.code(),
            stdout: String::new(),
            stderr: String::new(),
        };
        let mut error = self.failure(args, &output, false);
        error.details.output_streamed = true;
        Err(error)
    }
}

/// The caller's stderr, which takes mise's stdout too so that the caller's stdout keeps only
/// its own result.
fn terminal_stdout() -> Result<Stdio, Error> {
    #[cfg(unix)]
    {
        use std::os::fd::AsFd;
        let stderr = std::io::stderr().as_fd().try_clone_to_owned().map_err(|error| {
            let message = format!("cannot share the terminal with mise: {error}");
            Error::new(crate::ErrorKind::Internal, message)
        })?;
        Ok(Stdio::from(stderr))
    }
    #[cfg(not(unix))]
    {
        Ok(Stdio::inherit())
    }
}

/// Starts mise in a session of its own, without a controlling terminal, so nothing it runs (sudo,
/// git, ssh) can open `/dev/tty` and wait for an answer no one is there to give.
#[cfg(unix)]
fn without_terminal(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: the closure runs between fork and exec, and calls only `setsid`, which is
    // async-signal-safe, and `last_os_error`, which reads errno without allocating.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn without_terminal(_command: &mut Command) {}
