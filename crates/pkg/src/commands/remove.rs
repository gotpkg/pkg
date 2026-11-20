//! `pkg remove`: take packages out of the mise config and uninstall them.

use pkg_core::{ReceiptStatus, RemoveReceipt, RemoveRequest};
use schemars::schema_for;
use serde_json::{Value, json};

use super::common::{PACKAGES, execute_change, for_people, join_words};
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::progress::shows_display;
use crate::registry::{CommandSpec, DRY_RUN, Effect, Example, Requirement};
use crate::response::{RenderOutput, Response};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "remove",
    path: &["remove"],
    aliases: &["uninstall", "rm"],
    summary: "Uninstall packages and take them out of your mise config",
    description: "Takes each package out of the config file mise writes to and uninstalls it. \
        Homebrew formulae and casks are uninstalled by `mise bootstrap packages prune`, which \
        removes every package no config declares, together with dependencies nothing else \
        needs; pkg first checks that the prune would remove nothing besides what you named, and \
        refuses with PRECONDITION_FAILED, changing nothing, when other undeclared packages are \
        installed (`details.undeclared`; see `pkg prune --dry-run`). Tools are removed with \
        `mise unuse`, which also uninstalls versions no other config uses. mise cannot uninstall \
        other managers' packages, such as App Store apps; pkg takes them out of the config and \
        lists them under `leftInstalled`. pkg edits only mise's write target: a package declared \
        in another file, such as a config shared with other machines, fails with \
        PRECONDITION_FAILED. --dry-run makes the same checks and changes nothing. With text \
        output on a terminal, pkg shows its progress as a live display on stderr; otherwise \
        mise's progress lines go to stderr as they arrive.",
    arguments: &[PACKAGES, DRY_RUN],
    effect: Effect::Modifies("installed software and your global mise config"),
    requires: &[Requirement::Mise],
    dry_run: true,
    idempotent: false,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::NotFound,
        ErrorCode::PreconditionFailed,
        ErrorCode::MiseFailed,
    ],
    examples: &[
        Example {
            summary: "Preview removing a formula, changing nothing",
            args: &["remove", "brew:wget", "--dry-run"],
        },
        Example { summary: "Remove a formula", args: &["remove", "brew:wget"] },
    ],
    synonyms: &["uninstall", "delete", "drop", "undeclare", "erase", "rm", "get rid"],
    output_schema: || schema_for!(RemoveReceipt),
    run: remove,
}];

impl RenderOutput for RemoveReceipt {
    fn render_text(&self) -> String {
        let dry = self.status == ReceiptStatus::DryRun;
        let mut text = String::new();
        for removed in &self.removed {
            let verb = if dry { "Would remove" } else { "Removed" };
            text.push_str(&format!(
                "{verb} {} from {}\n",
                removed.package,
                for_people(&removed.config)
            ));
        }
        if !self.uninstalled.is_empty() {
            let verb = if dry { "Would uninstall" } else { "Uninstalled" };
            text.push_str(&format!("{verb} {}\n", join_words(&self.uninstalled)));
        }
        for left in &self.left_installed {
            text.push_str(&format!("{} stays installed: {}\n", left.package, left.reason));
        }
        if dry {
            text.push_str("Dry run: nothing was uninstalled or written.\n");
        }
        text
    }

    fn render_json(&self) -> Value {
        let removed: Vec<Value> = self
            .removed
            .iter()
            .map(|removed| {
                json!({ "package": removed.package, "config": removed.config.to_string_lossy() })
            })
            .collect();
        json!({
            "status": self.status,
            "removed": removed,
            "uninstalled": self.uninstalled,
            "leftInstalled": self.left_installed,
        })
    }
}

fn remove(invocation: &Invocation) -> Result<Response, Failure> {
    let request =
        RemoveRequest { packages: invocation.strings("packages"), mode: invocation.mode() };
    let live = shows_display(invocation.output);
    execute_change(invocation, live, |client, progress| client.remove(request, progress))
}

#[cfg(test)]
mod tests {
    use pkg_core::{LeftInstalled, Removed};

    use super::*;

    #[test]
    fn text_lists_what_was_removed_uninstalled_and_kept() {
        let receipt = RemoveReceipt {
            status: ReceiptStatus::Completed,
            removed: vec![
                Removed { package: "brew:wget".into(), config: "/x/config.toml".into() },
                Removed { package: "mas:1".into(), config: "/x/config.toml".into() },
            ],
            uninstalled: vec!["brew:wget@1.25.0".into(), "brew:libidn2@2.3.8".into()],
            left_installed: vec![LeftInstalled { package: "mas:1".into(), reason: "why".into() }],
        };
        assert_eq!(
            receipt.render_text(),
            "Removed brew:wget from /x/config.toml\nRemoved mas:1 from /x/config.toml\nUninstalled brew:wget@1.25.0 and brew:libidn2@2.3.8\nmas:1 stays installed: why\n"
        );
    }

    #[test]
    fn json_matches_the_receipt_fields_in_order() {
        let receipt = RemoveReceipt {
            status: ReceiptStatus::DryRun,
            removed: vec![Removed { package: "mas:1".into(), config: "/x/config.toml".into() }],
            uninstalled: Vec::new(),
            left_installed: vec![LeftInstalled { package: "mas:1".into(), reason: "why".into() }],
        };
        assert_eq!(receipt.render_json().to_string(), serde_json::to_string(&receipt).unwrap());
    }
}
