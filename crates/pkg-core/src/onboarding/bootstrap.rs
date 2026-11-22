//! `bootstrap`: restoring a configuration with `mise bootstrap`.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;

use super::handoff::{Handoff, hand_off};
use super::mise::{MiseAction, MiseSetup, ensure};
use super::preview::{config_directory_files, declares_templated_files, follow_up};
use super::startup_files::{StartupFiles, installers_files, move_aside, restore};
use super::supported_platform;
use crate::common::{Machine, ReceiptStatus};
use crate::mise::{BootstrapRun, Mise};
use crate::requests::validate_operand;
use crate::{
    BootstrapRequest, BootstrapSource, Client, Error, ErrorDetails, Outcome, Progress, Recovery,
    Settings,
};

/// The outcome of `pkg bootstrap`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapReceipt {
    pub status: ReceiptStatus,
    pub mise: MiseSetup,
    pub source: SourceReport,
    /// mise's global configuration directory, which `--from` links into the checkout.
    pub config_dir: PathBuf,
    /// The configuration environments selected.
    pub environments: Vec<String>,
    pub startup_files: StartupFiles,
    /// For `dry-run`, what the preview leaves out, and why.
    pub not_previewed: Vec<String>,
    /// The follow-up work mise listed; null when mise wrote to the terminal instead.
    pub follow_up: Option<Vec<String>>,
    /// What happened when pkg was handed to mise; null in a dry run from a repository, whose
    /// configuration does not exist yet.
    pub handoff: Option<Handoff>,
}

/// The configuration bootstrap applied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceReport {
    /// `global`, `repository`, or `config`.
    pub kind: SourceKind,
    /// The repository's URL.
    pub url: Option<String>,
    /// The checkout.
    pub directory: Option<PathBuf>,
    /// The checkout's directory that is the global configuration.
    pub config_dir: Option<String>,
    /// The config file applied with the global configuration.
    pub config: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    Global,
    Repository,
    Config,
}

