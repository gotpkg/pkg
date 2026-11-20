//! Uninstalling Homebrew packages that no config declares.

use schemars::JsonSchema;
use serde::Serialize;

use crate::common::{Machine, ReceiptStatus};
use crate::mise::{removals, uninstalls};
use crate::{Client, Error, Outcome, Progress, PruneRequest, Step};

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
        let managers = ["brew", "brew-cask"];
        // Reading the config belongs to finding the first manager's undeclared packages.
        progress(Progress::Step(Step::PlanPrune { manager: managers[0] }));
        let machine = Machine::load(&self.settings)?;
        let mise = &machine.environment.mise;
        let dry_run = request.mode.is_dry_run();
        let mut uninstalled = Vec::new();
        for (index, manager) in managers.into_iter().enumerate() {
            if index > 0 {
                progress(Progress::Step(Step::PlanPrune { manager }));
            }
            let mut selection = vec!["--manager", manager];
            if request.force && manager == "brew-cask" {
                selection.push("--force");
            }
            let mut args = vec!["bootstrap", "packages", "prune", "--dry-run"];
            args.extend(&selection);
            let plan = removals(&mise.query(&args)?.combined());
            if !plan.is_empty() && !dry_run {
                progress(Progress::Step(Step::Uninstall { manager }));
                progress(Progress::Plan(&uninstalls(&plan)));
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
