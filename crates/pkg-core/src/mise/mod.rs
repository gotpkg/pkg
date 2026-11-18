//! mise execution within a caller-supplied home and child environment.

mod plans;
mod queries;
mod release;
mod streams;

use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::{Error, ErrorDetails, ErrorKind, Progress, Recovery, Settings};

pub(crate) use plans::{removals, upgrade_plan};
pub(crate) use queries::{ConfigTarget, OutdatedTool, PackageState, ToolState};
pub(crate) use release::{MISE, MiseRelease, PACKAGE_MANAGER_REASON};

pub(crate) struct Mise<'a> {
    settings: &'a Settings,
    home: &'a Path,
}

pub(crate) struct Output {
    pub success: bool,
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub(crate) fn combined(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

impl<'a> Mise<'a> {
    pub(crate) fn new(settings: &'a Settings) -> Self {
        Self { settings, home: &settings.home }
    }

    pub(crate) fn program(&self) -> String {
        self.settings.mise.to_string_lossy().into_owned()
    }

    fn command(&self, args: &[impl AsRef<OsStr>]) -> Command {
        let mut command = Command::new(&self.settings.mise);
        command
            .arg("-C")
            .arg(self.home)
            .args(args)
            .current_dir(self.home)
            .env_clear()
            .envs(&self.settings.child_env)
            .env("HOME", self.home)
            .env("USERPROFILE", self.home)
            .env("MISE_CONFIG_DIR", &self.settings.config_dir)
            .env_remove("MISE_GLOBAL_CONFIG_FILE")
            .stdin(Stdio::null());
        if let Some(path) = &self.settings.global_config_override {
            command.env("MISE_GLOBAL_CONFIG_FILE", path);
        }
        command
    }

    pub(crate) fn read(&self, args: &[&str]) -> Result<String, Error> {
        self.query(args).map(|output| output.stdout)
    }

    pub(crate) fn query(&self, args: &[&str]) -> Result<Output, Error> {
        let output = self.capture(args)?;
        if output.success { Ok(output) } else { Err(self.failure(args, &output, true)) }
    }

    pub(crate) fn capture(&self, args: &[&str]) -> Result<Output, Error> {
        let output = self
            .command(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| self.spawn_failure(&error))?;
        Ok(Output {
            success: output.status.success(),
            status: output.status.code(),
            stdout: decode(output.stdout),
            stderr: decode(output.stderr),
        })
    }

    pub(crate) fn run(
        &self,
        args: &[impl AsRef<OsStr>],
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Output, Error> {
        let mut child = self
            .command(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| self.spawn_failure(&error))?;
        let collected = streams::collect(child.stdout.take(), child.stderr.take(), progress);
        // Reap even when reading a pipe failed; a call never leaves its child behind.
        let status = child.wait().map_err(|error| self.spawn_failure(&error))?;
        let (stdout, stderr) = collected.map_err(|error| {
            Error::new(ErrorKind::MiseFailed, format!("cannot read mise output: {error}"))
        })?;
        let output = Output { success: status.success(), status: status.code(), stdout, stderr };
        if output.success {
            Ok(output)
        } else {
            let mut error = self.failure(args, &output, true);
            error.details.output_streamed = true;
            Err(error)
        }
    }

    fn spawn_failure(&self, error: &io::Error) -> Error {
        let program = self.program();
        let failure = if error.kind() == io::ErrorKind::NotFound {
            Error::precondition(format!("cannot find mise (`{program}`); pkg needs it on PATH"))
                .recover(Recovery::InstallMise)
                .recover(Recovery::SelectMise)
        } else {
            Error::precondition(format!("cannot run mise (`{program}`): {error}"))
        };
        failure.with_details(ErrorDetails {
            program: Some(self.settings.mise.clone().into()),
            ..Default::default()
        })
    }

    fn failure(&self, args: &[impl AsRef<OsStr>], output: &Output, show_output: bool) -> Error {
        let arguments = display_args(args);
        let lines: Vec<&str> = output
            .stderr
            .lines()
            .filter(|line| !line.trim().is_empty() && !is_boilerplate(line))
            .collect();
        let message = lines
            .iter()
            .find_map(|line| line.trim().strip_prefix("mise ERROR "))
            .map(str::to_string)
            .unwrap_or_else(|| {
                let status = output.status.map_or("a signal".to_string(), |code| code.to_string());
                format!("`mise {arguments}` exited with {status}")
            });
        let shown = if show_output {
            lines.iter().rev().take(15).rev().map(|line| (*line).to_string()).collect()
        } else {
            Vec::new()
        };
        Error::new(ErrorKind::MiseFailed, message).with_details(ErrorDetails {
            command: Some(format!("mise -C {} {arguments}", self.home.display())),
            status: Some(output.status),
            output: Some(shown),
            ..Default::default()
        })
    }
}

fn display_args(args: &[impl AsRef<OsStr>]) -> String {
    let mut text = String::new();
    for (index, argument) in args.iter().enumerate() {
        if index > 0 {
            text.push(' ');
        }
        text.push_str(&argument.as_ref().to_string_lossy());
    }
    text
}

fn is_boilerplate(line: &str) -> bool {
    let line = line.trim();
    line.starts_with("mise ERROR Version:") || line.starts_with("mise ERROR Run with --verbose")
}

fn decode(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_carries_mises_error_line_without_its_footer() {
        let settings = Settings::new("/home/mark".into(), "mise".into());
        let mise = Mise::new(&settings);
        let output = Output {
            success: false,
            status: Some(1),
            stdout: String::new(),
            stderr: "mise ERROR unknown bootstrap package manager 'foo'\nmise ERROR Version: 2026.11.9\nmise ERROR Run with --verbose or MISE_VERBOSE=1 for more information\n".into(),
        };
        let failure = mise.failure(&["bootstrap", "packages", "use"], &output, true);
        assert_eq!(failure.kind, ErrorKind::MiseFailed);
        assert_eq!(failure.message, "unknown bootstrap package manager 'foo'");
        assert_eq!(
            failure.details.output,
            Some(vec!["mise ERROR unknown bootstrap package manager 'foo'".into()])
        );
    }
}
