//! `pkg bootstrap`: restore a configuration with `mise bootstrap`.

use std::io::IsTerminal;
use std::path::PathBuf;

use pkg_core::{BootstrapReceipt, BootstrapRequest, BootstrapSource, ReceiptStatus, SourceKind};
use schemars::schema_for;
use serde_json::json;

use super::common::{execute, for_people, join_words};
use super::setup::{handoff_lines, mise_line};
use crate::failure::{ErrorCode, Failure};
use crate::invocation::{Invocation, OutputFormat};
use crate::registry::{
    ArgumentKind, ArgumentSpec, CommandSpec, DRY_RUN, Effect, Example, Requirement, ValueKind,
};
use crate::response::{RenderOutput, Response};

const fn option(name: &'static str, multiple: bool, summary: &'static str) -> ArgumentSpec {
    ArgumentSpec {
        name,
        kind: ArgumentKind::Option,
        value: ValueKind::String,
        required: false,
        default: None,
        multiple,
        summary,
    }
}

const fn flag(name: &'static str, summary: &'static str) -> ArgumentSpec {
    ArgumentSpec {
        name,
        kind: ArgumentKind::Flag,
        value: ValueKind::Boolean,
        required: false,
        default: None,
        multiple: false,
        summary,
    }
}

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "bootstrap",
    path: &["bootstrap"],
    aliases: &[],
    summary: "Restore a machine from your mise configuration with mise bootstrap",
    description: "Runs `mise bootstrap`, which applies everything a configuration declares: \
        packages, tools, dotfiles, settings, services, hooks, and the `bootstrap` task. Name a \
        dotfiles repository with --from <url> --directory <checkout> --config-dir <directory>: \
        mise clones it (or reuses a checkout whose origin is <url>) and links its global config \
        directory, normally `~/.config/mise`, to <config-dir> inside the checkout; pkg refuses \
        before changing anything when that directory already holds another configuration. Name \
        one existing config file with --config, applied with the global configuration. With no \
        source it applies the global configuration, as every run after the first does. --env \
        selects configuration environments, --save-env makes them the default, and --update \
        pulls new commits and refreshes package metadata first; pkg never prunes, forces, or \
        answers for you. When mise is missing it is installed first, as `pkg setup` does. \
        Startup files that hold only the installer's PATH block are moved aside while mise \
        runs and put back where the configuration deployed nothing. With text output in a \
        terminal, mise and the programs it runs may ask you for passwords and answers there; \
        with --output json or without a terminal nothing waits, and a step that needs you \
        fails. A successful run hands pkg to mise. A configuration can run any command, so \
        name only one you have reviewed. --dry-run runs `mise bootstrap --dry-run`, without \
        the `files` part when a templated `[bootstrap.files]` entry could run a command, and \
        changes nothing. Fails on Windows.",
    arguments: &[
        option(
            "from",
            false,
            "A dotfiles repository's Git URL; needs --directory and --config-dir",
        ),
        option(
            "directory",
            false,
            "Where the repository's checkout is or goes, such as ~/.dotfiles",
        ),
        option(
            "config-dir",
            false,
            "The checkout's directory that holds your global mise config, such as `mise` (`.` for the root)",
        ),
        option(
            "config",
            false,
            "One existing mise config file to apply with the global configuration",
        ),
        option("env", true, "A configuration environment to select, such as `work`; repeatable"),
        flag("save-env", "Make the --env selection the default for later runs"),
        flag("update", "Pull new commits and refresh package metadata first"),
        DRY_RUN,
    ],
    effect: Effect::Modifies(
        "the whole machine as the configuration declares it, and pkg's own installation",
    ),
    requires: &[Requirement::Network],
    dry_run: true,
    idempotent: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::NotFound,
        ErrorCode::PreconditionFailed,
        ErrorCode::Unavailable,
        ErrorCode::MiseFailed,
    ],
    examples: &[
        Example {
            summary: "Preview applying your global configuration",
            args: &["bootstrap", "--dry-run"],
        },
        Example { summary: "Apply your global configuration", args: &["bootstrap"] },
    ],
    synonyms: &["restore", "dotfiles", "new machine", "enroll", "clone", "apply", "provision"],
    output_schema: || schema_for!(BootstrapReceipt),
    run: bootstrap,
}];

