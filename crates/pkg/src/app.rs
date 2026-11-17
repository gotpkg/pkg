//! One run of `pkg`: parse, dispatch, and report on the standard streams.

use std::ffi::OsString;
use std::io::{IsTerminal, Write};

use clap::ArgMatches;
use clap::error::{ContextKind, ContextValue, ErrorKind};
use serde_json::{Value, json};

use crate::NAME;
use crate::failure::{ErrorCode, Failure};
use crate::invocation::{Invocation, OutputFormat};
use crate::parser;
use crate::registry;
use crate::response::Response;

/// Runs `pkg` with `args` (including the program name) and returns the exit status.
pub fn run(args: &[OsString]) -> u8 {
    let output = requested_output(args);
    let mut command = parser::build();
    if args.len() <= 1 {
        return printed(command.print_help());
    }

    let matches = match command.try_get_matches_from_mut(args) {
        Ok(matches) => matches,
        Err(error) => match (error.kind(), output) {
            (ErrorKind::DisplayHelp | ErrorKind::DisplayVersion, _) => {
                return printed(error.print());
            }
            // Clap's own message names the problem and the usage, which is what a person needs.
            (_, OutputFormat::Text) => {
                let _ = error.print();
                return ErrorCode::InvalidArgument.exit_status();
            }
            (_, OutputFormat::Json) => {
                return report(output, None, Err(parse_failure(&error, args)));
            }
        },
    };

    let (path, leaf) = leaf_matches(&matches);
    let words: Vec<&str> = path.iter().map(String::as_str).collect();
    let Some(spec) = registry::find(&words) else {
        let failure = Failure::internal(format!("no registry entry for `{}`", words.join(" ")));
        return report(output, None, Err(failure));
    };
    let invocation = Invocation::new(spec, leaf, output);
    report(output, Some(spec.id), (spec.run)(&invocation))
}

/// Reports a failure that happened before `run` could start and returns its exit status.
pub fn fail_early(failure: &Failure) -> u8 {
    eprint!("{}", failure.to_text());
    failure.code.exit_status()
}

fn leaf_matches(matches: &ArgMatches) -> (Vec<String>, &ArgMatches) {
    let mut path = Vec::new();
    let mut current = matches;
    while let Some((name, sub)) = current.subcommand() {
        path.push(name.to_string());
        current = sub;
    }
    (path, current)
}

/// `--output` as written, read before parsing so a parse error can honor it.
fn requested_output(args: &[OsString]) -> OutputFormat {
    let words: Vec<String> = args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
    for (index, word) in words.iter().enumerate() {
        let value = if word == "--output" {
            words.get(index + 1).map(String::as_str)
        } else {
            word.strip_prefix("--output=")
        };
        if value == Some("json") {
            return OutputFormat::Json;
        }
    }
    OutputFormat::Text
}

fn parse_failure(error: &clap::Error, args: &[OsString]) -> Failure {
    let message = error
        .to_string()
        .lines()
        .next()
        .unwrap_or("invalid arguments")
        .trim_start_matches("error: ")
        .to_string();
    let mut details = serde_json::Map::new();
    if let Some(ContextValue::String(argument)) = error.get(ContextKind::InvalidArg) {
        details.insert("argument".into(), json!(argument));
    }
    if let Some(ContextValue::Strings(arguments)) = error.get(ContextKind::InvalidArg) {
        details.insert("missing".into(), json!(arguments));
    }
    if let Some(ContextValue::String(value)) = error.get(ContextKind::InvalidValue) {
        details.insert("value".into(), json!(value));
    }
    if let Some(ContextValue::Strings(values)) = error.get(ContextKind::ValidValue) {
        details.insert("acceptedValues".into(), json!(values));
    }
    if let Some(ContextValue::Strings(values)) = error.get(ContextKind::ValidSubcommand) {
        details.insert("validSubcommands".into(), json!(values));
    }

    let words: Vec<String> = args
        .iter()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .take_while(|word| !word.starts_with('-'))
        .collect();
    let mut failure = Failure::invalid_argument(message);
    if let Some(spec) = longest_known_command(&words) {
        details.insert("command".into(), json!(spec.command_line()));
        details.insert(
            "arguments".into(),
            json!(spec.all_arguments().map(registry::ArgumentSpec::describe).collect::<Vec<_>>()),
        );
        // `info --package brew:wget`: the flag names an argument that is positional here.
        if let Some(ContextValue::String(flag)) = error.get(ContextKind::InvalidArg)
            && let Some(positional) = spec.arguments.iter().find(|argument| {
                argument.kind == registry::ArgumentKind::Positional
                    && flag.trim_start_matches('-').split('=').next() == Some(argument.name)
            })
        {
            failure = failure.suggest(format!(
                "Pass the {name} without a flag: {} <{name}>",
                spec.command_line(),
                name = positional.name
            ));
        }
        failure = failure.suggest(format!("{NAME} schema {}", spec.path.join(" ")));
    } else {
        let operations: Vec<String> = registry::all()
            .into_iter()
            .filter(|command| words.is_empty() || command.path[0] == words[0])
            .map(registry::CommandSpec::command_line)
            .collect();
        let operations = if operations.is_empty() {
            registry::all().into_iter().map(registry::CommandSpec::command_line).collect()
        } else {
            operations
        };
        details.insert("commands".into(), json!(operations));
        failure = failure.suggest(format!("{NAME} cli search \"<what you want to do>\""));
    }
    if error.kind() == ErrorKind::MissingRequiredArgument {
        failure = failure.suggest("Pass every required argument; `details.arguments` lists them");
    }
    failure.with_details(Value::Object(details))
}

