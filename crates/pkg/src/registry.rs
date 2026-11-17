//! The command registry: the one declaration of every operation `pkg` performs.
//!
//! The parser, `--help`, `pkg schema`, the `pkg cli search` index, and the CLI reference
//! (`docs/cli.md`) are all generated from these entries, and the tests in `tests/cli/registry.rs`
//! fail when the parser, the runtime defaults, or the examples drift from them.

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::NAME;
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::response::Response;

/// One operation, declared once.
pub struct CommandSpec {
    /// Stable identifier, the command path joined with dots: `tap.add`.
    pub id: &'static str,
    /// The words after `pkg`: `["tap", "add"]`.
    pub path: &'static [&'static str],
    /// Other names the last word of the path answers to: `rm` for `remove`.
    pub aliases: &'static [&'static str],
    /// One line, shown in search results and `--help` listings.
    pub summary: &'static str,
    /// Everything a person or agent needs to use the command correctly, in prose.
    pub description: &'static str,
    pub arguments: &'static [ArgumentSpec],
    pub effect: Effect,
    pub requires: &'static [Requirement],
    /// Whether the command accepts `--dry-run`. Every modifying command does.
    pub dry_run: bool,
    /// Whether running it twice with the same input leaves the same state as running it once.
    pub idempotent: bool,
    /// Every error code the command can return, besides `INTERNAL`.
    pub errors: &'static [ErrorCode],
    /// At least one example. Each one runs against a fake mise in the test suite, and its result
    /// must match `output_schema`.
    pub examples: &'static [Example],
    /// Words someone might use for this operation that its name and summary do not contain.
    pub synonyms: &'static [&'static str],
    /// JSON Schema for the result `--output json` writes to stdout.
    pub output_schema: fn() -> schemars::Schema,
    pub run: fn(&Invocation) -> Result<Response, Failure>,
}

/// Whether an operation reads or modifies state, and what it modifies.
#[derive(Clone, Copy)]
pub enum Effect {
    Reads,
    Modifies(&'static str),
}

/// What an operation needs at run time beyond `pkg` itself.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    /// The `mise` executable, which pkg runs in your home directory.
    Mise,
    /// The Homebrew or App Store catalog, over HTTPS.
    Network,
}

impl Requirement {
    pub fn as_str(self) -> &'static str {
        match self {
            Requirement::Mise => "mise",
            Requirement::Network => "network",
        }
    }
}

/// Who a resource is for: `--help` lists the commands for people and points agents at the rest.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    People,
    Agents,
}

pub struct ArgumentSpec {
    /// Kebab-case. A flag or option is passed as `--<name>`; a positional is passed bare.
    pub name: &'static str,
    pub kind: ArgumentKind,
    pub value: ValueKind,
    pub required: bool,
    /// The value used when the argument is absent. Clap applies it, so it is the runtime default.
    pub default: Option<&'static str>,
    /// A positional or option that accepts several values.
    pub multiple: bool,
    pub summary: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ArgumentKind {
    Positional,
    Option,
    Flag,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    String,
    /// A whole number the parser checks against its bounds.
    Integer {
        min: u64,
        max: Option<u64>,
    },
    /// A package as mise names it: see [`PACKAGE_PATTERN`].
    Package,
    Choice(&'static [&'static str]),
    Boolean,
}

/// What a package looks like: `manager:name`, `backend:name`, or a bare mise registry tool, with an
/// optional `@version`.
pub const PACKAGE_PATTERN: &str = r"^([a-z0-9][a-z0-9-]*:)?\S+$";

impl ValueKind {
    pub fn type_name(self) -> &'static str {
        match self {
            ValueKind::String => "string",
            ValueKind::Integer { .. } => "integer",
            ValueKind::Package => "package",
            ValueKind::Choice(_) => "choice",
            ValueKind::Boolean => "boolean",
        }
    }

    pub fn accepted_values(self) -> Option<&'static [&'static str]> {
        match self {
            ValueKind::Choice(values) => Some(values),
            _ => None,
        }
    }
}

pub struct Example {
    pub summary: &'static str,
    /// The arguments after `pkg`.
    pub args: &'static [&'static str],
}

/// Options every command accepts.
pub const GLOBAL_ARGUMENTS: &[ArgumentSpec] = &[ArgumentSpec {
    name: "output",
    kind: ArgumentKind::Option,
    value: ValueKind::Choice(&["text", "json"]),
    required: false,
    default: Some("text"),
    multiple: false,
    summary: "Result format: text for people, or json for scripts and agents (results on stdout, \
              errors as one JSON document on stderr)",
}];