impl RenderOutput for BootstrapReceipt {
    fn render_text(&self) -> String {
        let dry = self.status == ReceiptStatus::DryRun;
        let mut text = mise_line(&self.mise, dry);
        text.push_str(&source_line(self, dry));
        let moved: Vec<String> = self.startup_files.moved_aside.iter().map(for_people).collect();
        let restored: Vec<String> = self.startup_files.restored.iter().map(for_people).collect();
        if !moved.is_empty() {
            let verb = if dry { "would move aside" } else { "moved aside" };
            text.push_str(&format!("startup files: {verb} {}", join_words(&moved)));
            if !restored.is_empty() {
                text.push_str(&format!("; put back {}", join_words(&restored)));
            }
            text.push('\n');
        }
        if let Some(handoff) = &self.handoff {
            text.push_str(&handoff_lines(handoff, dry));
        }
        if let Some(items) = self.follow_up.as_ref().filter(|items| !items.is_empty()) {
            text.push_str(if dry { "Left for you if applied:\n" } else { "Left for you:\n" });
            for item in items {
                text.push_str(&format!("  - {item}\n"));
            }
        }
        for part in &self.not_previewed {
            text.push_str(&format!("Not previewed: {part}\n"));
        }
        if dry {
            text.push_str("Dry run: nothing was installed, cloned, linked, or written.\n");
        }
        text
    }
}

fn source_line(receipt: &BootstrapReceipt, dry: bool) -> String {
    let source = &receipt.source;
    let applied = if dry { "would apply" } else { "applied" };
    let environments = if receipt.environments.is_empty() {
        String::new()
    } else {
        format!(" ({})", receipt.environments.join(", "))
    };
    match source.kind {
        SourceKind::Global => {
            format!("bootstrap: {applied} your global configuration{environments}\n")
        }
        SourceKind::Config => format!(
            "bootstrap: {applied} {}{environments} with your global configuration\n",
            source.config.as_ref().map(for_people).unwrap_or_default()
        ),
        SourceKind::Repository => format!(
            "bootstrap: {applied} {}{environments}; {} links to {}\n",
            source.url.as_deref().unwrap_or_default(),
            for_people(&receipt.config_dir),
            for_people(
                source
                    .directory
                    .as_ref()
                    .map(|directory| directory.join(source.config_dir.as_deref().unwrap_or(".")))
                    .unwrap_or_default()
            ),
        ),
    }
}

fn bootstrap(invocation: &Invocation) -> Result<Response, Failure> {
    let request = BootstrapRequest {
        source: source(invocation)?,
        environments: invocation.strings("env"),
        save_environments: invocation.flag("save-env"),
        update: invocation.flag("update"),
        mode: invocation.mode(),
        terminal: invocation.output == OutputFormat::Text
            && std::io::stdin().is_terminal()
            && std::io::stderr().is_terminal(),
        pkg_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    execute(invocation, |client, progress| client.bootstrap(request, progress))
}

/// The configuration the flags name, refusing combinations that name more or less than one.
fn source(invocation: &Invocation) -> Result<BootstrapSource, Failure> {
    let from = invocation.string("from");
    let directory = invocation.string("directory");
    let config_dir = invocation.string("config-dir");
    let config = invocation.string("config");
    let conflict = |message: &str| {
        Failure::invalid_argument(message)
            .with_details(json!({ "arguments": ["from", "directory", "config-dir", "config"] }))
    };
    match (from, directory, config_dir, config) {
        (None, None, None, None) => Ok(BootstrapSource::Global),
        (None, None, None, Some(config)) => Ok(BootstrapSource::ConfigFile(absolute(&config)?)),
        (Some(url), Some(directory), Some(config_dir), None) => {
            Ok(BootstrapSource::Repository { url, directory: absolute(&directory)?, config_dir })
        }
        (Some(_), _, _, Some(_)) => Err(conflict("--config cannot be combined with --from")),
        _ => Err(conflict("--from, --directory, and --config-dir go together")),
    }
}

/// `path` relative to the current directory when it is not absolute.
fn absolute(path: &str) -> Result<PathBuf, Failure> {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|directory| directory.join(path))
        .map_err(|error| Failure::internal(format!("cannot read the current directory: {error}")))
}
