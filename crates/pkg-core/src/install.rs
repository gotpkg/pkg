//! Installing packages and recording them in the mise config.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Serialize;

use crate::common::{Machine, ReceiptStatus, absolute, join_words};
use crate::declarations::{declares_package, remove_package};
use crate::mise::{PackageState, ToolState};
use crate::package::Package;
use crate::requests::validate_packages;
use crate::{Client, Error, ErrorKind, InstallRequest, Outcome, Progress, Recovery};

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
        let machine = Machine::load(&self.settings)?;
        let packages = machine.packages(&request.packages);
        let tools = packages.iter().any(Package::is_tool);
        // A bootstrap package no config declares yet cannot be in mise's answer, so state is read
        // before installing only for a request that names a declared one.
        let declared = packages
            .iter()
            .any(|package| !package.is_tool() && machine.declared_in(package).is_some());
        let before = States::load(&machine, declared, tools)?;
        let mut steps = Vec::new();
        for package in &packages {
            steps.push(match machine.declared_in(package) {
                Some(config) if before.installed(package) => {
                    Step::Unchanged { config: config.to_path_buf() }
                }
                Some(config) => Step::Install { config: config.to_path_buf() },
                None => Step::Add { target: machine.target(package.is_tool())?.path },
            });
        }

        if request.mode.is_dry_run() {
            return preview(&machine, &packages, &steps, &before).map(Outcome::new);
        }

        let mut done: Vec<String> = Vec::new();
        for (package, step) in packages.iter().zip(&steps) {
            apply(&machine, package, step, progress)
                .map_err(|error| failed(error, package, step, &done))?;
            done.push(package.key.clone());
        }

        let after =
            States::load(&machine, packages.iter().any(|package| !package.is_tool()), tools)?;
        let items = packages
            .iter()
            .zip(&steps)
            .map(|(package, step)| {
                let action = match step {
                    Step::Add { .. } if !after.available(package) => InstallAction::Declared,
                    step => step.action(),
                };
                item(package, step, action, after.version(package))
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
enum Step {
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

impl Step {
    fn config(&self) -> &PathBuf {
        match self {
            Step::Add { target } => target,
            Step::Install { config } | Step::Unchanged { config } => config,
        }
    }

    fn action(&self) -> InstallAction {
        match self {
            Step::Add { .. } => InstallAction::Added,
            Step::Install { .. } => InstallAction::Installed,
            Step::Unchanged { .. } => InstallAction::Unchanged,
        }
    }
}

/// What mise reports about the packages and tools the request names.
struct States {
    packages: Vec<PackageState>,
    tools: Vec<ToolState>,
}

impl States {
    /// What mise reports about bootstrap packages when `packages` is set, and about global tools
    /// when `tools` is.
    fn load(machine: &Machine<'_>, packages: bool, tools: bool) -> Result<States, Error> {
        Ok(States {
            packages: if packages { machine.package_states()? } else { Vec::new() },
            tools: if tools { machine.environment.mise.tool_states()? } else { Vec::new() },
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
    steps: &[Step],
    states: &States,
) -> Result<InstallReceipt, Error> {
    let mut plan = Vec::new();
    let mut items = Vec::new();
    for (package, step) in packages.iter().zip(steps) {
        let argument = package.install_argument();
        let args: &[&str] = match (step, package.is_tool()) {
            (Step::Add { .. }, false) => {
                &["bootstrap", "packages", "use", "--dry-run", "-g", &argument]
            }
            (Step::Add { .. }, true) => &["use", "-g", "--dry-run", &argument],
            (Step::Install { .. }, false) => {
                &["bootstrap", "packages", "apply", "--dry-run", &package.key]
            }
            (Step::Install { .. }, true) => &["install", "--dry-run", &package.key],
            (Step::Unchanged { .. }, _) => &[],
        };
        if !args.is_empty() {
            let output = machine.environment.mise.query(args).map_err(|error| {
                failed(error, package, &Step::Unchanged { config: step.config().clone() }, &[])
            })?;
            plan.extend(
                output
                    .combined()
                    .lines()
                    .map(|line| line.trim().trim_start_matches("mise ").to_string())
                    .filter(|line| !line.is_empty()),
            );
        }
        items.push(item(package, step, step.action(), states.version(package)));
    }
    Ok(InstallReceipt { status: ReceiptStatus::DryRun, packages: items, plan })
}

fn apply(
    machine: &Machine<'_>,
    package: &Package,
    step: &Step,
    progress: &mut dyn FnMut(Progress<'_>),
) -> Result<(), Error> {
    let mise = &machine.environment.mise;
    let argument = package.install_argument();
    match (step, package.is_tool()) {
        (Step::Unchanged { .. }, _) => Ok(()),
        (Step::Install { .. }, false) => {
            mise.run(&["bootstrap", "packages", "apply", "--yes", &package.key], progress).map(drop)
        }
        (Step::Install { .. }, true) => mise.run(&["install", &package.key], progress).map(drop),
        // mise writes a tool's declaration only once it is installed.
        (Step::Add { .. }, true) => mise.run(&["use", "-g", &argument], progress).map(drop),
        (Step::Add { target }, false) => {
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

fn failed(mut error: Error, package: &Package, step: &Step, done: &[String]) -> Error {
    error.details.package = Some(package.key.clone());
    error.details.installed = Some(done.to_vec());
    let mut message = format!("mise could not install {package}: {}", error.message);
    if let Step::Add { target } = step {
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
    step: &Step,
    action: InstallAction,
    version: Option<String>,
) -> Installed {
    Installed {
        package: package.key.clone(),
        kind: if package.is_tool() { InstallKind::Tool } else { InstallKind::Package },
        action,
        config: absolute(step.config()),
        version,
    }
}
