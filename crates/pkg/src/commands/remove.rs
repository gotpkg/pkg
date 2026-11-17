//! `pkg remove`: take packages out of the mise config and uninstall them.
//!
//! mise has no command that uninstalls one Homebrew package. `mise bootstrap packages prune`
//! uninstalls every package no config declares, so pkg removes the declarations and prunes, but
//! only when the prune would remove nothing else: it checks with a dry run before touching the
//! config, and refuses when other undeclared packages are already waiting to be pruned.

use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::json;

use super::common::{Machine, PACKAGES, ReceiptStatus, absolute, for_people, join_words};
use crate::NAME;
use crate::declarations::remove_package;
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::mise::{ConfigTarget, Mise, removals};
use crate::package::Package;
use crate::registry::{CommandSpec, DRY_RUN, Effect, Example, Requirement};
use crate::response::{RenderText, Response};

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
        PRECONDITION_FAILED. --dry-run makes the same checks and changes nothing.",
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

/// The outcome of `pkg remove`.
#[derive(Serialize, JsonSchema)]
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

#[derive(Serialize, JsonSchema)]
pub struct Removed {
    pub package: String,
    /// The config file it was declared in, symlinks resolved.
    pub config: String,
}

#[derive(Serialize, JsonSchema)]
pub struct LeftInstalled {
    pub package: String,
    pub reason: String,
}

impl RenderText for RemoveReceipt {
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
}

/// The managers whose packages a prune uninstalls, with the arguments that select them. Casks
/// that update themselves cannot be proven unchanged, so their prune is forced, as
/// `brew uninstall` would be.
const PRUNABLE: &[(&str, &[&str])] =
    &[("brew", &["--manager", "brew"]), ("brew-cask", &["--manager", "brew-cask", "--force"])];

fn remove(invocation: &Invocation) -> Result<Response, Failure> {
    let machine = Machine::load(invocation)?;
    let packages = machine.packages(invocation, "packages");
    let mut targets: Vec<(Package, ConfigTarget)> = Vec::new();
    for package in packages {
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
                reason: format!("mise cannot uninstall {manager}: packages; remove it yourself"),
            })
        })
        .collect();

    if invocation.dry_run() {
        let uninstalled = targets
            .iter()
            .filter(|(package, _)| package.is_tool() || unprunable(package).is_none())
            .map(|(package, _)| package.key.clone())
            .collect();
        return Ok(Response::new(&RemoveReceipt {
            status: ReceiptStatus::DryRun,
            removed,
            uninstalled,
            left_installed,
        }));
    }

    for (package, target) in &targets {
        if package.is_tool() {
            let path = target.path.display().to_string();
            mise.run(&["unuse", "-p", &path, &package.key])?;
        } else {
            remove_package(&target.path, &package.key)?;
        }
    }
    let mut uninstalled: Vec<(String, Option<String>)> = Vec::new();
    for (_, selection) in &managers {
        let plan = removals(&mise.query(&prune_args("--dry-run", selection))?.combined());
        if !plan.is_empty() {
            mise.run(&prune_args("--yes", selection))?;
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
    Ok(Response::new(&RemoveReceipt {
        status: ReceiptStatus::Completed,
        removed,
        uninstalled,
        left_installed,
    }))
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
    mise: &Mise,
    managers: &[(&str, &[&str])],
    targets: &[(Package, ConfigTarget)],
) -> Result<(), Failure> {
    let mut undeclared: Vec<String> = Vec::new();
    for (_, selection) in managers {
        let output = mise.query(&prune_args("--dry-run", selection))?;
        undeclared.extend(removals(&output.combined()).into_iter().map(|(key, _)| key));
    }
    if undeclared.is_empty() {
        return Ok(());
    }
    let named: Vec<String> = targets.iter().map(|(package, _)| package.key.clone()).collect();
    let mut failure = Failure::precondition(format!(
        "removing {} would also uninstall {}, which no config declares",
        join_words(&named),
        join_words(&undeclared)
    ))
    .with_details(json!({ "packages": named, "undeclared": undeclared }))
    .suggest(format!("See what a prune removes: {NAME} prune --dry-run"))
    .suggest(format!("Uninstall them first: {NAME} prune"));
    for package in undeclared.iter().take(3) {
        failure = failure.suggest(format!("Or keep {package}: {NAME} install {package}"));
    }
    Err(failure)
}

#[cfg(test)]
mod tests {
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
}
