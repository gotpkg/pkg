//! CLI argument declarations, operation adapters, and text formatting.

use std::io::Write;
use std::path::Path;

use pkg_core::{Client, Outcome, Progress};
use serde::Serialize;

use crate::environment::real_path;
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::registry::{ArgumentKind, ArgumentSpec, ValueKind};
use crate::response::{RenderOutput, Response};

/// The packages a command acts on.
pub const PACKAGES: ArgumentSpec = ArgumentSpec {
    name: "packages",
    kind: ArgumentKind::Positional,
    value: ValueKind::Package,
    required: true,
    default: None,
    multiple: true,
    summary: "One or more packages: `brew:wget`, `brew-cask:firefox`, `mas:497799835`, \
              `npm:prettier`, or a tool from mise's registry such as `ripgrep`",
};

/// The one package a command describes.
pub const PACKAGE: ArgumentSpec = ArgumentSpec {
    name: "package",
    kind: ArgumentKind::Positional,
    value: ValueKind::Package,
    required: true,
    default: None,
    multiple: false,
    summary: "The package: `brew:wget`, `brew-cask:firefox`, `mas:497799835`, `npm:prettier`, \
              or a tool from mise's registry such as `ripgrep`",
};

/// The errors every command that runs mise can report.
pub const MISE_ERRORS: &[ErrorCode] =
    &[ErrorCode::InvalidArgument, ErrorCode::PreconditionFailed, ErrorCode::MiseFailed];

/// Runs the typed operation and renders only the requested representation.
///
/// # Errors
/// Reports context construction or operation failures using the CLI error contract.
pub fn execute<T: Serialize + RenderOutput>(
    invocation: &Invocation,
    run: impl FnOnce(&mut Client, &mut dyn FnMut(Progress<'_>)) -> Result<Outcome<T>, pkg_core::Error>,
) -> Result<Response, Failure> {
    let mut client = Client::new(invocation.environment()?)?;
    let stderr = std::io::stderr();
    let mut stderr = stderr.lock();
    let mut forwarded_output = false;
    let mut progress = |event: Progress<'_>| {
        if let Progress::Output { text, .. } = event {
            forwarded_output = true;
            let _ = stderr.write_all(text.as_bytes());
            let _ = stderr.flush();
        }
    };
    let outcome = run(&mut client, &mut progress).map_err(|mut error| {
        if forwarded_output && error.details.output_streamed {
            error.details.output = Some(Vec::new());
        }
        Failure::from(error)
    })?;
    let mut response = Response::new(&outcome.result, invocation.output);
    response.warnings = outcome.warnings;
    Ok(response)
}

/// An absolute path from a result, for text: under the home directory, written as mise resolved
/// it or with its symlinks resolved, it starts with `~`.
pub fn for_people(path: impl AsRef<Path>) -> String {
    let path = path.as_ref().display().to_string();
    let home =
        std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap_or_default();
    if home.is_empty() {
        return path.to_string();
    }
    let resolved = real_path(Path::new(&home)).display().to_string();
    for home in [home.as_str(), resolved.as_str()] {
        if let Some(rest) = path.strip_prefix(home)
            && (rest.is_empty() || rest.starts_with(['/', '\\']))
        {
            return format!("~{rest}");
        }
    }
    path.to_string()
}

/// `values` as `a`, `a and b`, or `a, b, and c`.
pub fn join_words(values: &[String]) -> String {
    match values {
        [] => String::new(),
        [one] => one.clone(),
        [one, two] => format!("{one} and {two}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// Rows of cells, each column padded to its widest cell, indented by two spaces.
pub fn table(rows: &[Vec<String>]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.get(column))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut text = String::new();
    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(column, cell)| {
                if column + 1 == row.len() {
                    cell.clone()
                } else {
                    format!("{cell:width$}", width = widths[column])
                }
            })
            .collect();
        text.push_str(&format!("  {}\n", cells.join("  ").trim_end()));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_words_as_a_sentence_would() {
        let words =
            |values: &[&str]| values.iter().map(|value| (*value).to_string()).collect::<Vec<_>>();
        assert_eq!(join_words(&words(&["a"])), "a");
        assert_eq!(join_words(&words(&["a", "b"])), "a and b");
        assert_eq!(join_words(&words(&["a", "b", "c"])), "a, b, and c");
    }

    #[test]
    fn tables_pad_every_column_but_the_last() {
        let rows = vec![
            vec!["wget".to_string(), "1.25.0".to_string(), String::new()],
            vec!["coreutils".to_string(), "9.12".to_string(), "GNU".to_string()],
        ];
        assert_eq!(table(&rows), "  wget       1.25.0\n  coreutils  9.12    GNU\n");
    }
}
