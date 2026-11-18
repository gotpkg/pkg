//! `setup`: getting a machine with nothing to restore ready for `install`.

use schemars::JsonSchema;
use serde::Serialize;

use super::handoff::{Handoff, hand_off};
use super::mise::{MiseAction, MiseSetup, ensure};
use super::shims::{PathChange, global_config_exists, put_on_path};
use super::supported_platform;
use crate::common::ReceiptStatus;
use crate::{Client, Error, Outcome, Progress, SetupRequest};

/// The outcome of `pkg setup`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SetupReceipt {
    pub status: ReceiptStatus,
    pub mise: MiseSetup,
    /// What pkg did to put mise's shims on `PATH`; null when nothing was needed, because a
    /// configuration exists or the shims are on `PATH` already.
    pub path: Option<PathChange>,
    pub handoff: Handoff,
}

impl Client {
    /// Installs mise with mise's own installer when none is found, puts mise's shims on `PATH`
    /// when no global configuration exists to do it, and hands pkg to mise. Running it again
    /// changes nothing that is already in place.
    ///
    /// # Errors
    ///
    /// `PreconditionFailed` on Windows, when a mise named by path is missing, when mise is older
    /// than pkg supports, or when a startup file cannot be written; `Unavailable` when mise's
    /// installer cannot be downloaded; `MiseFailed` when the installer or mise fails.
    pub fn setup(
        &mut self,
        request: SetupRequest,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<SetupReceipt>, Error> {
        supported_platform("setup")?;
        let has_config = global_config_exists(&self.settings);
        let (settings, mise) = ensure(&self.settings, request.mode, progress)?;
        let path = put_on_path(&settings, has_config, request.modify_path, request.mode)?;
        let mise_ready = !(request.mode.is_dry_run() && mise.action == MiseAction::Installed);
        let handoff =
            hand_off(&settings, request.mode, &request.pkg_version, mise_ready, progress)?;
        let status = if request.mode.is_dry_run() {
            ReceiptStatus::DryRun
        } else {
            ReceiptStatus::Completed
        };
        Ok(Outcome::new(SetupReceipt { status, mise, path, handoff }))
    }
}