/// The `--dry-run` flag every modifying command declares.
pub const DRY_RUN: ArgumentSpec = ArgumentSpec {
    name: "dry-run",
    kind: ArgumentKind::Flag,
    value: ValueKind::Boolean,
    required: false,
    default: None,
    multiple: false,
    summary: "Show what would change without installing, removing, or writing anything",
};

/// The resource groups, in the order `--help` lists them, with a summary of each and who it is
/// for.
pub const RESOURCES: &[(&str, &str, Audience)] = &[
    ("install", "Install packages and record them in your mise config", Audience::People),
    ("remove", "Uninstall packages and take them out of your mise config", Audience::People),
    ("list", "List the packages and tools your mise config declares", Audience::People),
    ("search", "Find packages in Homebrew, mise's registry, and the App Store", Audience::People),
    ("info", "Describe one package and whether it is installed", Audience::People),
    ("outdated", "List installed packages that have a newer version", Audience::People),
    ("upgrade", "Upgrade installed packages to their newest versions", Audience::People),
    ("sync", "Install everything your mise config declares that is missing", Audience::People),
    ("prune", "Uninstall Homebrew packages no config declares", Audience::People),
    ("where", "Print where a formula or tool is installed", Audience::People),
    ("tap", "List, add, and remove Homebrew tap URLs", Audience::People),
    ("cli", "Find the command for a task", Audience::Agents),
    ("schema", "Inspect a command's full specification", Audience::Agents),
    ("docs", "Search and read pkg's documentation", Audience::Agents),
    ("context", "Show the mise config and catalogs pkg uses", Audience::Agents),
];

/// Every command, in the order discovery lists them.
pub fn all() -> Vec<&'static CommandSpec> {
    crate::commands::GROUPS.iter().flat_map(|group| group.iter()).collect()
}

pub fn find(path: &[&str]) -> Option<&'static CommandSpec> {
    all().into_iter().find(|command| command.path == path)
}

/// The command `words` name, where the last word may be an alias: `rm` finds `remove`.
pub fn resolve<S: AsRef<str>>(words: &[S]) -> Option<&'static CommandSpec> {
    all().into_iter().find(|command| {
        command.path.len() == words.len()
            && command.path.iter().zip(words).enumerate().all(|(index, (word, given))| {
                let given = given.as_ref();
                *word == given
                    || (index + 1 == command.path.len() && command.aliases.contains(&given))
            })
    })
}

impl CommandSpec {
    pub fn command_line(&self) -> String {
        format!("{NAME} {}", self.path.join(" "))
    }

    pub fn effect_label(&self) -> String {
        match self.effect {
            Effect::Reads => "reads".to_string(),
            Effect::Modifies(what) => format!("modifies {what}"),
        }
    }

    /// The command's arguments followed by the global ones.
    pub fn all_arguments(&self) -> impl Iterator<Item = &'static ArgumentSpec> {
        self.arguments.iter().chain(GLOBAL_ARGUMENTS.iter())
    }

    /// The full entry `pkg schema <command>` returns.
    pub fn describe(&self) -> CommandDescription {
        let errors = self
            .errors
            .iter()
            .chain(std::iter::once(&ErrorCode::Internal))
            .map(|code| ErrorDescription {
                code: code.as_str().to_string(),
                exit: code.exit_status(),
                retryable: code.retryable(),
                meaning: code.meaning().to_string(),
            })
            .collect();
        CommandDescription {
            id: self.id.to_string(),
            command: self.command_line(),
            aliases: self.aliases.iter().map(|alias| (*alias).to_string()).collect(),
            summary: self.summary.to_string(),
            description: self.description.to_string(),
            arguments: self.all_arguments().map(ArgumentSpec::describe).collect(),
            effect: match self.effect {
                Effect::Reads => EffectDescription { kind: "reads".into(), what: None },
                Effect::Modifies(what) => {
                    EffectDescription { kind: "modifies".into(), what: Some(what.to_string()) }
                }
            },
            requires: self
                .requires
                .iter()
                .map(|requirement| requirement.as_str().to_string())
                .collect(),
            dry_run: self.dry_run,
            idempotent: self.idempotent,
            errors,
            examples: self
                .examples
                .iter()
                .map(|example| ExampleDescription {
                    summary: example.summary.to_string(),
                    command: example_command_line(example),
                })
                .collect(),
            output: (self.output_schema)().to_value(),
        }
    }
}

