//! Installing packages and recording them in the mise config.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Serialize;

use crate::common::{Machine, ReceiptStatus, absolute, join_words};
use crate::declarations::{declares_package, remove_package};
use crate::mise::{Mise, PackageState, ToolState, install_plan};
use crate::package::Package;
use crate::requests::validate_packages;
use crate::{
    Client, Error, ErrorKind, InstallRequest, Outcome, PackageStep, PlannedItem, Progress,
    Recovery, Step,
};

/// The outcome of `pkg install`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct InstallReceipt {
    pub status: ReceiptStatus,
    /// One entry per package, in the order given.
    pub packages: Vec<Installed>,
    /// For `dry-run`, the lines mise printed for what it would do; empty otherwise.
    pub plan: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Installed {
    pub package: String,
    /// `package` for `[bootstrap.packages]`, `tool` for `[tools]`.
    pub kind: InstallKind,
    /// For `dry-run`, what running without --dry-run would do.
    pub action: InstallAction,
    /// The config file that declares it, symlinks resolved.
    pub config: PathBuf,
    /// The installed version, when mise reports one.
    pub version: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "Kind")]
pub enum InstallKind {
    Package,
    Tool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "Action")]
pub enum InstallAction {
    /// pkg declared it and mise installed it.
    Added,
    /// A config already declared it; mise installed it.
    Installed,
    /// pkg declared it, but its package manager is not available on this machine.
    Declared,
    /// It was declared and installed already; nothing changed.
    Unchanged,
}

impl Client {
    /// Installs each package with mise and declares it in the global mise config, one at a time
    /// in the order given. A package some config already declares is installed where it is
    /// declared; one already installed is left alone. When mise fails, the failed package's new
    /// `[bootstrap.packages]` entry is taken back out and the remaining packages are skipped.
    /// With `report_plans`, each bootstrap package's plan is read from mise just before it
    /// installs and reported as `Progress::Plan`.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` when no package is named or a name is not mise syntax,
    /// `PreconditionFailed` when mise cannot start or a config cannot be read or edited, and
    /// `MiseFailed` when mise fails. A failure names the package in `details.package` and lists
    /// the packages installed before it in `details.installed`.
    pub fn install(
        &mut self,
        request: InstallRequest,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<InstallReceipt>, Error> {
        validate_packages(&request.packages, true)?;
        progress(Progress::Step(Step::ReadInstalled));
        let machine = Machine::load(&self.settings)?;
        let packages = machine.packages(&request.packages);
        let before = States::load(&machine, &packages)?;
        let mut work = Vec::new();
        for package in &packages {
            work.push(match machine.declared_in(package) {
                Some(config) if before.installed(package) => {
                    Work::Unchanged { config: config.to_path_buf() }
                }
                Some(config) => Work::Install { config: config.to_path_buf() },
                None => Work::Add { target: machine.target(package.is_tool())?.path },
            });
        }

        if request.mode.is_dry_run() {
            return preview(&machine, &packages, &work, &before, progress).map(Outcome::new);
        }

        let mut done: Vec<String> = Vec::new();
        for (index, (package, work)) in packages.iter().zip(&work).enumerate() {
            let place =
                PackageStep { package: &package.key, position: index + 1, count: packages.len() };
            apply(&machine, package, work, place, request.report_plans, progress)
                .map_err(|error| failed(error, package, work, &done))?;
            done.push(package.key.clone());
        }

        progress(Progress::Step(Step::ReadResult));
        let after = States::load(&machine, &packages)?;
        let items = packages
            .iter()
            .zip(&work)
            .map(|(package, work)| {
                let action = match work {
                    Work::Add { .. } if !after.available(package) => InstallAction::Declared,
                    work => work.action(),
                };
                item(package, work, action, after.version(package))
            })
            .collect();
        Ok(Outcome::new(InstallReceipt {
            status: ReceiptStatus::Completed,
            packages: items,
            plan: Vec::new(),
        }))
    }
}

/// What pkg will do for one package.
enum Work {
    /// Declare it in `target` and install it.
    Add {
        target: PathBuf,
    },
    /// Install it where `config` already declares it.
    Install {
        config: PathBuf,
    },
    Unchanged {
        config: PathBuf,
    },
}

impl Work {
    fn config(&self) -> &PathBuf {
        match self {
            Work::Add { target } => target,
            Work::Install { config } | Work::Unchanged { config } => config,
        }
    }

    fn action(&self) -> InstallAction {
        match self {
            Work::Add { .. } => InstallAction::Added,
            Work::Install { .. } => InstallAction::Installed,
            Work::Unchanged { .. } => InstallAction::Unchanged,
        }
    }

    /// The read-only query that previews this work, which is also the package's plan; `None`
    /// when there is nothing to do.
    fn preview_args<'a>(&self, package: &'a Package, argument: &'a str) -> Option<Vec<&'a str>> {
        Some(match (self, package.is_tool()) {
            (Work::Add { .. }, false) => {
                vec!["bootstrap", "packages", "use", "--dry-run", "-g", argument]
            }
            (Work::Add { .. }, true) => vec!["use", "-g", "--dry-run", argument],
            (Work::Install { .. }, false) => {
                vec!["bootstrap", "packages", "apply", "--dry-run", &package.key]
            }
            (Work::Install { .. }, true) => vec!["install", "--dry-run", &package.key],
            (Work::Unchanged { .. }, _) => return None,
        })
    }
}

/// What mise reports about the packages and tools the request names.
struct States {
    packages: Vec<PackageState>,
    tools: Vec<ToolState>,
}

impl States {
    fn load(machine: &Machine<'_>, packages: &[Package]) -> Result<States, Error> {
        let mise = &machine.environment.mise;
        Ok(States {
            packages: if packages.iter().any(|package| !package.is_tool()) {
                mise.package_states()?
            } else {
                Vec::new()
            },
            tools: if packages.iter().any(Package::is_tool) {
                mise.tool_states()?
            } else {
                Vec::new()
            },
        })
    }

