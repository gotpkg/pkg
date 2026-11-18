//! Homebrew tap URLs for taps that do not live at `github.com/<owner>/homebrew-<tap>`.

use std::ffi::OsStr;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Serialize;

use crate::common::{Machine, ReceiptStatus, absolute};
use crate::declarations::same_file;
use crate::requests::validate_operand;
use crate::{
    Client, Error, ErrorDetails, Mode, Outcome, Progress, Recovery, TapAddRequest, TapRemoveRequest,
};

/// Every declared tap URL.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TapList {
    pub items: Vec<TapEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TapEntry {
    pub tap: String,
    pub url: String,
    /// The config file that declares it, symlinks resolved.
    pub config: PathBuf,
}

/// The outcome of `pkg tap add` and `pkg tap remove`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct TapReceipt {
    pub status: ReceiptStatus,
    /// `added` or `removed`; for `dry-run`, what would happen.
    pub action: String,
    pub tap: String,
    pub url: Option<String>,
    /// The config file changed, symlinks resolved.
    pub config: PathBuf,
}

impl Client {
    /// Lists `[bootstrap.brew.taps]` across the global config files: each tap with its Git URL
    /// and the file that declares it. Taps mise finds at their default GitHub URL need no entry.
    ///
    /// # Errors
    ///
    /// `PreconditionFailed` when mise cannot start or a config cannot be read, and `MiseFailed`
    /// when mise fails.
    pub fn tap_list(
        &mut self,
        _progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<TapList>, Error> {
        let machine = Machine::load(&self.settings)?;
        let items = machine
            .declarations
            .taps()
            .into_iter()
            .map(|tap| TapEntry { tap: tap.name, url: tap.url, config: absolute(&tap.config) })
            .collect();
        Ok(Outcome::new(TapList { items }))
    }

    /// Records the tap's GitHub URL in `[bootstrap.brew.taps]` of the file mise writes packages
    /// to, with `mise bootstrap packages brew tap`.
    ///
    /// # Errors
    ///
    /// `PreconditionFailed` when mise cannot start or a config cannot be read, and `MiseFailed`
    /// when mise rejects the tap or URL.
    pub fn tap_add(
        &mut self,
        request: TapAddRequest,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<TapReceipt>, Error> {
        validate_operand(&request.tap, "tap")?;
        validate_operand(&request.url, "url")?;
        let machine = Machine::load(&self.settings)?;
        let TapAddRequest { tap, url, mode } = request;
        let target = machine.target(false)?;
        let mut args = vec![
            OsStr::new("bootstrap"),
            OsStr::new("packages"),
            OsStr::new("brew"),
            OsStr::new("tap"),
            OsStr::new("-p"),
            target.path.as_os_str(),
        ];
        if mode.is_dry_run() {
            args.push(OsStr::new("--dry-run"));
        }
        args.extend([OsStr::new(&tap), OsStr::new(&url)]);
        machine.environment.mise.run(&args, progress)?;
        Ok(Outcome::new(TapReceipt {
            status: status(mode),
            action: "added".into(),
            tap,
            url: Some(url),
            config: absolute(&target.path),
        }))
    }

    /// Removes the tap's entry from `[bootstrap.brew.taps]` with
    /// `mise bootstrap packages brew untap`. Packages from the tap stay declared and installed.
    ///
    /// # Errors
    ///
    /// `NotFound` when no config declares a URL for the tap, `PreconditionFailed` when a file
    /// other than mise's write target declares it or mise cannot start, and `MiseFailed` when
    /// mise fails.
    pub fn tap_remove(
        &mut self,
        request: TapRemoveRequest,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<TapReceipt>, Error> {
        validate_operand(&request.tap, "tap")?;
        let machine = Machine::load(&self.settings)?;
        let TapRemoveRequest { tap, mode } = request;
        let target = machine.target(false)?;
        let declared = machine.declarations.taps().into_iter().find(|entry| entry.name == tap);
        let Some(declared) = declared else {
            return Err(Error::not_found(format!("no config declares a URL for the tap {tap}"))
                .with_details(ErrorDetails { tap: Some(tap), ..Default::default() })
                .recover(Recovery::ListTaps));
        };
        if !same_file(&declared.config, &target.path) {
            let config = absolute(&declared.config);
            return Err(Error::precondition(format!(
                "{tap} is declared in {}, and pkg only edits {}",
                config.display(),
                target.real_path.display()
            ))
            .with_details(ErrorDetails {
                tap: Some(tap.clone()),
                config: Some(config.clone()),
                ..Default::default()
            })
            .recover(Recovery::RemoveTap {
                tap,
                config,
                editable_config: target.real_path,
            }));
        }
        let mut args = vec![
            OsStr::new("bootstrap"),
            OsStr::new("packages"),
            OsStr::new("brew"),
            OsStr::new("untap"),
            OsStr::new("-p"),
            target.path.as_os_str(),
        ];
        if mode.is_dry_run() {
            args.push(OsStr::new("--dry-run"));
        }
        args.push(OsStr::new(&tap));
        machine.environment.mise.run(&args, progress)?;
        Ok(Outcome::new(TapReceipt {
            status: status(mode),
            action: "removed".into(),
            url: Some(declared.url),
            tap,
            config: absolute(&target.path),
        }))
    }
}

fn status(mode: Mode) -> ReceiptStatus {
    if mode.is_dry_run() { ReceiptStatus::DryRun } else { ReceiptStatus::Completed }
}