/// A command's full specification, as `pkg schema <command>` returns it.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommandDescription {
    /// Stable identifier: the command path joined with dots.
    pub id: String,
    /// The command to run, without arguments.
    pub command: String,
    /// Other names the command's last word answers to, such as `rm` for `remove`.
    pub aliases: Vec<String>,
    pub summary: String,
    pub description: String,
    /// The command's own arguments, then the options every command accepts.
    pub arguments: Vec<ArgumentDescription>,
    pub effect: EffectDescription,
    /// What the command needs besides pkg: `mise` (the executable, run in your home directory)
    /// or `network` (the Homebrew or App Store catalog).
    pub requires: Vec<String>,
    /// Whether the command accepts `--dry-run`.
    pub dry_run: bool,
    /// Whether running it twice with the same input leaves the same state as running it once.
    pub idempotent: bool,
    /// Every error the command can report on stderr.
    pub errors: Vec<ErrorDescription>,
    pub examples: Vec<ExampleDescription>,
    /// JSON Schema of the result `--output json` writes to stdout.
    pub output: Value,
}

#[derive(Serialize, JsonSchema)]
pub struct EffectDescription {
    /// `reads` or `modifies`.
    pub kind: String,
    /// What a modifying command changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub what: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ErrorDescription {
    pub code: String,
    /// The exit status the error sets.
    pub exit: u8,
    /// Whether retrying the same command can succeed.
    pub retryable: bool,
    pub meaning: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ExampleDescription {
    pub summary: String,
    pub command: String,
}

#[derive(Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArgumentDescription {
    pub name: String,
    /// `positional`, `option`, or `flag`.
    pub kind: String,
    /// How to write it: `<package>`, `--limit <integer>`, `--dry-run`.
    pub usage: String,
    /// `string`, `integer`, `package` (matches `pattern`), `choice`, or `boolean`.
    #[serde(rename = "type")]
    pub value_type: String,
    pub required: bool,
    pub default: Option<String>,
    /// Whether it accepts several values.
    pub multiple: bool,
    /// The only values a `choice` accepts.
    pub accepted_values: Option<Vec<String>>,
    /// The smallest `integer` accepted.
    pub minimum: Option<u64>,
    /// The largest `integer` accepted.
    pub maximum: Option<u64>,
    /// The regular expression a `package` matches.
    pub pattern: Option<String>,
    pub summary: String,
}

pub fn example_command_line(example: &Example) -> String {
    let words: Vec<String> = example.args.iter().map(|arg| shell_word(arg)).collect();
    format!("{NAME} {}", words.join(" "))
}

fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_./@=:".contains(character));
    if plain { word.to_string() } else { format!("'{}'", word.replace('\'', r"'\''")) }
}

impl ArgumentSpec {
    pub fn usage(&self) -> String {
        match self.kind {
            ArgumentKind::Positional => format!("<{}>", self.name),
            ArgumentKind::Option => format!("--{} <{}>", self.name, self.value.type_name()),
            ArgumentKind::Flag => format!("--{}", self.name),
        }
    }

    pub fn describe(&self) -> ArgumentDescription {
        let (minimum, maximum) = match self.value {
            ValueKind::Integer { min, max } => (Some(min), max),
            _ => (None, None),
        };
        ArgumentDescription {
            name: self.name.to_string(),
            kind: match self.kind {
                ArgumentKind::Positional => "positional",
                ArgumentKind::Option => "option",
                ArgumentKind::Flag => "flag",
            }
            .to_string(),
            usage: self.usage(),
            value_type: self.value.type_name().to_string(),
            required: self.required,
            default: self.default.map(str::to_string),
            multiple: self.multiple,
            accepted_values: self
                .value
                .accepted_values()
                .map(|values| values.iter().map(|value| (*value).to_string()).collect()),
            minimum,
            maximum,
            pattern: (self.value == ValueKind::Package).then(|| PACKAGE_PATTERN.to_string()),
            summary: self.summary.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn examples_quote_words_a_shell_would_split() {
        let example = Example { summary: "", args: &["search", "video player"] };
        assert_eq!(example_command_line(&example), format!("{NAME} search 'video player'"));
    }
}
