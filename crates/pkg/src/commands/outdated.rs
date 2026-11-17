//! `pkg outdated`: installed packages and tools with a newer version, which `upgrade` installs.

use schemars::{JsonSchema, schema_for};
use serde::Serialize;

use super::common::{Machine, table};
use crate::NAME;
use crate::catalog::Client;
use crate::catalog::app_store::AppStore;
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::mise::{OutdatedTool, Output, PackageState, upgrade_plan};
use crate::package::Package;
use crate::registry::{CommandSpec, Effect, Example, Requirement};
use crate::response::{RenderText, Response};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "outdated",
    path: &["outdated"],
    aliases: &[],
    summary: "List installed packages that have a newer version",
    description: "Lists every installed package and tool that `pkg upgrade` would update, with \
        the installed and the newest version. Homebrew formulae and casks come from \
        `mise bootstrap packages upgrade --dry-run`, which refreshes Homebrew's metadata; App \
        Store apps are compared with the App Store's current version; tools come from \
        `mise outdated --global`. Casks mise will not upgrade, such as apps that update \
        themselves, are listed under `skipped` with mise's reason. The whole list is returned at \
        once. Never changes anything.",
    arguments: &[],
    effect: Effect::Reads,
    requires: &[Requirement::Mise, Requirement::Network],
    dry_run: false,
    idempotent: true,
    errors: &[ErrorCode::PreconditionFailed, ErrorCode::MiseFailed],
    examples: &[Example { summary: "See what upgrade would update", args: &["outdated"] }],
    synonyms: &["old", "stale", "newer", "updates", "behind", "upgradable", "available"],
    output_schema: || schema_for!(OutdatedReport),
    run: outdated,
}];

/// Every outdated package, never paged.
#[derive(Default, Serialize, JsonSchema)]
pub struct OutdatedReport {
    pub items: Vec<OutdatedPackage>,
    /// Packages mise will not upgrade, with its reason.
    pub skipped: Vec<Skipped>,
}

#[derive(Clone, Serialize, JsonSchema)]
pub struct OutdatedPackage {
    pub package: String,
    pub installed: Option<String>,
    pub latest: String,
}

#[derive(Clone, Serialize, JsonSchema)]
pub struct Skipped {
    pub package: String,
    pub reason: String,
}

impl RenderText for OutdatedReport {
    fn render_text(&self) -> String {
        let mut text = if self.items.is_empty() {
            "Everything is up to date.\n".to_string()
        } else {
            let rows: Vec<Vec<String>> = self
                .items
                .iter()
                .map(|item| {
                    let installed = item.installed.clone().unwrap_or_else(|| "?".into());
                    vec![item.package.clone(), format!("{installed} -> {}", item.latest)]
                })
                .collect();
            format!("{}Upgrade with `{NAME} upgrade`.\n", table(&rows))
        };
        if !self.skipped.is_empty() {
            text.push_str("\nmise leaves these alone:\n");
            let rows: Vec<Vec<String>> = self
                .skipped
                .iter()
                .map(|skip| vec![skip.package.clone(), skip.reason.clone()])
                .collect();
            text.push_str(&table(&rows));
        }
        text
    }
}

fn outdated(invocation: &Invocation) -> Result<Response, Failure> {
    let machine = Machine::load(invocation)?;
    let (report, warnings) = find(&machine, &[])?;
    let mut response = Response::new(&report);
    for warning in warnings {
        response = response.warn(warning);
    }
    Ok(response)
}

