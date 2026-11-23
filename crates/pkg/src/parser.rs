//! The Clap parser, generated from the registry.

use clap::builder::{PossibleValuesParser, RangedU64ValueParser};
use clap::{Arg, ArgAction, Command};

use crate::NAME;
use crate::registry::{
    self, ArgumentKind, ArgumentSpec, Audience, CommandSpec, GLOBAL_ARGUMENTS, RESOURCES, ValueKind,
};
use pkg_core::is_package;

const ORIENTATION: &str = "\
Install, remove, and find packages with mise. Every install is recorded in your mise config, so \
`mise bootstrap` brings it back on a new machine.

Name a package the way mise does:
  brew:wget            a Homebrew formula
  brew-cask:firefox    a Homebrew cask (an app)
  mas:497799835        a Mac App Store app
  npm:prettier         a tool from a mise backend (npm, cargo, github, aqua, ...)
  ripgrep              a tool from mise's registry";

const AFTER_HELP: &str = "\
Examples:
  pkg search ffmpeg
  pkg install brew:wget brew-cask:firefox
  pkg remove brew:wget
  pkg outdated

Every command that changes something accepts --dry-run.

For scripts and agents: add --output json for JSON results and errors, and use
  pkg cli search \"<task>\"   pkg schema <command>   pkg docs search \"<topic>\"   pkg context show";

pub fn build() -> Command {
    let mut root = Command::new(NAME)
        .version(env!("CARGO_PKG_VERSION"))
        .about(ORIENTATION)
        .after_help(AFTER_HELP)
        .subcommand_required(true)
        .disable_help_subcommand(true);
    for argument in GLOBAL_ARGUMENTS {
        root = root.arg(clap_arg(argument).global(true));
    }

    let commands = registry::all();
    for (resource, summary, audience) in RESOURCES {
        let members: Vec<&CommandSpec> =
            commands.iter().copied().filter(|command| command.path[0] == *resource).collect();
        let subcommand = match members.as_slice() {
            [] => continue,
            [single] if single.path.len() == 1 => leaf(single),
            _ => {
                let mut group = Command::new(*resource)
                    .about(*summary)
                    .subcommand_required(true)
                    .disable_help_subcommand(true);
                for member in members {
                    group = group.subcommand(leaf(member));
                }
                group
            }
        };
        root = root.subcommand(subcommand.hide(*audience == Audience::Agents));
    }
    root
}

fn leaf(spec: &CommandSpec) -> Command {
    let name = *spec.path.last().expect("a command path has at least one word");
    let mut command = Command::new(name)
        .visible_aliases(spec.aliases.iter().copied())
        .about(spec.summary)
        .long_about(format!("{}\n\n{}", spec.summary, spec.description))
        .after_help(examples_help(spec));
    for argument in spec.arguments {
        command = command.arg(clap_arg(argument));
    }
    command
}

fn examples_help(spec: &CommandSpec) -> String {
    let mut help = String::from("Examples:");
    for example in spec.examples {
        help.push_str(&format!(
            "\n  {}\n      {}",
            registry::example_command_line(example),
            example.summary
        ));
    }
    help
}

fn clap_arg(spec: &ArgumentSpec) -> Arg {
    let mut arg = Arg::new(spec.name).help(spec.summary);
    arg = match spec.kind {
        ArgumentKind::Positional => arg.value_name(spec.name.to_uppercase()),
        ArgumentKind::Option => {
            arg.long(spec.name).value_name(spec.value.type_name().to_uppercase())
        }
        ArgumentKind::Flag => arg.long(spec.name),
    };
    arg = match spec.value {
        ValueKind::Boolean => arg.action(ArgAction::SetTrue),
        ValueKind::Integer { min, max } => arg
            .value_parser(RangedU64ValueParser::<u64>::new().range(min..=max.unwrap_or(u64::MAX))),
        ValueKind::Package => arg.value_parser(parse_package),
        ValueKind::Choice(values) => {
            arg.value_parser(PossibleValuesParser::new(values.iter().copied()))
        }
        ValueKind::Raw => arg.value_parser(clap::value_parser!(std::ffi::OsString)),
        ValueKind::String => arg,
    };
    if spec.kind != ArgumentKind::Flag {
        arg = arg.required(spec.required);
        if spec.multiple {
            arg = match spec.kind {
                ArgumentKind::Positional => arg.num_args(if spec.required { 1.. } else { 0.. }),
                _ => arg.action(ArgAction::Append),
            };
        }
        if let Some(default) = spec.default {
            arg = arg.default_value(default);
        }
    }
    arg
}

/// Accepts a package: see [`registry::PACKAGE_PATTERN`].
fn parse_package(value: &str) -> Result<String, String> {
    if is_package(value) {
        Ok(value.to_string())
    } else {
        Err("name a package as `manager:name`, such as `brew:wget` or `brew-cask:firefox`, or a \
             mise tool such as `ripgrep` or `npm:prettier`"
            .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generated_parser_is_well_formed() {
        build().debug_assert();
    }

    #[test]
    fn help_lists_commands_for_people_and_hides_the_agent_commands() {
        let help = build().render_help().to_string();
        assert!(help.contains("install"), "{help}");
        assert!(!help.contains("Find the command for a task"), "{help}");
        assert!(help.contains("pkg cli search"), "{help}");
    }
}
