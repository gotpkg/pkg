//! `pkg upgrade`: install newer versions of what the config declares.

use pkg_core::{ReceiptStatus, UpgradeReceipt, UpgradeRequest};
use schemars::schema_for;

use super::common::{execute, table};
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::registry::{
    ArgumentKind, ArgumentSpec, CommandSpec, DRY_RUN, Effect, Example, Requirement, ValueKind,
};
use crate::response::{RenderOutput, Response};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "upgrade",
    path: &["upgrade"],
    aliases: &["up"],
    summary: "Upgrade installed packages to their newest versions",
    description: "Upgrades the named packages, or with none every installed package and global \
        tool: `mise bootstrap packages upgrade` for Homebrew, the App Store, and the other \
        package managers, and `mise upgrade --global` for tools. Packages that are declared but \
        not installed are left to `pkg sync`. The receipt lists each package whose installed \
        version changed. --dry-run lists what would be upgraded, as `pkg outdated` does, and \
        changes nothing. mise's progress goes to stderr.",
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
        Example { summary: "Preview upgrading everything", args: &["upgrade", "--dry-run"] },
        Example { summary: "Upgrade one formula", args: &["upgrade", "brew:wget"] },
    ],
    synonyms: &["update", "newest", "latest", "bump", "refresh", "outdated"],
    output_schema: || schema_for!(UpgradeReceipt),
    run: upgrade,
}];

impl RenderOutput for UpgradeReceipt {
    fn render_text(&self) -> String {
        let dry = self.status == ReceiptStatus::DryRun;
        let mut text = if self.upgraded.is_empty() {
            "Everything is up to date.\n".to_string()
        } else {
            let rows: Vec<Vec<String>> = self
                .upgraded
                .iter()
                .map(|item| {
                    let from = item.from.clone().unwrap_or_else(|| "?".into());
                    let to = item.to.clone().unwrap_or_else(|| "?".into());
                    vec![item.package.clone(), format!("{from} -> {to}")]
                })
                .collect();
            format!("{}\n{}", if dry { "Would upgrade:" } else { "Upgraded:" }, table(&rows))
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
            text.push_str("Dry run: nothing was upgraded.\n");
        }
        text
    }
}

fn upgrade(invocation: &Invocation) -> Result<Response, Failure> {
    let request =
        UpgradeRequest { packages: invocation.strings("packages"), mode: invocation.mode() };
    execute(invocation, |client, progress| client.upgrade(request, progress))
}
