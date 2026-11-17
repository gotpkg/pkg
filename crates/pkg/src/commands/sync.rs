//! `pkg sync`: install everything the config declares that is missing, as `mise bootstrap` does
//! for packages and tools alone.

use schemars::{JsonSchema, schema_for};
use serde::Serialize;

use super::common::{MISE_ERRORS, Machine, ReceiptStatus, join_words};
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::registry::{CommandSpec, DRY_RUN, Effect, Example, Requirement};
use crate::response::{RenderText, Response};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "sync",
    path: &["sync"],
    aliases: &[],
    summary: "Install everything your mise config declares that is missing",
    description: "Brings this machine in line with the config's packages and tools: \
        `mise bootstrap packages apply` installs every missing `[bootstrap.packages]` entry (and \
        removes packages declared with `state = \"absent\"`, where the manager can), and \
        `mise install` installs every missing global tool. Use it after pulling config changes \
        made on another machine; unlike `mise bootstrap` it leaves dotfiles, services, and \
        settings alone. Packages whose manager this machine lacks, such as `apt:` on macOS, are \
        skipped. --dry-run lists what is missing and changes nothing. mise's progress goes to \
        stderr.",
    arguments: &[DRY_RUN],
    effect: Effect::Modifies("installed software"),
    requires: &[Requirement::Mise],
    dry_run: true,
    idempotent: true,
    errors: MISE_ERRORS,
    examples: &[
        Example { summary: "See what is missing", args: &["sync", "--dry-run"] },
        Example { summary: "Install what is missing", args: &["sync"] },
    ],
    synonyms: &["apply", "restore", "bundle", "missing", "bootstrap", "converge", "catch up"],
    output_schema: || schema_for!(SyncReceipt),
    run: sync,
}];

/// The outcome of `pkg sync`.
#[derive(Serialize, JsonSchema)]
pub struct SyncReceipt {
    pub status: ReceiptStatus,
    /// Packages and tools that were missing and are now installed (for `dry-run`: are missing).
    pub installed: Vec<String>,
    /// Packages still missing after the run; empty for `dry-run`.
    pub missing: Vec<String>,
}

impl RenderText for SyncReceipt {
    fn render_text(&self) -> String {
        let mut text = match (self.status, self.installed.is_empty()) {
            (_, true) if self.missing.is_empty() => {
                "Everything your config declares is installed.\n".to_string()
            }
            (_, true) => String::new(),
            (ReceiptStatus::Completed, false) => {
                format!("Installed {}\n", join_words(&self.installed))
            }
            (ReceiptStatus::DryRun, false) => {
                format!(
                    "Would install {}\nDry run: nothing was installed.\n",
                    join_words(&self.installed)
                )
            }
        };
        if !self.missing.is_empty() {
            text.push_str(&format!("Still missing: {}\n", join_words(&self.missing)));
        }
        text
    }
}

fn sync(invocation: &Invocation) -> Result<Response, Failure> {
    let machine = Machine::load(invocation)?;
    let mise = &machine.environment.mise;
    let (packages, tools) = missing(&machine)?;
    let before: Vec<String> = packages.iter().chain(&tools).cloned().collect();
    if invocation.dry_run() {
        return Ok(Response::new(&SyncReceipt {
            status: ReceiptStatus::DryRun,
            installed: before,
            missing: Vec::new(),
        }));
    }
    if !packages.is_empty() {
        mise.run(&["bootstrap", "packages", "apply", "--yes"])?;
    }
    if !tools.is_empty() {
        mise.run(&["install"])?;
    }
    let (packages, tools) = missing(&machine)?;
    let still: Vec<String> = packages.into_iter().chain(tools).collect();
    let installed = before.into_iter().filter(|key| !still.contains(key)).collect();
    Ok(Response::new(&SyncReceipt { status: ReceiptStatus::Completed, installed, missing: still }))
}

/// The declared packages and global tools that are not installed, on managers this machine has.
fn missing(machine: &Machine) -> Result<(Vec<String>, Vec<String>), Failure> {
    let mise = &machine.environment.mise;
    let packages = mise
        .package_states()?
        .into_iter()
        .filter(|state| state.available && state.desired == "present" && state.state != "installed")
        .map(|state| state.key)
        .collect();
    let tools = mise
        .tool_states()?
        .into_iter()
        .filter(|tool| !tool.installed)
        .map(|tool| tool.key)
        .collect();
    Ok((packages, tools))
}