    fn installed(&self, package: &Package) -> bool {
        if package.is_tool() {
            self.tools.iter().any(|tool| tool.key == package.key && tool.installed)
        } else {
            self.packages.iter().any(|state| state.key == package.key && state.state == "installed")
        }
    }

    fn version(&self, package: &Package) -> Option<String> {
        if package.is_tool() {
            self.tools
                .iter()
                .find(|tool| tool.key == package.key && tool.installed)?
                .version
                .clone()
        } else {
            self.packages.iter().find(|state| state.key == package.key)?.installed.clone()
        }
    }

    /// Whether mise can install `package` here; a manager it knows nothing about counts as
    /// available until mise says otherwise.
    fn available(&self, package: &Package) -> bool {
        self.packages
            .iter()
            .find(|state| state.key == package.key)
            .is_none_or(|state| state.available)
    }
}

fn preview(
    machine: &Machine<'_>,
    packages: &[Package],
    work: &[Work],
    states: &States,
    progress: &mut dyn FnMut(Progress<'_>),
) -> Result<InstallReceipt, Error> {
    let mut plan = Vec::new();
    let mut items = Vec::new();
    for (index, (package, work)) in packages.iter().zip(work).enumerate() {
        let argument = package.install_argument();
        if let Some(args) = work.preview_args(package, &argument) {
            let place =
                PackageStep { package: &package.key, position: index + 1, count: packages.len() };
            progress(Progress::Step(Step::PlanInstall(place)));
            let output = machine.environment.mise.query(&args).map_err(|error| {
                failed(error, package, &Work::Unchanged { config: work.config().clone() }, &[])
            })?;
            plan.extend(
                output
                    .combined()
                    .lines()
                    .map(|line| line.trim().trim_start_matches("mise ").to_string())
                    .filter(|line| !line.is_empty()),
            );
        }
        items.push(item(package, work, work.action(), states.version(package)));
    }
    Ok(InstallReceipt { status: ReceiptStatus::DryRun, packages: items, plan })
}

fn apply(
    machine: &Machine<'_>,
    package: &Package,
    work: &Work,
    place: PackageStep<'_>,
    report_plan: bool,
    progress: &mut dyn FnMut(Progress<'_>),
) -> Result<(), Error> {
    if let Work::Unchanged { .. } = work {
        return Ok(());
    }
    let mise = &machine.environment.mise;
    let argument = package.install_argument();
    let plan = match work.preview_args(package, &argument) {
        Some(args) if report_plan && !package.is_tool() => {
            progress(Progress::Step(Step::PlanInstall(place)));
            planned_items(mise, &args)
        }
        _ => Vec::new(),
    };
    progress(Progress::Step(Step::Install(place)));
    if !plan.is_empty() {
        progress(Progress::Plan(&plan));
    }
    match (work, package.is_tool()) {
        (Work::Unchanged { .. }, _) => Ok(()),
        (Work::Install { .. }, false) => {
            mise.run(&["bootstrap", "packages", "apply", "--yes", &package.key], progress).map(drop)
        }
        (Work::Install { .. }, true) => mise.run(&["install", &package.key], progress).map(drop),
        // mise writes a tool's declaration only once it is installed.
        (Work::Add { .. }, true) => mise.run(&["use", "-g", &argument], progress).map(drop),
        (Work::Add { target }, false) => {
            let Err(error) =
                mise.run(&["bootstrap", "packages", "use", "--yes", "-g", &argument], progress)
            else {
                return Ok(());
            };
            // `use` declares before it installs, and an entry left behind would stop every
            // later bootstrap. Take out the entry this attempt added; nothing declared it before.
            if declares_package(target, &package.key) {
                remove_package(target, &package.key)?;
            }
            Err(error)
        }
    }
}

/// The items mise's preview says the install brings. The plan only feeds the caller's display,
/// so a preview that fails costs that detail and changes nothing else.
fn planned_items(mise: &Mise<'_>, args: &[&str]) -> Vec<PlannedItem> {
    match mise.query(args) {
        Ok(output) => install_plan(&output.combined()),
        Err(_) => Vec::new(),
    }
}

fn failed(mut error: Error, package: &Package, work: &Work, done: &[String]) -> Error {
    error.details.package = Some(package.key.clone());
    error.details.installed = Some(done.to_vec());
    let mut message = format!("mise could not install {package}: {}", error.message);
    if let Work::Add { target } = work {
        error.details.config = Some(absolute(target));
        message.push_str(&format!(" ({package} is not in your mise config)"));
    }
    if !done.is_empty() {
        message.push_str(&format!("; installed {} before it", join_words(done)));
    }
    error.message = message;
    if error.kind == ErrorKind::MiseFailed {
        error = error
            .recover(Recovery::Search(package.name().to_string()))
            .recover(Recovery::Info(package.key.clone()));
    }
    error
}

fn item(
    package: &Package,
    work: &Work,
    action: InstallAction,
    version: Option<String>,
) -> Installed {
    Installed {
        package: package.key.clone(),
        kind: if package.is_tool() { InstallKind::Tool } else { InstallKind::Package },
        action,
        config: absolute(work.config()),
        version,
    }
}
