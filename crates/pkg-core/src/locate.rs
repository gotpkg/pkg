//! The directory an installed Homebrew formula or mise tool lives in.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Serialize;

use crate::common::Machine;
use crate::requests::validate_package;
use crate::{Client, Error, ErrorDetails, LocateRequest, Outcome, Progress};

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct Location {
    pub package: String,
    /// The absolute directory.
    pub path: PathBuf,
}

impl Client {
    /// The directory an installed Homebrew formula (its `opt` prefix) or mise tool lives in.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for a name that is not a package or a package that is neither a
    /// Homebrew formula nor a tool, `PreconditionFailed` when mise cannot run or a configuration
    /// file cannot be read, and `MiseFailed` when the package is not installed.
    pub fn locate(
        &mut self,
        request: LocateRequest,
        _progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<Location>, Error> {
        validate_package(&request.package)?;
        let machine = Machine::load(&self.settings)?;
        let package = machine.package(&request.package);
        if !package.is_tool() && package.manager() != Some("brew") {
            return Err(Error::invalid_argument(format!(
                "{package} has no install directory to show; `where` takes a Homebrew formula \
                 (`brew:`) or a mise tool"
            ))
            .with_details(ErrorDetails { package: Some(package.key), ..Default::default() }));
        }
        let path = machine.environment.mise.location(&package.key, package.is_tool())?;
        Ok(Outcome::new(Location { package: package.key, path }))
    }
}
