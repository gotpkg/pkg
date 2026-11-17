//! `pkg install`: install packages and record them in the mise config.

use std::path::PathBuf;

use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::json;

use super::common::{Machine, PACKAGES, ReceiptStatus, absolute, for_people, join_words};
use crate::NAME;
use crate::declarations::{declares_package, remove_package};
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::mise::{PackageState, ToolState};
use crate::package::Package;
use crate::registry::{CommandSpec, DRY_RUN, Effect, Example, Requirement};
use crate::response::{RenderText, Response};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "install",
    path: &["install"],
    aliases: &["i", "add"],
    summary: "Install packages and record them in your mise config",
    description: "Installs each package with mise and declares it in your global mise config, so \
        `mise bootstrap` installs it again on a new machine. Homebrew formulae (`brew:`), casks \
        (`brew-cask:`), App Store apps (`mas:`), and every other mise package manager go in \
        `[bootstrap.packages]` of the file mise's `write_targets.packages` setting names; tools \
        (`npm:prettier`, `github:cli/cli`, or a registry name such as `ripgrep`) go in `[tools]` \
        of the `write_targets.tools` file, at `latest` unless you pass `@<version>`. A package \
        some config already declares is installed where it is declared, and one that is already \
        installed is left alone, so installing twice is safe. Packages install one at a time in \
        the order given; when mise fails, pkg takes that package's new declaration back out and \
        stops, and the error lists what was installed before it. mise's progress goes to stderr.",
    arguments: &[PACKAGES, DRY_RUN],
    effect: Effect::Modifies("installed software and your global mise config"),
    requires: &[Requirement::Mise],
    dry_run: true,
    idempotent: true,
    errors: &[ErrorCode::InvalidArgument, ErrorCode::PreconditionFailed, ErrorCode::MiseFailed],
    examples: &[
        Example {
            summary: "Preview installing GNU Hello, changing nothing",
            args: &["install", "brew:hello", "--dry-run"],
        },
        Example {
            summary: "Install a formula and a cask",
            args: &["install", "brew:hello", "brew-cask:iina"],
        },
        Example { summary: "Install ripgrep from mise's registry", args: &["install", "ripgrep"] },
    ],
    synonyms: &[
        "add", "get", "setup", "declare", "brew", "homebrew", "formula", "cask", "app", "tool",
    ],
    output_schema: || schema_for!(InstallReceipt),
    run: install,
}];

