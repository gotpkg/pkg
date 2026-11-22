#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// Borrowed for the callback only; retain an owned copy if the host needs a log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress<'a> {
    /// Text mise wrote, in mise's own wording, which any mise release can change.
    Output {
        stream: OutputStream,
        text: &'a str,
    },
    Warning(&'a str),
    /// A stretch of the operation's own work begins. It lasts until the next step begins or the
    /// method returns, so every event delivered in between belongs to it.
    Step(Step<'a>),
    /// The items mise is about to install or uninstall in the current step, delivered right
    /// after the step begins.
    Plan(&'a [PlannedItem]),
}

/// A stretch of an operation's own work, reported as [`Progress::Step`] by `install`, `update`,
/// `sync`, `remove`, and `prune`, and by no other operation. Their steps, in order:
///
/// - `install`: `ReadInstalled`; for each package that needs work, `PlanInstall` and `Install`;
///   `ReadResult`.
/// - `update`: `UpdateMise`, `ReadInstalled`, `UpdatePackages`, `UpdateTools`, `ReadResult`.
/// - `sync`: `ReadInstalled`, `InstallMissingPackages`, `InstallMissingTools`, `ReadResult`.
/// - `remove`: `CheckPrune`, `Undeclare`, then for each Homebrew manager of the packages named,
///   `PlanPrune` and `Uninstall`.
/// - `prune`: for each Homebrew manager, `PlanPrune` and `Uninstall`.
///
/// A step whose work is not needed is not reported: `install` reports no step for a package that
/// is installed already, and `PlanInstall` only for a bootstrap package when
/// [`InstallRequest::report_plans`](crate::InstallRequest::report_plans) is set; `update` reports
/// `UpdateMise` only when no package is named, and `UpdatePackages` or `UpdateTools` only when
/// the request covers them; `sync` installs only what is missing; `Uninstall` follows only a
/// non-empty prune plan. A dry run reports only the steps that change nothing, and an `install`
/// dry run reports `PlanInstall` for every package that needs work, since asking mise for the
/// plan is the dry run itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step<'a> {
    /// `mise self-update`.
    UpdateMise,
    /// Reading the configuration and what is installed, before anything changes.
    ReadInstalled,
    /// Asking mise what installing one package involves.
    PlanInstall(PackageStep<'a>),
    /// Installing one package.
    Install(PackageStep<'a>),
    /// Upgrading bootstrap packages.
    UpdatePackages,
    /// Upgrading global tools.
    UpdateTools,
    /// Installing the bootstrap packages that are declared and missing.
    InstallMissingPackages,
    /// Installing the global tools that are declared and missing.
    InstallMissingTools,
    /// Reading the configuration and checking that a prune would uninstall nothing besides the
    /// packages named.
    CheckPrune,
    /// Taking the declarations out of the configuration.
    Undeclare,
    /// Asking mise what a prune of one manager's packages would uninstall.
    PlanPrune { manager: &'a str },
    /// Uninstalling one manager's packages that no configuration declares.
    Uninstall { manager: &'a str },
    /// Reading what is installed after the changes.
    ReadResult,
}

/// The package a step works on, and its place among the request's distinct packages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackageStep<'a> {
    /// The package as mise keys it, such as `brew:wget` or `ripgrep`.
    pub package: &'a str,
    /// Counting from 1, in the order the request names them.
    pub position: usize,
    pub count: usize,
}

/// One item of mise's plan: a formula, cask, or package, requested or a dependency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedItem {
    /// `manager:name`, such as `brew:openssl@3` or `brew-cask:iina`.
    pub name: String,
    /// The version, when the plan states one.
    pub version: Option<String>,
    pub action: PlannedAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlannedAction {
    /// Installed because the request names it.
    Install,
    /// Installed because an item being installed needs it.
    InstallDependency,
    Uninstall,
}

#[derive(Debug)]
pub struct Outcome<T> {
    pub result: T,
    pub warnings: Vec<String>,
}

impl<T> Outcome<T> {
    pub(crate) fn new(result: T) -> Self {
        Self { result, warnings: Vec::new() }
    }

    pub(crate) fn with_warnings(
        result: T,
        warnings: Vec<String>,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Self {
        for warning in &warnings {
            progress(Progress::Warning(warning));
        }
        Self { result, warnings }
    }
}
