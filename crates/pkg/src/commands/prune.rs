//! `pkg prune`: uninstall Homebrew packages that no config declares.

use pkg_core::{PruneReceipt, PruneRequest, ReceiptStatus};
use schemars::schema_for;

use super::common::{MISE_ERRORS, execute, join_words};
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::registry::{
    ArgumentKind, ArgumentSpec, CommandSpec, DRY_RUN, Effect, Example, Requirement, ValueKind,
};
use crate::response::{RenderOutput, Response};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "prune",
    path: &["prune"],
    aliases: &[],
    summary: "Uninstall Homebrew packages no config declares",
    description: "Runs `mise bootstrap packages prune` for formulae and casks: it uninstalls every \
        formula that no config declares and no declared package needs, and every cask mise \
        installed that no config declares. Casks that update themselves or run install scripts \
        cannot be proven unchanged and are kept unless you pass --force, which removes the files \
        mise recorded but not what the cask's own scripts created. Packages that `pkg remove` \
        would otherwise take with it show up here first. --dry-run lists what would be \
        uninstalled and changes nothing.",
    arguments: &[
        ArgumentSpec {
            name: "force",
            kind: ArgumentKind::Flag,
            value: ValueKind::Boolean,
            required: false,
            default: None,
            multiple: false,
            summary: "Also uninstall casks mise cannot prove are unchanged",
        },
        DRY_RUN,
    ],
    effect: Effect::Modifies("installed software"),
    requires: &[Requirement::Mise],
    dry_run: true,
    idempotent: true,
    errors: MISE_ERRORS,
    examples: &[Example {
        summary: "See what a prune would uninstall",
        args: &["prune", "--dry-run"],
    }],
    synonyms: &["autoremove", "cleanup", "undeclared", "leftovers", "orphans", "unused", "clean"],
    output_schema: || schema_for!(PruneReceipt),
    run: prune,
}];

impl RenderOutput for PruneReceipt {
    fn render_text(&self) -> String {
        match (self.status, self.uninstalled.is_empty()) {
            (_, true) => "Nothing to prune: every installed package is declared.\n".to_string(),
            (ReceiptStatus::Completed, false) => {
                format!("Uninstalled {}\n", join_words(&self.uninstalled))
            }
            (ReceiptStatus::DryRun, false) => {
                format!(
                    "Would uninstall {}\nDry run: nothing was uninstalled.\n",
                    join_words(&self.uninstalled)
                )
            }
        }
    }
}

fn prune(invocation: &Invocation) -> Result<Response, Failure> {
    let request = PruneRequest { mode: invocation.mode(), force: invocation.flag("force") };
    execute(invocation, |client, progress| client.prune(request, progress))
}