/// The longest command a word list names, aliases included: `rm` finds `remove`.
fn longest_known_command(words: &[String]) -> Option<&'static registry::CommandSpec> {
    (1..=words.len()).rev().find_map(|length| registry::resolve(&words[..length]))
}

fn report(output: OutputFormat, command: Option<&str>, outcome: Result<Response, Failure>) -> u8 {
    let (stdout, stderr) = (std::io::stdout(), std::io::stderr());
    let indented = stdout.is_terminal();
    report_to(&mut stdout.lock(), &mut stderr.lock(), indented, output, command, outcome)
}

/// [`report`] to the given streams, so a test can pass ones that fail. `indented` pretty-prints
/// JSON for a terminal.
fn report_to(
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    indented: bool,
    output: OutputFormat,
    command: Option<&str>,
    outcome: Result<Response, Failure>,
) -> u8 {
    match outcome {
        Ok(response) => {
            let rendered = match output {
                OutputFormat::Json => json_text(&response.result, indented),
                OutputFormat::Text => response.text,
            };
            let written = writeln!(stdout, "{}", rendered.trim_end_matches('\n'))
                .and_then(|()| stdout.flush());
            if let Some(failure) = written.err().and_then(unwritable_stdout) {
                return write_failure(stderr, output, &failure);
            }
            for warning in &response.warnings {
                let _ = writeln!(stderr, "warning: {warning}");
            }
            0
        }
        Err(mut failure) => {
            if failure.code == ErrorCode::Internal {
                failure = failure.suggest(format!(
                    "Report this defect in {NAME} {}{}",
                    env!("CARGO_PKG_VERSION"),
                    command.map(|id| format!(" ({id})")).unwrap_or_default()
                ));
            }
            write_failure(stderr, output, &failure)
        }
    }
}

/// The exit status once clap has printed help or the version to stdout.
fn printed(result: std::io::Result<()>) -> u8 {
    match result.and_then(|()| std::io::stdout().flush()).err().and_then(unwritable_stdout) {
        Some(failure) => fail_early(&failure),
        None => 0,
    }
}

/// The failure for output stdout did not take. `None` for a closed stdout
/// (`pkg list | head -1`): that is the reader's choice, not a failure.
fn unwritable_stdout(error: std::io::Error) -> Option<Failure> {
    (error.kind() != std::io::ErrorKind::BrokenPipe).then(|| {
        Failure::internal(format!("cannot write to stdout: {error}"))
            .with_details(json!({ "stream": "stdout" }))
            .suggest("The command ran and any change it made stands; verify it before rerunning")
    })
}

/// Writes `failure` to stderr, as text for people or one JSON document for `--output json`, and
/// returns its exit status.
fn write_failure(stderr: &mut impl Write, output: OutputFormat, failure: &Failure) -> u8 {
    let _ = match output {
        OutputFormat::Text => write!(stderr, "{}", failure.to_text()),
        OutputFormat::Json => writeln!(stderr, "{}", json_text(&failure.to_json(), false)),
    };
    failure.code.exit_status()
}

fn json_text(value: &Value, indented: bool) -> String {
    if indented { serde_json::to_string_pretty(value) } else { serde_json::to_string(value) }
        .unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;

    use super::*;

    /// A stdout that fails with `kind` on every write, or only when flushed.
    struct Failing {
        kind: ErrorKind,
        on_flush_only: bool,
    }

    impl Write for Failing {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            if self.on_flush_only { Ok(buffer.len()) } else { Err(self.kind.into()) }
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(self.kind.into())
        }
    }

    /// Reports a successful command with one warning to `stdout`: the exit status and stderr.
    fn report_success(stdout: &mut Failing) -> (u8, String) {
        let response = Response {
            result: json!({ "items": [] }),
            text: String::new(),
            warnings: vec!["searched an older catalog".into()],
        };
        let mut stderr = Vec::new();
        let status =
            report_to(stdout, &mut stderr, false, OutputFormat::Json, Some("list"), Ok(response));
        (status, String::from_utf8(stderr).unwrap())
    }

    #[test]
    fn a_stdout_that_cannot_take_the_result_is_a_structured_failure() {
        for on_flush_only in [false, true] {
            let mut stdout = Failing { kind: ErrorKind::StorageFull, on_flush_only };
            let (status, stderr) = report_success(&mut stdout);
            assert_eq!(status, ErrorCode::Internal.exit_status(), "{stderr}");
            let document: Value = serde_json::from_str(&stderr).unwrap();
            assert_eq!(document["error"]["code"], "INTERNAL", "{stderr}");
            assert_eq!(document["error"]["details"]["stream"], "stdout", "{stderr}");
        }
    }

    #[test]
    fn a_closed_stdout_is_the_readers_choice() {
        let mut stdout = Failing { kind: ErrorKind::BrokenPipe, on_flush_only: false };
        let (status, stderr) = report_success(&mut stdout);
        assert_eq!(status, 0);
        assert_eq!(stderr, "warning: searched an older catalog\n");
    }

    #[test]
    fn an_alias_names_its_command() {
        let words = ["rm".to_string()];
        assert_eq!(longest_known_command(&words).map(|command| command.id), Some("remove"));
    }
}
