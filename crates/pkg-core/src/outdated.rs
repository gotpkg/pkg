//! Installed packages and tools with a newer version, which `update` installs.

use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog;
use crate::catalog::app_store::AppStore;
use crate::common::Machine;
use crate::mise::{
    MISE, MiseRelease, OutdatedTool, Output, PACKAGE_MANAGER_REASON, PackageState, upgrade_plan,
};
use crate::package::Package;
use crate::{Client, Error, Outcome, Progress};

/// Every outdated package, never paged.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct OutdatedReport {
    pub items: Vec<OutdatedPackage>,
    /// Packages mise will not update, with its reason.
    pub skipped: Vec<Skipped>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct OutdatedPackage {
    pub package: String,
    pub installed: Option<String>,
    pub latest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Skipped {
    pub package: String,
    pub reason: String,
}

impl Client {
    /// Lists every installed package and tool that `update` would update, with the installed
    /// and the newest version, and the casks mise will not update. Homebrew's metadata is
    /// refreshed; nothing is installed or written. App Store apps the store cannot be asked about
    /// are left out with a warning.
    ///
    /// # Errors
    ///
    /// `PreconditionFailed` when mise cannot start or a config cannot be read, and `MiseFailed`
    /// when mise fails.
    pub fn outdated(
        &mut self,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<OutdatedReport>, Error> {
        let machine = Machine::load(&self.settings)?;
        let (report, warnings) = find(&machine, &[])?;
        Ok(Outcome::with_warnings(report, warnings, progress))
    }
}

/// What is outdated, limited to `only` when it names packages, and any warnings. With no names,
/// mise itself comes first.
pub(crate) fn find(
    machine: &Machine<'_>,
    only: &[Package],
) -> Result<(OutdatedReport, Vec<String>), Error> {
    let mise = &machine.environment.mise;
    let wants = |key: &str| only.is_empty() || only.iter().any(|package| package.key == key);
    let wants_packages = only.is_empty() || only.iter().any(|package| !package.is_tool());
    let wants_tools = only.is_empty() || only.iter().any(Package::is_tool);
    let (plan, tools, states, release) = std::thread::scope(|scope| {
        let plan = scope.spawn(|| -> Result<Option<Output>, Error> {
            wants_packages
                .then(|| mise.query(&["bootstrap", "packages", "upgrade", "--dry-run"]))
                .transpose()
        });
        let tools = scope.spawn(|| -> Result<Vec<OutdatedTool>, Error> {
            if wants_tools { mise.outdated_tools() } else { Ok(Vec::new()) }
        });
        let states = scope.spawn(|| -> Result<Vec<PackageState>, Error> {
            if wants_packages { mise.package_states() } else { Ok(Vec::new()) }
        });
        let release = scope.spawn(|| -> Result<Option<MiseRelease>, Error> {
            only.is_empty().then(|| mise.release()).transpose()
        });
        (joined(plan.join()), joined(tools.join()), joined(states.join()), joined(release.join()))
    });
    let (plan, tools, states, release) = (plan?, tools?, states?, release?);
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
        let client = catalog::Client::new(machine.environment.child_env)?;
        let ids: Vec<&str> =
            apps.iter().map(|state| state.key.trim_start_matches("mas:")).collect();
        match AppStore::new(&client, machine.environment.app_store_api).lookup(&ids) {
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
            Err(error) => warnings.push(format!("skipped App Store apps: {}", error.message)),
        }
    }

    for tool in tools.into_iter().filter(|tool| wants(&tool.key)) {
        report.items.push(OutdatedPackage {
            package: tool.key,
            installed: tool.current,
            latest: tool.latest,
        });
    }
    if let Some(latest) = release.as_ref().and_then(MiseRelease::newer_available) {
        if mise.updates_itself() {
            report.items.insert(
                0,
                OutdatedPackage {
                    package: MISE.into(),
                    installed: release.as_ref().map(|release| release.version.clone()),
                    latest: latest.into(),
                },
            );
        } else {
            let reason = PACKAGE_MANAGER_REASON.into();
            report.skipped.insert(0, Skipped { package: MISE.into(), reason });
        }
    }
    Ok((report, warnings))
}

fn joined<T>(result: std::thread::Result<Result<T, Error>>) -> Result<T, Error> {
    result.unwrap_or_else(|_| Err(Error::internal("a mise query panicked")))
}

/// Whether the dotted version `latest` is above `installed`: `3.2` and `3.2.0` are the same, and
/// numeric parts compare as numbers.
pub(crate) fn is_newer(latest: &str, installed: &str) -> bool {
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
}
