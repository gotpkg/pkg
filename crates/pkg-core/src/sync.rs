//! Installing everything the config declares that is missing, as `mise bootstrap` does for
//! packages and tools alone.

use schemars::JsonSchema;
use serde::Serialize;

use crate::common::{Machine, ReceiptStatus};
use crate::{Client, Error, Outcome, Progress, SyncRequest};

/// The outcome of `pkg sync`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SyncReceipt {
    pub status: ReceiptStatus,
    /// Packages and tools that were missing and are now installed (for `dry-run`: are missing).
    pub installed: Vec<String>,
    /// Packages still missing after the run; empty for `dry-run`.
    pub missing: Vec<String>,
}

impl Client {
    /// Installs every declared `[bootstrap.packages]` entry and global tool that is missing:
    /// `mise bootstrap packages apply`, which also removes packages declared absent where the
    /// manager can, and `mise install`. Packages whose manager this machine lacks are skipped.
    ///
    /// # Errors
    ///
    /// `PreconditionFailed` when mise cannot start or a config cannot be read, and `MiseFailed`
    /// when mise fails.
    pub fn sync(
        &mut self,
        request: SyncRequest,
        progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<SyncReceipt>, Error> {
        let machine = Machine::load(&self.settings)?;
        let mise = &machine.environment.mise;
        let (packages, tools) = missing(&machine)?;
        let before: Vec<String> = packages.iter().chain(&tools).cloned().collect();
        if request.mode.is_dry_run() {
            return Ok(Outcome::new(SyncReceipt {
                status: ReceiptStatus::DryRun,
                installed: before,
                missing: Vec::new(),
            }));
        }
        if !packages.is_empty() {
            mise.run(&["bootstrap", "packages", "apply", "--yes"], progress)?;
        }
        if !tools.is_empty() {
            mise.run(&["install"], progress)?;
        }
        let (packages, tools) = missing(&machine)?;
        let still: Vec<String> = packages.into_iter().chain(tools).collect();
        let installed = before.into_iter().filter(|key| !still.contains(key)).collect();
        Ok(Outcome::new(SyncReceipt {
            status: ReceiptStatus::Completed,
            installed,
            missing: still,
        }))
    }
}

/// The declared packages and global tools that are not installed, on managers this machine has.
fn missing(machine: &Machine<'_>) -> Result<(Vec<String>, Vec<String>), Error> {
    let mise = &machine.environment.mise;
    let packages = mise
        .package_states()?
        .into_iter()
        .filter(|state| state.available && state.desired == "present" && state.state != "installed")
        .map(|state| state.key)
        .collect();
    let tools = mise
        .tool_states()?
        .into_iter()
        .filter(|tool| !tool.installed)
        .map(|tool| tool.key)
        .collect();
    Ok((packages, tools))
}
