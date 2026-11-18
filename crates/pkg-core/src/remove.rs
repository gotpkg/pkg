//! Taking packages out of the mise config and uninstalling them.
//!
//! mise has no command that uninstalls one Homebrew package. `mise bootstrap packages prune`
//! uninstalls every package no config declares, so the declarations are removed and then
//! pruned, but only when the prune would remove nothing else: a dry run checks before the config
//! changes, and the removal is refused when other undeclared packages are already waiting to be
//! pruned.

use std::ffi::OsStr;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Serialize;

use crate::common::{Machine, ReceiptStatus, absolute, join_words};
use crate::declarations::remove_package;
use crate::mise::{ConfigTarget, Mise, removals};
use crate::package::Package;
use crate::requests::validate_packages;
use crate::{Client, Error, ErrorDetails, Outcome, Progress, Recovery, RemoveRequest};

/// The outcome of `pkg remove`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoveReceipt {
    pub status: ReceiptStatus,
    /// The declarations taken out of the config (for `dry-run`: that would be).
    pub removed: Vec<Removed>,
    /// Homebrew packages uninstalled, dependencies included, as `manager:name@version`; for
    /// `dry-run`, the packages you named, since their dependencies are known only after the
    /// config changes.
    pub uninstalled: Vec<String>,
    /// Packages that stay installed, with the reason.
    pub left_installed: Vec<LeftInstalled>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Removed {
    pub package: String,
    /// The config file it was declared in, symlinks resolved.
    pub config: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct LeftInstalled {
    pub package: String,
    pub reason: String,
}

/// The managers whose packages a prune uninstalls, with the arguments that select them. Casks
/// that update themselves cannot be proven unchanged, so their prune is forced, as
/// `brew uninstall` would be.
const PRUNABLE: &[(&str, &[&str])] =
    &[("brew", &["--manager", "brew"]), ("brew-cask", &["--manager", "brew-cask", "--force"])];

impl Client {
    /// Takes each package out of the config file mise writes to and uninstalls it: Homebrew
    /// formulae and casks by a prune that must remove nothing besides the named packages and
    /// their unneeded dependencies, tools with `mise unuse`. Packages of managers mise cannot
    /// uninstall are only undeclared and reported under `left_installed`.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` when no package is named or a name is not mise syntax, `NotFound` when
    /// no config declares a package, `PreconditionFailed` when a package is declared outside
    /// mise's write target or other undeclared packages would be pruned too (both before any
    /// change), and `MiseFailed` when mise fails.
    pub fn remove(
        &mut self,
        request: RemoveRequest,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<RemoveReceipt>, Error> {
        validate_packages(&request.packages, true)?;
        let machine = Machine::load(&self.settings)?;
        let mut targets: Vec<(Package, ConfigTarget)> = Vec::new();
        for package in machine.packages(&request.packages) {
            let target = machine.editable_declaration(&package)?;
            targets.push((package, target));
        }
        let mise = &machine.environment.mise;
        let managers: Vec<(&str, &[&str])> = PRUNABLE
            .iter()
            .filter(|(manager, _)| {
                targets.iter().any(|(package, _)| package.manager() == Some(*manager))
            })
            .copied()
            .collect();
        refuse_unrelated_prunes(mise, &managers, &targets)?;

        let removed: Vec<Removed> = targets
            .iter()
            .map(|(package, target)| Removed {
                package: package.key.clone(),
                config: absolute(&target.path),
            })
            .collect();
        let mut left_installed: Vec<LeftInstalled> = targets
            .iter()
            .filter_map(|(package, _)| {
                unprunable(package).map(|manager| LeftInstalled {
                    package: package.key.clone(),
                    reason: format!(
                        "mise cannot uninstall {manager}: packages; remove it yourself"
                    ),
                })
            })
            .collect();

        if request.mode.is_dry_run() {
            let uninstalled = targets
                .iter()
                .filter(|(package, _)| package.is_tool() || unprunable(package).is_none())
                .map(|(package, _)| package.key.clone())
                .collect();
            return Ok(Outcome::new(RemoveReceipt {
                status: ReceiptStatus::DryRun,
                removed,
                uninstalled,
                left_installed,
            }));
        }

        for (index, (package, target)) in targets.iter().enumerate() {
            let outcome = if package.is_tool() {
                mise.run(
                    &[
                        OsStr::new("unuse"),
                        OsStr::new("-p"),
                        target.path.as_os_str(),
                        OsStr::new(&package.key),
                    ],
                    progress,
                )
                .map(drop)
            } else {
                remove_package(&target.path, &package.key)
            };
            outcome.map_err(|error| after_removals(error, &removed[..index]))?;
        }
        let mut uninstalled: Vec<(String, Option<String>)> = Vec::new();
        for (_, selection) in &managers {
            let output = mise
                .query(&prune_args("--dry-run", selection))
                .map_err(|error| after_removals(error, &removed))?;
            let plan = removals(&output.combined());
            if !plan.is_empty() {
                mise.run(&prune_args("--yes", selection), progress)
                    .map_err(|error| after_removals(error, &removed))?;
            }
            uninstalled.extend(plan);
        }
        for (package, _) in &targets {
            let pruned = uninstalled.iter().any(|(key, _)| *key == package.key);
            if !package.is_tool() && unprunable(package).is_none() && !pruned {
                left_installed.push(LeftInstalled {
                    package: package.key.clone(),
                    reason: "a package your config declares depends on it".into(),
                });
            }
        }
        let mut uninstalled: Vec<String> = uninstalled
            .into_iter()
            .map(|(key, version)| version.map_or(key.clone(), |version| format!("{key}@{version}")))
            .collect();
        uninstalled.extend(
            targets
                .iter()
                .filter(|(package, _)| package.is_tool())
                .map(|(package, _)| package.key.clone()),
        );
        Ok(Outcome::new(RemoveReceipt {
            status: ReceiptStatus::Completed,
            removed,
            uninstalled,
            left_installed,
        }))
    }
}

fn after_removals(mut error: Error, removed: &[Removed]) -> Error {
    error.details.removed = Some(removed.iter().map(|item| item.package.clone()).collect());
    error
}

/// The manager of a bootstrap package mise cannot uninstall; `None` for tools and Homebrew.
fn unprunable(package: &Package) -> Option<&str> {
    package.manager().filter(|manager| !PRUNABLE.iter().any(|(prunable, _)| prunable == manager))
}

fn prune_args<'a>(mode: &'a str, selection: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["bootstrap", "packages", "prune", mode];
    args.extend_from_slice(selection);
    args
}

/// Fails when a prune would already remove packages before any declaration changes: removing
/// these packages would uninstall those too.
fn refuse_unrelated_prunes(
    mise: &Mise<'_>,
    managers: &[(&str, &[&str])],
    targets: &[(Package, ConfigTarget)],
) -> Result<(), Error> {
    let mut undeclared: Vec<String> = Vec::new();
    for (_, selection) in managers {
        let output = mise.query(&prune_args("--dry-run", selection))?;
        undeclared.extend(removals(&output.combined()).into_iter().map(|(key, _)| key));
    }
    if undeclared.is_empty() {
        return Ok(());
    }
    let named: Vec<String> = targets.iter().map(|(package, _)| package.key.clone()).collect();
    let mut error = Error::precondition(format!(
        "removing {} would also uninstall {}, which no config declares",
        join_words(&named),
        join_words(&undeclared)
    ))
    .recover(Recovery::PreviewPrune)
    .recover(Recovery::Prune);
    for package in undeclared.iter().take(3) {
        error = error.recover(Recovery::Keep(package.clone()));
    }
    Err(error.with_details(ErrorDetails {
        packages: Some(named),
        undeclared: Some(undeclared),
        ..Default::default()
    }))
}
