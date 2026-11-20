//! `pkg update`: install newer versions of what the config declares.

use pkg_core::{ReceiptStatus, UpdateReceipt, UpdateRequest};
use schemars::schema_for;

use super::common::{execute, table};
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::registry::{
    ArgumentKind, ArgumentSpec, CommandSpec, DRY_RUN, Effect, Example, Requirement, ValueKind,
};
use crate::response::{RenderOutput, Response};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "update",
    path: &["update"],
    aliases: &["up"],
    summary: "Update installed packages to their newest versions",
    description: "Updates the named packages, or with none mise itself and then every installed \
        package and global tool: `mise self-update`, then `mise bootstrap packages upgrade` for \
        Homebrew, the App Store, and the other package managers, and `mise upgrade --global` for \
        tools, which include pkg once pkg is handed to mise. A mise that a package manager \
        installed is left to it and listed under `skipped`; when `mise self-update` fails, \
        nothing else changes. Packages that are declared but not installed are left to \
        `pkg sync`. The receipt lists each package whose installed version changed, mise first. \
        --dry-run lists what would be updated, as `pkg outdated` does, and changes nothing. \
        mise's progress goes to stderr.",
    arguments: &[
        ArgumentSpec {
            name: "packages",
            kind: ArgumentKind::Positional,
            value: ValueKind::Package,
            required: false,
            default: None,
            multiple: true,
            summary: "Only these packages, such as `brew:wget` or `ripgrep`; everything when absent",
        },
        DRY_RUN,
    ],
    effect: Effect::Modifies("installed software"),
    requires: &[Requirement::Mise, Requirement::Network],
    dry_run: true,
    idempotent: true,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::NotFound,
        ErrorCode::PreconditionFailed,
        ErrorCode::MiseFailed,
    ],
    examples: &[
        Example { summary: "Preview updating everything", args: &["update", "--dry-run"] },
        Example { summary: "Update one formula", args: &["update", "brew:wget"] },
    ],
    synonyms: &["upgrade", "newest", "latest", "bump", "refresh", "outdated", "self-update"],
    output_schema: || schema_for!(UpdateReceipt),
    run: update,
}];

impl RenderOutput for UpdateReceipt {
    fn render_text(&self) -> String {
        let dry = self.status == ReceiptStatus::DryRun;
        let mut text = if self.updated.is_empty() {
            "Everything is up to date.\n".to_string()
        } else {
            let rows: Vec<Vec<String>> = self
                .updated
                .iter()
                .map(|item| {
                    let from = item.from.clone().unwrap_or_else(|| "?".into());
                    let to = item.to.clone().unwrap_or_else(|| "?".into());
                    vec![item.package.clone(), format!("{from} -> {to}")]
                })
                .collect();
            format!("{}\n{}", if dry { "Would update:" } else { "Updated:" }, table(&rows))
        };
        if !self.skipped.is_empty() {
            text.push_str("mise leaves these alone:\n");
            let rows: Vec<Vec<String>> = self
                .skipped
                .iter()
                .map(|skip| vec![skip.package.clone(), skip.reason.clone()])
                .collect();
            text.push_str(&table(&rows));
        }
        if dry {
            text.push_str("Dry run: nothing was updated.\n");
        }
        text
    }
}

fn update(invocation: &Invocation) -> Result<Response, Failure> {
    let request =
        UpdateRequest { packages: invocation.strings("packages"), mode: invocation.mode() };
    execute(invocation, |client, progress| client.update(request, progress))
}
