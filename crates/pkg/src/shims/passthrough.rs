//! Running the real tool behind a shim.

use std::convert::Infallible;
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use super::directory::read_start;
use super::launcher;
use super::path;
use crate::failure::Failure;

/// `mise` subcommands that run the caller's own commands, so those commands keep the shims.
const RUNNING_SUBCOMMANDS: [&str; 6] = ["exec", "x", "run", "r", "watch", "w"];

/// mise's global options that take a separate value.
const MISE_GLOBAL_VALUES: [&str; 8] =
    ["-C", "--cd", "-E", "--env", "-j", "--jobs", "--output", "--log-level"];

/// Whether the real `tool` runs without the shim directory on `PATH`: mise does, so the package
/// managers it runs by name are the real ones, except when it runs the caller's own command.
fn hides_shims(tool: &str, args: &[OsString]) -> bool {
    if tool != "mise" {
        return false;
    }
    let words: Vec<String> = args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
    let mut rest = words.iter().map(String::as_str);
    while let Some(argument) = rest.next() {
        if MISE_GLOBAL_VALUES.contains(&argument) {
            rest.next();
        } else if !argument.starts_with('-') {
            return !RUNNING_SUBCOMMANDS.contains(&argument);
        }
    }
    true
}

/// Runs the first executable called `tool` on `search_path` that is not a shim, with `args`, and
/// never returns when it runs: the tool's status is the shim's. With no such executable, says so
/// and exits 127.
///
/// # Errors
///
/// When the tool is found but cannot be started.
pub(crate) fn run(
    tool: &str,
    args: &[OsString],
    shim_directory: Option<&Path>,
    search_path: Option<OsString>,
    pathext: Option<OsString>,
) -> Result<Infallible, Failure> {
    let directories = search_path.as_deref().map(path::directories).unwrap_or_default();
    let extensions = path::executable_extensions(pathext.as_deref());
    let is_a_shim = |candidate: &Path| {
        let in_shim_directory = shim_directory.is_some_and(|directory| {
            candidate.parent().is_some_and(|parent| path::same_directory(parent, directory))
        });
        in_shim_directory || read_start(candidate).is_some_and(|start| launcher::is_shim(&start))
    };
    let Some(real) = path::find_executable(&directories, tool, &extensions, is_a_shim) else {
        eprintln!("{}", not_installed(tool));
        std::process::exit(127);
    };

    let mut command = Command::new(&real);
    command.args(args);
    if let (true, Some(search_path), Some(directory)) =
        (hides_shims(tool, args), search_path.as_deref(), shim_directory)
    {
        command.env("PATH", path::without(search_path, directory));
    }
    launch(&mut command, &real)
}

fn not_installed(tool: &str) -> String {
    if tool == "brew" {
        "brew: Homebrew is not installed on this machine; mise provides its packages. \
         Find one with `pkg search <words>` and install it with `pkg install brew:<name>`."
            .to_string()
    } else {
        format!(
            "{tool}: not installed on this machine. Find software with `pkg search <words>`; \
             `pkg --help` shows what pkg can do."
        )
    }
}

#[cfg(unix)]
fn launch(command: &mut Command, real: &Path) -> Result<Infallible, Failure> {
    use std::os::unix::process::CommandExt;
    let error = command.exec();
    Err(Failure::internal(format!("cannot run {}: {error}", real.display())))
}

#[cfg(not(unix))]
fn launch(command: &mut Command, real: &Path) -> Result<Infallible, Failure> {
    match command.status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => Err(Failure::internal(format!("cannot run {}: {error}", real.display()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(words: &str) -> Vec<OsString> {
        words.split_whitespace().map(OsString::from).collect()
    }

    #[test]
    fn mise_hides_the_shims_from_the_managers_it_runs() {
        for words in ["bootstrap packages apply", "install node@24", "use ripgrep", "--version", ""]
        {
            assert!(hides_shims("mise", &arguments(words)), "mise {words}");
        }
    }

    #[test]
    fn mise_keeps_the_shims_for_the_commands_it_runs_for_the_caller() {
        for words in [
            "exec -- npm i -g x",
            "x -- ls",
            "run build",
            "-C /project run build",
            "--env ci watch",
        ] {
            assert!(!hides_shims("mise", &arguments(words)), "mise {words}");
        }
    }

    #[test]
    fn a_tool_named_like_a_running_subcommand_is_not_one() {
        assert!(hides_shims("mise", &arguments("use -g r")));
    }

    #[test]
    fn other_tools_keep_the_path_they_were_given() {
        assert!(!hides_shims("npm", &arguments("install")));
    }
}
