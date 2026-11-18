//! Uninstalling Homebrew packages that no config declares.

use schemars::JsonSchema;
use serde::Serialize;

use crate::common::{Machine, ReceiptStatus};
use crate::mise::removals;
use crate::{Client, Error, Outcome, Progress, PruneRequest};

/// The outcome of `pkg prune`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct PruneReceipt {
    pub status: ReceiptStatus,
    /// What was uninstalled (for `dry-run`: would be), as `manager:name@version`.
    pub uninstalled: Vec<String>,
}

impl Client {
    /// Uninstalls every formula that no config declares and no declared package needs, and every
    /// cask mise installed that no config declares, with `mise bootstrap packages prune`. Casks
    /// that cannot be proven unchanged are kept unless `force` is set.
    ///
    /// # Errors
    ///
    /// `PreconditionFailed` when mise cannot start or a config cannot be read, and `MiseFailed`
    /// when mise fails.
    pub fn prune(
        &mut self,
        request: PruneRequest,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<PruneReceipt>, Error> {
        let machine = Machine::load(&self.settings)?;
        let mise = &machine.environment.mise;
        let dry_run = request.mode.is_dry_run();
        let mut uninstalled = Vec::new();
        for manager in ["brew", "brew-cask"] {
            let mut selection = vec!["--manager", manager];
            if request.force && manager == "brew-cask" {
                selection.push("--force");
            }
            let mut args = vec!["bootstrap", "packages", "prune", "--dry-run"];
            args.extend(&selection);
            let plan = removals(&mise.query(&args)?.combined());
            if !plan.is_empty() && !dry_run {
                args[3] = "--yes";
                mise.run(&args, progress)?;
            }
            uninstalled.extend(plan.into_iter().map(|(key, version)| {
                version.map_or(key.clone(), |version| format!("{key}@{version}"))
            }));
        }
        let status = if dry_run { ReceiptStatus::DryRun } else { ReceiptStatus::Completed };
        Ok(Outcome::new(PruneReceipt { status, uninstalled }))
    }
}