/// What is outdated, limited to `only` when it names packages, and any warnings.
pub(super) fn find(
    machine: &Machine,
    only: &[Package],
) -> Result<(OutdatedReport, Vec<String>), Failure> {
    let mise = &machine.environment.mise;
    let wants = |key: &str| only.is_empty() || only.iter().any(|package| package.key == key);
    let wants_packages = only.is_empty() || only.iter().any(|package| !package.is_tool());
    let wants_tools = only.is_empty() || only.iter().any(Package::is_tool);
    let (plan, tools, states) = std::thread::scope(|scope| {
        let plan = scope.spawn(|| -> Result<Option<Output>, Failure> {
            wants_packages
                .then(|| mise.query(&["bootstrap", "packages", "upgrade", "--dry-run"]))
                .transpose()
        });
        let tools = scope.spawn(|| -> Result<Vec<OutdatedTool>, Failure> {
            if wants_tools { mise.outdated_tools() } else { Ok(Vec::new()) }
        });
        let states = scope.spawn(|| -> Result<Vec<PackageState>, Failure> {
            if wants_packages { mise.package_states() } else { Ok(Vec::new()) }
        });
        (joined(plan.join()), joined(tools.join()), joined(states.join()))
    });
    let (plan, tools, states) = (plan?, tools?, states?);
    let mut report = OutdatedReport::default();
    let mut warnings = Vec::new();

    if let Some(output) = plan {
        let plan = upgrade_plan(&output.combined());
        // mise names a tap's package by its short name: `tg` for `alyraffauf/tap/tg`.
        let declared = |manager: &str, name: &str| {
            let (short, tapped) = (format!("{manager}:{name}"), format!("/{name}"));
            states.iter().find(|state| {
                state.manager == manager && (state.key == short || state.key.ends_with(&tapped))
            })
        };
        for upgrade in plan.upgrades {
            let Some(state) = declared(upgrade.manager, &upgrade.name) else { continue };
            if wants(&state.key) {
                // A cask version can carry a build hash after a comma.
                let installed = state
                    .installed
                    .as_deref()
                    .map(|version| version.split(',').next().unwrap_or(version).to_string());
                report.items.push(OutdatedPackage {
                    package: state.key.clone(),
                    installed,
                    latest: upgrade.version,
                });
            }
        }
        for (key, reason) in plan.skipped {
            let (manager, name) = key.split_once(':').unwrap_or(("", &key));
            let package = declared(manager, name).map_or(key.clone(), |state| state.key.clone());
            if wants(&package) {
                report.skipped.push(Skipped { package, reason });
            }
        }
    }

    let apps: Vec<&PackageState> = states
        .iter()
        .filter(|state| state.manager == "mas" && state.state == "installed" && wants(&state.key))
        .collect();
    if !apps.is_empty() {
        let client = Client::default();
        let ids: Vec<&str> =
            apps.iter().map(|state| state.key.trim_start_matches("mas:")).collect();
        match AppStore::new(&client, &machine.environment.app_store_api).lookup(&ids) {
            Ok(found) => {
                for app in found {
                    let key = format!("mas:{}", app.track_id);
                    let Some(state) = apps.iter().find(|state| state.key == key) else { continue };
                    let newer = app.version.filter(|latest| {
                        state
                            .installed
                            .as_deref()
                            .is_none_or(|installed| is_newer(latest, installed))
                    });
                    if let Some(latest) = newer {
                        report.items.push(OutdatedPackage {
                            package: key,
                            installed: state.installed.clone(),
                            latest,
                        });
                    }
                }
            }
            Err(failure) => warnings.push(format!("skipped App Store apps: {}", failure.message)),
        }
    }

    for tool in tools.into_iter().filter(|tool| wants(&tool.key)) {
        report.items.push(OutdatedPackage {
            package: tool.key,
            installed: tool.current,
            latest: tool.latest,
        });
    }
    Ok((report, warnings))
}

fn joined<T>(result: std::thread::Result<Result<T, Failure>>) -> Result<T, Failure> {
    result.unwrap_or_else(|_| Err(Failure::internal("a mise query panicked")))
}

/// Whether the dotted version `latest` is above `installed`: `3.2` and `3.2.0` are the same, and
/// numeric parts compare as numbers.
fn is_newer(latest: &str, installed: &str) -> bool {
    let parts = |version: &str| -> Vec<String> {
        let mut parts: Vec<String> = version.split('.').map(str::to_string).collect();
        while parts.last().is_some_and(|part| part == "0") {
            parts.pop();
        }
        parts
    };
    let (latest, installed) = (parts(latest), parts(installed));
    for index in 0..latest.len().max(installed.len()) {
        let (left, right) = (latest.get(index), installed.get(index));
        let order = match (
            left.and_then(|part| part.parse::<u64>().ok()),
            right.and_then(|part| part.parse::<u64>().ok()),
        ) {
            (Some(left), Some(right)) => left.cmp(&right),
            _ => left.cmp(&right),
        };
        if order != std::cmp::Ordering::Equal {
            return order == std::cmp::Ordering::Greater;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_higher_store_version_is_newer() {
        assert!(is_newer("15.4.2", "15.4"));
        assert!(is_newer("3.10", "3.9"));
        assert!(!is_newer("3.2", "3.2.0"));
        assert!(!is_newer("1.6.0", "2.1.11"));
    }

    #[test]
    fn text_shows_each_upgrade_and_what_mise_skips() {
        let report = OutdatedReport {
            items: vec![OutdatedPackage {
                package: "brew:wget".into(),
                installed: Some("1.24.5".into()),
                latest: "1.25.0".into(),
            }],
            skipped: vec![Skipped {
                package: "brew-cask:1password".into(),
                reason: "updates itself".into(),
            }],
        };
        assert_eq!(
            report.render_text(),
            "  brew:wget  1.24.5 -> 1.25.0\nUpgrade with `pkg upgrade`.\n\nmise leaves these alone:\n  brew-cask:1password  updates itself\n"
        );
    }
}