impl Client {
    /// Runs `mise bootstrap` for the global configuration, a dotfiles repository whose config
    /// directory mise links as the global configuration, or one config file, installing mise
    /// first when it is missing. Startup files holding only pkg's `PATH` block are moved aside
    /// while it runs and put back where the configuration deployed nothing. A successful run
    /// ends by handing pkg to mise. A preview runs `mise bootstrap --dry-run`, without the
    /// `files` part when a templated `[bootstrap.files]` entry could run a command.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for a malformed source or `--save-env` without an environment,
    /// `NotFound` for a missing config file, `PreconditionFailed` on Windows, when mise's config
    /// directory already holds another configuration, or when mise cannot start, and
    /// `MiseFailed` when mise fails; `details.phase` names the step and
    /// `details.changesMayRemain` says whether mise had started.
    pub fn bootstrap(
        &mut self,
        request: BootstrapRequest,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<BootstrapReceipt>, Error> {
        supported_platform("bootstrap")?;
        validate(&request)?;
        if let BootstrapSource::Repository { directory, config_dir, .. } = &request.source {
            config_dir_free(&self.settings, &directory.join(config_dir))?;
        }
        let dry = request.mode.is_dry_run();
        let (settings, mise) = ensure(&self.settings, request.mode, progress)
            .map_err(|error| failed(error, "installing mise", Vec::new(), false, true))?;
        let mut receipt = BootstrapReceipt {
            status: if dry { ReceiptStatus::DryRun } else { ReceiptStatus::Completed },
            source: report(&request.source),
            config_dir: settings.config_dir.clone(),
            environments: request.environments.clone(),
            startup_files: StartupFiles::default(),
            not_previewed: Vec::new(),
            follow_up: None,
            handoff: None,
            mise,
        };
        if dry {
            preview(&settings, &request, &mut receipt, progress)?;
            return Ok(Outcome::new(receipt));
        }

        let mut completed = Vec::new();
        if receipt.mise.action == MiseAction::Installed {
            completed.push(format!("installed mise at {}", receipt.mise.program.display()));
        }
        let moved = move_aside(installers_files(&settings))?;
        let args = mise_args(&request, false);
        let ran = Mise::new(&settings).bootstrap(
            &args,
            &run(&request, &settings, request.terminal),
            progress,
        );
        receipt.startup_files = restore(moved)?;
        completed.extend(
            receipt
                .startup_files
                .moved_aside
                .iter()
                .map(|file| format!("moved {} aside", file.display())),
        );
        completed.extend(
            receipt
                .startup_files
                .restored
                .iter()
                .map(|file| format!("put {} back", file.display())),
        );
        let output = ran.map_err(|error| {
            failed(error, "bootstrap", completed.clone(), true, request.terminal)
        })?;
        receipt.follow_up = output.map(|output| follow_up(&output.stdout));
        completed.push("mise bootstrap".into());
        let handoff = hand_off(&settings, request.mode, &request.pkg_version, true, progress)
            .map_err(|error| {
                failed(error, "handing pkg to mise", completed, true, request.terminal)
            })?;
        receipt.handoff = Some(handoff);
        Ok(Outcome::new(receipt))
    }
}

fn preview(
    settings: &Settings,
    request: &BootstrapRequest,
    receipt: &mut BootstrapReceipt,
    progress: &mut dyn FnMut(Progress<'_>),
) -> Result<(), Error> {
    receipt.startup_files.moved_aside =
        installers_files(settings).into_iter().map(|(file, _)| file).collect();
    let mise_ready = receipt.mise.action == MiseAction::Found;
    if !mise_ready {
        receipt
            .not_previewed
            .push("the configuration: mise is not installed yet; preview again once it is".into());
    } else {
        let skip_files = declares_templated_files(&preview_files(settings, request)?);
        if skip_files {
            receipt.not_previewed.push(
                "files: mise renders [bootstrap.files] templates even in a dry run, and a \
                 template can run a command"
                    .into(),
            );
        }
        let args = mise_args(request, skip_files);
        let output =
            Mise::new(settings).bootstrap(&args, &run(request, settings, false), progress)?;
        receipt.follow_up = output.map(|output| follow_up(&output.stdout));
    }
    if matches!(request.source, BootstrapSource::Repository { .. }) {
        receipt.not_previewed.push(
            "handing pkg to mise: it depends on the configuration the checkout brings".into(),
        );
    } else {
        receipt.handoff =
            Some(hand_off(settings, request.mode, &request.pkg_version, mise_ready, progress)?);
    }
    Ok(())
}

fn validate(request: &BootstrapRequest) -> Result<(), Error> {
    if request.save_environments && request.environments.is_empty() {
        return Err(argument("--save-env needs at least one --env", "save-env"));
    }
    for environment in &request.environments {
        validate_operand(environment, "environment")?;
    }
    match &request.source {
        BootstrapSource::Global => Ok(()),
        BootstrapSource::Repository { url, directory, config_dir } => {
            validate_operand(url, "repository URL")?;
            if !directory.is_absolute() {
                return Err(argument(
                    "the checkout directory must be an absolute path",
                    "directory",
                ));
            }
            let inside = Path::new(config_dir)
                .components()
                .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));
            if config_dir.is_empty() || !inside {
                return Err(argument(
                    "--config-dir must name a directory inside the checkout, such as `mise`",
                    "config-dir",
                ));
            }
            Ok(())
        }
        BootstrapSource::ConfigFile(file) => {
            if !file.is_absolute() {
                return Err(argument("the config file must be an absolute path", "config"));
            }
            if !file.is_file() {
                return Err(Error::not_found(format!("{} does not exist", file.display()))
                    .with_details(ErrorDetails {
                        path: Some(file.clone()),
                        ..Default::default()
                    }));
            }
            Ok(())
        }
    }
}

fn argument(message: &str, argument: &str) -> Error {
    Error::invalid_argument(message)
        .with_details(ErrorDetails { argument: Some(argument.into()), ..Default::default() })
}

/// mise links its config directory only where nothing else is: refuse before changing anything.
fn config_dir_free(settings: &Settings, target: &Path) -> Result<(), Error> {
    let config_dir = &settings.config_dir;
    let free = match config_dir.symlink_metadata() {
        Err(_) => true,
        Ok(metadata) if metadata.is_symlink() => std::fs::read_link(config_dir)
            .is_ok_and(|link| link == target || same_place(config_dir, target)),
        Ok(metadata) if metadata.is_dir() => {
            std::fs::read_dir(config_dir).is_ok_and(|mut entries| entries.next().is_none())
        }
        Ok(_) => false,
    };
    if free {
        return Ok(());
    }
    Err(Error::precondition(format!(
        "{} already holds a mise configuration, so mise cannot link it to {}",
        config_dir.display(),
        target.display()
    ))
    .with_details(ErrorDetails { path: Some(config_dir.clone()), ..Default::default() })
    .recover(Recovery::MoveAside(config_dir.clone())))
}

