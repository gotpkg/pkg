//! The CLI reference, generated from the registry and shipped as `docs/cli.md`.
//!
//! `mise run cli:reference` regenerates the file; a test fails when it is out of date.

use std::fmt::Write;

use crate::NAME;
use crate::failure::{ErrorCode, ErrorDocument};
use crate::registry::{self, ArgumentDescription, CommandDescription, GLOBAL_ARGUMENTS, RESOURCES};

const INTRODUCTION: &str = "\
`pkg` installs, removes, and finds packages with mise and records every install in your global \
mise config. People use the text output; scripts and agents add `--output json`. This page is \
generated from the command registry by `mise run cli:reference`; do not edit it by hand.

Every command follows one contract:

- **Results** are text for people by default. `--output json` writes the same result as one JSON \
document on stdout, compact when piped and indented on a terminal. mise's progress, warnings, \
and errors go to stderr.
- **Collections** return `items`, `hasMore`, and `nextCursor`; pass `nextCursor` back as \
`--cursor` until `hasMore` is false.
- **Modifying commands** accept `--dry-run`, which makes the same checks and decisions as the \
real run and installs, uninstalls, and writes nothing. Receipts carry a `status` (`completed` or \
`dry-run`) and what changed.
- **Failures** write a message and hints to stderr, or with `--output json` one JSON document, \
and exit with the code's status (see below). Nothing is written to stdout.
- **No command prompts.** Missing input fails at once with `INVALID_ARGUMENT`.
- **mise runs in your home directory**, so only the global config counts. New declarations go to \
the files mise's `write_targets.packages` and `write_targets.tools` settings name. \
`pkg context show` prints them.

Discover a command with `pkg cli search \"<task>\"`, inspect it with `pkg schema <command>`, \
preview it with `--dry-run`, run it, and verify the result with `pkg list` or `pkg info <package>`.";

/// The whole page.
///
/// # Panics
///
/// When a schema fails to serialize, which `serde_json` values never do.
pub fn render() -> String {
    let mut page = String::from("# CLI reference\n\n");
    page.push_str(INTRODUCTION);
    page.push_str("\n\n## Options every command accepts\n\n");
    page.push_str(&argument_table(
        &GLOBAL_ARGUMENTS.iter().map(registry::ArgumentSpec::describe).collect::<Vec<_>>(),
    ));
    page.push_str(
        "\n## Errors\n\n| Code | Exit | Retryable | Meaning |\n| --- | --- | --- | --- |\n",
    );
    for code in ErrorCode::ALL {
        let _ = writeln!(
            page,
            "| `{}` | {} | {} | {} |",
            code.as_str(),
            code.exit_status(),
            if code.retryable() { "yes" } else { "no" },
            code.meaning()
        );
    }
    page.push_str("\nEvery failure writes this document to stderr:\n\n```json\n");
    page.push_str(
        &serde_json::to_string_pretty(&schemars::schema_for!(ErrorDocument))
            .expect("schemas serialize"),
    );
    page.push_str("\n```\n\n## Commands\n");
    for (resource, summary, _) in RESOURCES {
        let commands: Vec<CommandDescription> = registry::all()
            .into_iter()
            .filter(|command| command.path[0] == *resource)
            .map(registry::CommandSpec::describe)
            .collect();
        if commands.is_empty() {
            continue;
        }
        let _ = write!(page, "\n### `{resource}`\n\n{summary}.\n");
        for command in &commands {
            page.push_str(&command_section(command));
        }
    }
    page
}

fn command_section(command: &CommandDescription) -> String {
    let mut section = format!(
        "\n#### `{}`\n\n{}.\n\n{}\n\n",
        command.command, command.summary, command.description
    );
    let effect = match &command.effect.what {
        Some(what) => format!("modifies {what}"),
        None => "reads only".to_string(),
    };
    let requires = if command.requires.is_empty() {
        format!("nothing beyond `{NAME}`")
    } else {
        command
            .requires
            .iter()
            .map(|requirement| format!("`{requirement}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let _ = writeln!(
        section,
        "- **Effect:** {effect}\n- **Requires:** {requires}\n- **Dry run:** {}\n- **Idempotent:** {}\n- **Errors:** {}\n",
        if command.dry_run { "yes" } else { "no" },
        if command.idempotent { "yes" } else { "no" },
        command
            .errors
            .iter()
            .map(|error| format!("`{}`", error.code))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let own: Vec<ArgumentDescription> = command
        .arguments
        .iter()
        .filter(|argument| !GLOBAL_ARGUMENTS.iter().any(|global| global.name == argument.name))
        .cloned()
        .collect();
    if !own.is_empty() {
        section.push_str(&argument_table(&own));
        section.push('\n');
    }
    section.push_str("Examples:\n\n```sh\n");
    for example in &command.examples {
        let _ = writeln!(section, "# {}\n{}", example.summary, example.command);
    }
    section.push_str("```\n\nResult schema:\n\n```json\n");
    section.push_str(&serde_json::to_string_pretty(&command.output).unwrap_or_default());
    section.push_str("\n```\n");
    section
}

fn argument_table(arguments: &[ArgumentDescription]) -> String {
    let mut table = String::from(
        "| Argument | Type | Required | Default | Description |\n| --- | --- | --- | --- | --- |\n",
    );
    for argument in arguments {
        let value_type = match (&argument.accepted_values, argument.minimum, argument.maximum) {
            (Some(values), _, _) => {
                values.iter().map(|value| format!("`{value}`")).collect::<Vec<_>>().join(", ")
            }
            (None, Some(minimum), Some(maximum)) => format!("integer, {minimum}–{maximum}"),
            (None, Some(minimum), None) => format!("integer, at least {minimum}"),
            _ if argument.pattern.is_some() => {
                format!("package: `{}`", argument.pattern.as_deref().unwrap_or_default())
            }
            _ if argument.multiple => format!("{}, repeatable", argument.value_type),
            _ => argument.value_type.clone(),
        };
        let _ = writeln!(
            table,
            "| `{}` | {} | {} | {} | {} |",
            argument.usage,
            value_type.replace('|', "\\|"),
            if argument.required { "yes" } else { "no" },
            argument
                .default
                .as_deref()
                .map(|default| if default.is_empty() {
                    "empty".into()
                } else {
                    format!("`{default}`")
                })
                .unwrap_or_else(|| "—".into()),
            argument.summary.replace('|', "\\|")
        );
    }
    table
}
