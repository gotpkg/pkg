//! Installing newer versions of what the config declares.

use schemars::JsonSchema;
use serde::Serialize;

use crate::common::{Machine, ReceiptStatus};
use crate::outdated::{self, Skipped};
use crate::package::Package;
use crate::requests::validate_packages;
use crate::{Client, Error, ErrorDetails, Outcome, Progress, Recovery, UpgradeRequest};

/// The outcome of `pkg upgrade`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct UpgradeReceipt {
    pub status: ReceiptStatus,
    /// Packages whose version changed (for `dry-run`: would change).
    pub upgraded: Vec<Upgraded>,
    /// For `dry-run`, packages mise will not upgrade, with its reason; empty otherwise.
    pub skipped: Vec<Skipped>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Upgraded {
    pub package: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl Client {
    /// Upgrades the named packages, or with none every installed package and global tool:
    /// `mise bootstrap packages upgrade` for bootstrap packages and `mise upgrade --global` for
    /// tools. Declared packages that are not installed are left to `sync`. The receipt lists
    /// each package whose installed version changed; a preview lists what `outdated` reports.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` when a name is not mise syntax, `NotFound` when no config declares a
    /// named package, `PreconditionFailed` when mise cannot start or a config cannot be read, and
    /// `MiseFailed` when mise fails.
    pub fn upgrade(
        &mut self,
        request: UpgradeRequest,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<UpgradeReceipt>, Error> {
        validate_packages(&request.packages, false)?;
        let machine = Machine::load(&self.settings)?;
        let packages = machine.packages(&request.packages);
        for package in &packages {
            if machine.declared_in(package).is_none() {
                return Err(Error::not_found(format!("{package} is not in your mise config"))
                    .with_details(ErrorDetails {
                        package: Some(package.key.clone()),
                        ..Default::default()
                    })
                    .recover(Recovery::Install(package.key.clone())));
            }
        }

        if request.mode.is_dry_run() {
            let (report, warnings) = outdated::find(&machine, &packages)?;
            let upgraded = report
                .items
                .into_iter()
                .map(|item| Upgraded {
                    package: item.package,
                    from: item.installed,
                    to: Some(item.latest),
                })
                .collect();
            let receipt =
                UpgradeReceipt { status: ReceiptStatus::DryRun, upgraded, skipped: report.skipped };
            return Ok(Outcome::with_warnings(receipt, warnings, progress));
        }

        let mise = &machine.environment.mise;
        let before = versions(&machine)?;
        let named: Vec<&str> = packages
            .iter()
            .filter(|package| !package.is_tool())
            .map(|package| package.key.as_str())
            .collect();
        let tools: Vec<&str> = packages
            .iter()
            .filter(|package| package.is_tool())
            .map(|package| package.key.as_str())
            .collect();
        if packages.is_empty() || !named.is_empty() {
            let mut args = vec!["bootstrap", "packages", "upgrade", "--yes"];
            args.extend(&named);
            mise.run(&args, progress)?;
        }
        if packages.is_empty() || !tools.is_empty() {
            let mut args = vec!["upgrade", "--global"];
            args.extend(&tools);
            mise.run(&args, progress)?;
        }
        let after = versions(&machine)?;
        let upgraded = after
            .into_iter()
            .filter_map(|(key, to)| {
                let from = before
                    .iter()
                    .find(|(old, _)| *old == key)
                    .and_then(|(_, version)| version.clone());
                (to.is_some()
                    && from != to
                    && (packages.is_empty()
                        || packages.iter().any(|package: &Package| package.key == key)))
                .then_some(Upgraded { package: key, from, to })
            })
            .collect();
        Ok(Outcome::new(UpgradeReceipt {
            status: ReceiptStatus::Completed,
            upgraded,
            skipped: Vec::new(),
        }))
    }
}

/// Every declared package's and global tool's installed version.
fn versions(machine: &Machine<'_>) -> Result<Vec<(String, Option<String>)>, Error> {
    let mise = &machine.environment.mise;
    let mut versions: Vec<(String, Option<String>)> =
        mise.package_states()?.into_iter().map(|state| (state.key, state.installed)).collect();
    versions.extend(
        mise.tool_states()?
            .into_iter()
            .map(|tool| (tool.key, tool.installed.then_some(tool.version).flatten())),
    );
    Ok(versions)
}
