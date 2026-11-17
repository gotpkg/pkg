//! Running mise: every package change and most queries go through it.
//!
//! pkg runs `mise -C <home> ...`, so the global config decides what is declared, never a
//! project's mise.toml in the current directory.

mod plans;
mod queries;

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::json;

use crate::environment::MISE_VARIABLE;
use crate::failure::{ErrorCode, Failure};

pub use plans::{PlannedUpgrade, UpgradePlan, removals, upgrade_plan};
pub use queries::{ConfigTarget, OutdatedTool, PackageState, RegistryTool, ToolInfo, ToolState};

pub struct Mise {
    program: OsString,
    home: PathBuf,
}

/// What a finished mise command wrote.
pub struct Output {
    pub success: bool,
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// stdout then stderr, for parsers that look for lines wherever mise printed them.
    pub fn combined(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

impl Mise {
    pub fn new(program: OsString, home: PathBuf) -> Mise {
        Mise { program, home }
    }

    /// The executable pkg runs, as configured.
    pub fn program(&self) -> String {
        self.program.to_string_lossy().into_owned()
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(&self.program);
        command.arg("-C").arg(&self.home).args(args).stdin(Stdio::null());
        command
    }

    /// Runs a query and returns its stdout.
    ///
    /// # Errors
    pub fn read(&self, args: &[&str]) -> Result<String, Failure> {
        self.query(args).map(|output| output.stdout)
    }

    /// Runs a query and returns everything it wrote.
    ///
    /// # Errors
    ///
    /// `PRECONDITION_FAILED` when mise cannot be started, and `MISE_FAILED` when it exits with a
    /// failure.
    pub fn query(&self, args: &[&str]) -> Result<Output, Failure> {
        let output = self.capture(args)?;
        if output.success { Ok(output) } else { Err(self.failure(args, &output, true)) }
    }

    /// Runs a query and returns what it wrote, whether or not it succeeded.
    ///
    /// # Errors
    ///
    /// `PRECONDITION_FAILED` when mise cannot be started.
    pub fn capture(&self, args: &[&str]) -> Result<Output, Failure> {
        let output = self
            .command(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| self.spawn_failure(&error))?;
        Ok(Output {
            success: output.status.success(),
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// Runs a change, passing mise's progress through to stderr as it arrives, and returns its
    /// output.
    ///
    /// # Errors
    ///
    /// `PRECONDITION_FAILED` when mise cannot be started, and `MISE_FAILED` when it exits with a
    /// failure.
    pub fn run(&self, args: &[&str]) -> Result<Output, Failure> {
        let mut child = self
            .command(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| self.spawn_failure(&error))?;
        let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
        let (stdout, stderr) = std::thread::scope(|scope| {
            let stdout = scope.spawn(|| stdout.map(forward).unwrap_or_default());
            let stderr = scope.spawn(|| stderr.map(forward).unwrap_or_default());
            (stdout.join().unwrap_or_default(), stderr.join().unwrap_or_default())
        });
        let status = child.wait().map_err(|error| self.spawn_failure(&error))?;
        let output = Output { success: status.success(), status: status.code(), stdout, stderr };
        if output.success { Ok(output) } else { Err(self.failure(args, &output, false)) }
    }

    fn spawn_failure(&self, error: &std::io::Error) -> Failure {
        let program = self.program();
        let failure = if error.kind() == std::io::ErrorKind::NotFound {
            Failure::precondition(format!("cannot find mise (`{program}`); pkg needs it on PATH"))
                .suggest("Install mise: curl https://mise.run | sh")
                .suggest(format!("Or point {MISE_VARIABLE} at the mise executable"))
        } else {
            Failure::precondition(format!("cannot run mise (`{program}`): {error}"))
        };
        failure.with_details(json!({ "program": program, "variable": MISE_VARIABLE }))
    }

    /// The failure for a mise command that exited unsuccessfully: its first error line, and, for
    /// a query whose output nobody saw, the end of that output.
    fn failure(&self, args: &[&str], output: &Output, show_output: bool) -> Failure {
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
                format!("`mise {}` exited with {status}", args.join(" "))
            });
        let shown: Vec<&str> = if show_output {
            lines.iter().rev().take(15).rev().copied().collect()
        } else {
            Vec::new()
        };
        Failure::new(ErrorCode::MiseFailed, message).with_details(json!({
            "command": format!("mise -C {} {}", self.home.display(), args.join(" ")),
            "status": output.status,
            "output": shown,
        }))
    }
}

/// mise's footer on every error, which says nothing about this one.
fn is_boilerplate(line: &str) -> bool {
    let line = line.trim();
    line.starts_with("mise ERROR Version:") || line.starts_with("mise ERROR Run with --verbose")
}

/// Copies `stream` to stderr line by line and returns everything it carried.
fn forward(stream: impl Read) -> String {
    let mut collected = String::new();
    for line in BufReader::new(stream).lines().map_while(Result::ok) {
        eprintln!("{line}");
        collected.push_str(&line);
        collected.push('\n');
    }
    collected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_carries_mises_error_line_without_its_footer() {
        let mise = Mise::new("mise".into(), PathBuf::from("/home/mark"));
        let output = Output {
            success: false,
            status: Some(1),
            stdout: String::new(),
            stderr: "mise ERROR unknown bootstrap package manager 'foo'\nmise ERROR Version: 2026.11.9\nmise ERROR Run with --verbose or MISE_VERBOSE=1 for more information\n".into(),
        };
        let failure = mise.failure(&["bootstrap", "packages", "use"], &output, true);
        assert_eq!(failure.code, ErrorCode::MiseFailed);
        assert_eq!(failure.message, "unknown bootstrap package manager 'foo'");
        assert_eq!(
            failure.details["output"],
            json!(["mise ERROR unknown bootstrap package manager 'foo'"])
        );
    }
}