fn same_place(left: &Path, right: &Path) -> bool {
    matches!((std::fs::canonicalize(left), std::fs::canonicalize(right)), (Ok(left), Ok(right)) if left == right)
}

fn report(source: &BootstrapSource) -> SourceReport {
    let mut report = SourceReport {
        kind: SourceKind::Global,
        url: None,
        directory: None,
        config_dir: None,
        config: None,
    };
    match source {
        BootstrapSource::Global => {}
        BootstrapSource::Repository { url, directory, config_dir } => {
            report.kind = SourceKind::Repository;
            report.url = Some(url.clone());
            report.directory = Some(directory.clone());
            report.config_dir = Some(config_dir.clone());
        }
        BootstrapSource::ConfigFile(file) => {
            report.kind = SourceKind::Config;
            report.config = Some(file.clone());
        }
    }
    report
}

/// The config files a preview reads for templated `[bootstrap.files]` entries, including those
/// of the requested environments.
fn preview_files(settings: &Settings, request: &BootstrapRequest) -> Result<Vec<PathBuf>, Error> {
    let mut settings = settings.clone();
    if !request.environments.is_empty() {
        settings.child_env.insert("MISE_ENV".into(), request.environments.join(",").into());
    }
    match &request.source {
        BootstrapSource::Repository { directory, config_dir, .. } => {
            Ok(config_directory_files(&directory.join(config_dir)))
        }
        BootstrapSource::Global => Ok(Machine::load(&settings)?.config_files),
        BootstrapSource::ConfigFile(file) => {
            let mut files = Machine::load(&settings)?.config_files;
            files.push(file.clone());
            Ok(files)
        }
    }
}

/// `mise [-E <env>]… bootstrap …` for the request.
fn mise_args(request: &BootstrapRequest, skip_files: bool) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();
    for environment in &request.environments {
        args.extend(["-E".into(), environment.into()]);
    }
    args.push("bootstrap".into());
    if let BootstrapSource::Repository { url, directory, config_dir } = &request.source {
        args.extend(["--from".into(), url.into(), "--from-dir".into()]);
        args.push(directory.clone().into_os_string());
        args.extend(["--config-dir".into(), config_dir.into()]);
    }
    if request.save_environments {
        args.push("--save-env".into());
    }
    if request.update {
        args.push("--update".into());
    }
    if request.mode.is_dry_run() {
        args.push("--dry-run".into());
    }
    if skip_files {
        args.extend(["--skip".into(), "files".into()]);
    }
    args
}

/// Where and how mise runs: a config file from its own directory, loaded under its own name and
/// trusted for this run only; everything else from the home directory.
fn run<'a>(
    request: &'a BootstrapRequest,
    settings: &'a Settings,
    terminal: bool,
) -> BootstrapRun<'a> {
    match &request.source {
        BootstrapSource::ConfigFile(file) => BootstrapRun {
            directory: file.parent().unwrap_or(&settings.home),
            env: vec![
                (
                    "MISE_OVERRIDE_CONFIG_FILENAMES",
                    file.file_name().map(OsString::from).unwrap_or_default(),
                ),
                ("MISE_TRUSTED_CONFIG_PATHS", file.clone().into_os_string()),
            ],
            terminal,
        },
        _ => BootstrapRun { directory: &settings.home, env: Vec::new(), terminal },
    }
}

/// `error` with the step that failed, what completed before it, and whether mise had started.
fn failed(
    mut error: Error,
    phase: &str,
    completed: Vec<String>,
    started: bool,
    terminal: bool,
) -> Error {
    error.details.phase = Some(phase.into());
    error.details.completed = Some(completed);
    error.details.changes_may_remain = Some(started);
    if started {
        error.message =
            format!("{phase} failed: {}; changes it made before that remain", error.message);
        if !terminal {
            error = error.recover(Recovery::RunInTerminal);
        }
        error = error.recover(Recovery::RunAgain);
    }
    error
}
