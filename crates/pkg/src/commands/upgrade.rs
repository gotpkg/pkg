//! `pkg upgrade`: install newer versions of what the config declares.

use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::json;

use super::common::{Machine, ReceiptStatus, table};
use super::outdated::{self, Skipped};
use crate::NAME;
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::package::Package;
use crate::registry::{
    ArgumentKind, ArgumentSpec, CommandSpec, DRY_RUN, Effect, Example, Requirement, ValueKind,
};
use crate::response::{RenderText, Response};

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

/// The outcome of `pkg upgrade`.
#[derive(Serialize, JsonSchema)]
pub struct UpgradeReceipt {
    pub status: ReceiptStatus,
    /// Packages whose version changed (for `dry-run`: would change).
    pub upgraded: Vec<Upgraded>,
    /// For `dry-run`, packages mise will not upgrade, with its reason; empty otherwise.
    pub skipped: Vec<Skipped>,
}

#[derive(Serialize, JsonSchema)]
pub struct Upgraded {
    pub package: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl RenderText for UpgradeReceipt {
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
    let machine = Machine::load(invocation)?;
    let packages = machine.packages(invocation, "packages");
    for package in &packages {
        if machine.declared_in(package).is_none() {
            return Err(Failure::not_found(format!("{package} is not in your mise config"))
                .with_details(json!({ "package": package.key }))
                .suggest(format!("{NAME} install {package}")));
        }
    }

    if invocation.dry_run() {
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
        let mut response = Response::new(&UpgradeReceipt {
            status: ReceiptStatus::DryRun,
            upgraded,
            skipped: report.skipped,
        });
        for warning in warnings {
            response = response.warn(warning);
        }
        return Ok(response);
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
        mise.run(&args)?;
    }
    if packages.is_empty() || !tools.is_empty() {
        let mut args = vec!["upgrade", "--global"];
        args.extend(&tools);
        mise.run(&args)?;
    }
    let after = versions(&machine)?;
    let upgraded = after
        .into_iter()
        .filter_map(|(key, to)| {
            let from =
                before.iter().find(|(old, _)| *old == key).and_then(|(_, version)| version.clone());
            (to.is_some()
                && from != to
                && (packages.is_empty()
                    || packages.iter().any(|package: &Package| package.key == key)))
            .then_some(Upgraded { package: key, from, to })
        })
        .collect();
    Ok(Response::new(&UpgradeReceipt {
        status: ReceiptStatus::Completed,
        upgraded,
        skipped: Vec::new(),
    }))
}

/// Every declared package's and global tool's installed version.
fn versions(machine: &Machine) -> Result<Vec<(String, Option<String>)>, Failure> {
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
