//! Installing newer versions of mise and of what the config declares.

use schemars::JsonSchema;
use serde::Serialize;

use crate::common::{Machine, ReceiptStatus};
use crate::mise::{MISE, Mise, PACKAGE_MANAGER_REASON};
use crate::outdated::{self, Skipped};
use crate::package::Package;
use crate::requests::validate_packages;
use crate::{Client, Error, ErrorDetails, Outcome, Progress, Recovery, Step, UpdateRequest};

/// The outcome of `pkg update`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct UpdateReceipt {
    pub status: ReceiptStatus,
    /// Packages whose version changed (for `dry-run`: would change), mise first.
    pub updated: Vec<Updated>,
    /// What is left alone, with the reason: mise when a package manager installed it, and for
    /// `dry-run` the packages mise will not update.
    pub skipped: Vec<Skipped>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Updated {
    pub package: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl Client {
    /// Updates the named packages, or with none mise itself and then every installed package and
    /// global tool: `mise self-update`, `mise bootstrap packages upgrade` for bootstrap packages,
    /// and `mise upgrade --global` for tools. A mise that a package manager installed is left to
    /// it and listed in `skipped`. Declared packages that are not installed are left to `sync`.
    /// The receipt lists each package whose installed version changed; a preview lists what
    /// `outdated` reports.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` when a name is not mise syntax, `NotFound` when no config declares a
    /// named package, `PreconditionFailed` when mise cannot start or a config cannot be read, and
    /// `MiseFailed` when mise fails. A failed `mise self-update` stops the update before any
    /// package changes.
    pub fn update(
        &mut self,
        request: UpdateRequest,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<UpdateReceipt>, Error> {
        validate_packages(&request.packages, false)?;
        let mut receipt = UpdateReceipt {
            status: ReceiptStatus::Completed,
            updated: Vec::new(),
            skipped: Vec::new(),
        };
        if request.packages.is_empty() && !request.mode.is_dry_run() {
            progress(Progress::Step(Step::UpdateMise));
            match update_mise(&Mise::new(&self.settings), progress)? {
                MiseUpdate::Updated(updated) => receipt.updated.push(updated),
                MiseUpdate::Skipped(skipped) => receipt.skipped.push(skipped),
                MiseUpdate::Unchanged => {}
            }
        }

        progress(Progress::Step(Step::ReadInstalled));
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
            receipt.status = ReceiptStatus::DryRun;
            receipt.updated = report
                .items
                .into_iter()
                .map(|item| Updated {
                    package: item.package,
                    from: item.installed,
                    to: Some(item.latest),
                })
                .collect();
            receipt.skipped = report.skipped;
            return Ok(Outcome::with_warnings(receipt, warnings, progress));
        }

        receipt.updated.extend(update_packages(&machine, &packages, progress)?);
        Ok(Outcome::new(receipt))
    }
}

enum MiseUpdate {
    Updated(Updated),
    Skipped(Skipped),
    Unchanged,
}

/// Runs `mise self-update`, unless a package manager owns this mise and mise knows of a newer
/// release, which the package manager has to install.
fn update_mise(
    mise: &Mise<'_>,
    progress: &mut dyn FnMut(Progress<'_>),
) -> Result<MiseUpdate, Error> {
    let before = mise.release()?;
    let skipped = || Skipped { package: MISE.into(), reason: PACKAGE_MANAGER_REASON.into() };
    if !mise.updates_itself() {
        return Ok(before
            .newer_available()
            .map_or(MiseUpdate::Unchanged, |_| MiseUpdate::Skipped(skipped())));
    }
    if let Err(mut error) = mise.run(&["self-update", "--yes"], progress) {
        // A build without the self-update feature says so only when asked to update.
        if error.message.contains("self-update") && error.message.contains("disabled") {
            return Ok(MiseUpdate::Skipped(skipped()));
        }
        error.message = format!("mise could not update itself: {}", error.message);
        return Err(error);
    }
    let after = mise.release()?;
    if after.version == before.version {
        return Ok(MiseUpdate::Unchanged);
    }
    Ok(MiseUpdate::Updated(Updated {
        package: MISE.into(),
        from: Some(before.version),
        to: Some(after.version),
    }))
}

/// Upgrades the bootstrap packages and global tools in `packages`, or every one when it is empty,
/// and reports each version that changed.
fn update_packages(
    machine: &Machine<'_>,
    packages: &[Package],
    progress: &mut dyn FnMut(Progress<'_>),
) -> Result<Vec<Updated>, Error> {
    let mise = &machine.environment.mise;
    let before = versions(machine)?;
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
        progress(Progress::Step(Step::UpdatePackages));
        let mut args = vec!["bootstrap", "packages", "upgrade", "--yes"];
        args.extend(&named);
        mise.run(&args, progress)?;
    }
    if packages.is_empty() || !tools.is_empty() {
        progress(Progress::Step(Step::UpdateTools));
        let mut args = vec!["upgrade", "--global"];
        args.extend(&tools);
        mise.run(&args, progress)?;
    }
    progress(Progress::Step(Step::ReadResult));
    let after = versions(machine)?;
    Ok(after
        .into_iter()
        .filter_map(|(key, to)| {
            let from =
                before.iter().find(|(old, _)| *old == key).and_then(|(_, version)| version.clone());
            (to.is_some()
                && from != to
                && (packages.is_empty() || packages.iter().any(|package| package.key == key)))
            .then_some(Updated { package: key, from, to })
        })
        .collect())
}

/// Every declared package's and global tool's installed version.
fn versions(machine: &Machine<'_>) -> Result<Vec<(String, Option<String>)>, Error> {
    let mise = &machine.environment.mise;
    let mut versions: Vec<(String, Option<String>)> =
        machine.package_states()?.into_iter().map(|state| (state.key, state.installed)).collect();
    versions.extend(
        mise.tool_states()?
            .into_iter()
            .map(|tool| (tool.key, tool.installed.then_some(tool.version).flatten())),
    );
    Ok(versions)
}