/// The outcome of `pkg install`.
#[derive(Serialize, JsonSchema)]
pub struct InstallReceipt {
    pub status: ReceiptStatus,
    /// One entry per package, in the order given.
    pub packages: Vec<Installed>,
    /// For `dry-run`, the lines mise printed for what it would do; empty otherwise.
    pub plan: Vec<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct Installed {
    pub package: String,
    /// `package` for `[bootstrap.packages]`, `tool` for `[tools]`.
    pub kind: Kind,
    /// For `dry-run`, what running without --dry-run would do.
    pub action: Action,
    /// The config file that declares it, symlinks resolved.
    pub config: String,
    /// The installed version, when mise reports one.
    pub version: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Package,
    Tool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// pkg declared it and mise installed it.
    Added,
    /// A config already declared it; mise installed it.
    Installed,
    /// pkg declared it, but its package manager is not available on this machine.
    Declared,
    /// It was declared and installed already; nothing changed.
    Unchanged,
}

impl RenderText for InstallReceipt {
    fn render_text(&self) -> String {
        let dry = self.status == ReceiptStatus::DryRun;
        let mut text = String::new();
        for item in &self.packages {
            let package = &item.package;
            let version =
                item.version.as_ref().map(|version| format!(" {version}")).unwrap_or_default();
            let config = for_people(&item.config);
            let line = match (item.action, dry) {
                (Action::Added, false) => {
                    format!("Installed {package}{version}, declared in {config}")
                }
                (Action::Added, true) => {
                    format!("Would install {package} and declare it in {config}")
                }
                (Action::Installed, false) => {
                    format!("Installed {package}{version}, already declared in {config}")
                }
                (Action::Installed, true) => {
                    format!("Would install {package}, already declared in {config}")
                }
                (Action::Declared, false) => format!(
                    "Declared {package} in {config}; its package manager is not available on this \
                     machine"
                ),
                (Action::Declared, true) => format!("Would declare {package} in {config}"),
                (Action::Unchanged, _) => {
                    format!("{package}{version} is already installed, declared in {config}")
                }
            };
            text.push_str(&line);
            text.push('\n');
        }
        if dry {
            if !self.plan.is_empty() {
                text.push_str("\nmise's plan:\n");
                for line in &self.plan {
                    text.push_str(&format!("  {line}\n"));
                }
            }
            text.push_str("Dry run: nothing was installed or written.\n");
        }
        text
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

    fn action(&self) -> Action {
        match self {
            Step::Add { .. } => Action::Added,
            Step::Install { .. } => Action::Installed,
            Step::Unchanged { .. } => Action::Unchanged,
        }
    }
}

/// What mise reports about the packages and tools the command names.
struct States {
    packages: Vec<PackageState>,
    tools: Vec<ToolState>,
}

impl States {
    fn load(machine: &Machine, packages: &[Package]) -> Result<States, Failure> {
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

fn install(invocation: &Invocation) -> Result<Response, Failure> {
    let machine = Machine::load(invocation)?;
    let packages = machine.packages(invocation, "packages");
    let before = States::load(&machine, &packages)?;
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

    if invocation.dry_run() {
        return preview(&machine, &packages, &steps, &before);
    }

    let mut done: Vec<String> = Vec::new();
    for (package, step) in packages.iter().zip(&steps) {
        apply(&machine, package, step).map_err(|failure| failed(failure, package, step, &done))?;
        done.push(package.key.clone());
    }

    let after = States::load(&machine, &packages)?;
    let items = packages
        .iter()
        .zip(&steps)
        .map(|(package, step)| {
            let action = match step {
                Step::Add { .. } if !after.available(package) => Action::Declared,
                step => step.action(),
            };
            item(package, step, action, after.version(package))
        })
        .collect();
    Ok(Response::new(&InstallReceipt {
        status: ReceiptStatus::Completed,
        packages: items,
        plan: Vec::new(),
    }))
}

fn preview(
    machine: &Machine,
    packages: &[Package],
    steps: &[Step],
    states: &States,
) -> Result<Response, Failure> {
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
            let output = machine.environment.mise.query(args).map_err(|failure| {
                failed(failure, package, &Step::Unchanged { config: step.config().clone() }, &[])
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
    Ok(Response::new(&InstallReceipt { status: ReceiptStatus::DryRun, packages: items, plan }))
}

fn apply(machine: &Machine, package: &Package, step: &Step) -> Result<(), Failure> {
    let mise = &machine.environment.mise;
    let argument = package.install_argument();
    match (step, package.is_tool()) {
        (Step::Unchanged { .. }, _) => Ok(()),
        (Step::Install { .. }, false) => {
            mise.run(&["bootstrap", "packages", "apply", "--yes", &package.key]).map(drop)
        }
        (Step::Install { .. }, true) => mise.run(&["install", &package.key]).map(drop),
        // mise writes a tool's declaration only once it is installed.
        (Step::Add { .. }, true) => mise.run(&["use", "-g", &argument]).map(drop),
        (Step::Add { target }, false) => {
            let Err(failure) =
                mise.run(&["bootstrap", "packages", "use", "--yes", "-g", &argument])
            else {
                return Ok(());
            };
            // `use` declares before it installs, and an entry left behind would stop every
            // later bootstrap. Take out the entry this attempt added; nothing declared it before.
            if declares_package(target, &package.key) {
                remove_package(target, &package.key)?;
            }
            Err(failure)
        }
    }
}

fn failed(failure: Failure, package: &Package, step: &Step, done: &[String]) -> Failure {
    let mut details = (*failure.details).clone();
    details["package"] = json!(package.key);
    details["installed"] = json!(done);
    let mut message = format!("mise could not install {package}: {}", failure.message);
    if let Step::Add { target } = step {
        details["config"] = json!(absolute(target));
        message.push_str(&format!(" ({package} is not in your mise config)"));
    }
    if !done.is_empty() {
        message.push_str(&format!("; installed {} before it", join_words(done)));
    }
    let mut failure = Failure { message, details: Box::new(details), ..failure };
    if failure.code == ErrorCode::MiseFailed {
        failure = failure
            .suggest(format!("{NAME} search {}", package.name()))
            .suggest(format!("{NAME} info {package}"));
    }
    failure
}

fn item(package: &Package, step: &Step, action: Action, version: Option<String>) -> Installed {
    Installed {
        package: package.key.clone(),
        kind: if package.is_tool() { Kind::Tool } else { Kind::Package },
        action,
        config: absolute(step.config()),
        version,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_names_the_package_version_and_config() {
        let receipt = InstallReceipt {
            status: ReceiptStatus::Completed,
            packages: vec![Installed {
                package: "brew:wget".into(),
                kind: Kind::Package,
                action: Action::Added,
                config: "/nowhere/config.macos.toml".into(),
                version: Some("1.25.0".into()),
            }],
            plan: Vec::new(),
        };
        assert_eq!(
            receipt.render_text(),
            "Installed brew:wget 1.25.0, declared in /nowhere/config.macos.toml\n"
        );
    }
}
