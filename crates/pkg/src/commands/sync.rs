//! `pkg sync`: install everything the config declares that is missing, as `mise bootstrap` does
//! for packages and tools alone.

use pkg_core::{ReceiptStatus, SyncReceipt, SyncRequest};
use schemars::schema_for;

use super::common::{MISE_ERRORS, execute, join_words};
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::registry::{CommandSpec, DRY_RUN, Effect, Example, Requirement};
use crate::response::{RenderOutput, Response};

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

impl RenderOutput for SyncReceipt {
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
    let request = SyncRequest { mode: invocation.mode() };
    execute(invocation, |client, progress| client.sync(request, progress))
}
