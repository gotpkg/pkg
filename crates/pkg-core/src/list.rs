//! What the global mise configuration declares, and whether each entry is installed.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Serialize;

use crate::common::{Machine, absolute};
use crate::matching::{self, Page};
use crate::requests::validate_limit;
use crate::{Client, Error, ListRequest, Outcome, Progress};

/// One declared package or tool.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct Listed {
    pub package: String,
    /// The package manager, such as `brew`, or `tool` for a mise tool.
    pub manager: String,
    /// A name mise knows besides the package, such as an App Store app's.
    pub name: Option<String>,
    /// The version the config asks for, usually `latest`.
    pub requested: String,
    pub installed: Option<String>,
    /// `installed`, `missing`, or another state mise reports, such as `unavailable` for a
    /// manager this machine does not have.
    pub state: String,
    /// The config file that declares it, symlinks resolved.
    pub config: Option<PathBuf>,
}

impl Client {
    /// Every `[bootstrap.packages]` entry, in manager then configuration order, followed by every
    /// global `[tools]` entry, each with its installed state. A nonempty `managers` keeps only
    /// those managers; `tool` selects tools. Without a limit, the page holds every entry.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for a zero limit or a cursor this operation did not return,
    /// `PreconditionFailed` when mise cannot run or a configuration file cannot be read, and
    /// `MiseFailed` when mise cannot report the declarations.
    pub fn list(
        &mut self,
        request: ListRequest,
        _progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<Page<Listed>>, Error> {
        if let Some(limit) = request.limit {
            validate_limit(limit, None)?;
        }
        let machine = Machine::load(&self.settings)?;
        let managers = &request.managers;
        let wants =
            |manager: &str| managers.is_empty() || managers.iter().any(|wanted| wanted == manager);
        let mise = &machine.environment.mise;
        let mut items: Vec<Listed> = Vec::new();
        if managers.is_empty() || managers.iter().any(|manager| manager != "tool") {
            for state in machine.package_states()? {
                if !wants(&state.manager) {
                    continue;
                }
                items.push(Listed {
                    config: machine.declarations.package_file(&state.key).map(absolute),
                    state: if state.available { state.state } else { "unavailable".into() },
                    package: state.key,
                    manager: state.manager,
                    name: state.name,
                    requested: state.requested,
                    installed: state.installed,
                });
            }
        }
        if wants("tool") {
            for tool in mise.tool_states()? {
                items.push(Listed {
                    state: if tool.installed { "installed" } else { "missing" }.into(),
                    installed: tool.installed.then_some(tool.version).flatten(),
                    config: tool.config.as_deref().map(absolute),
                    package: tool.key,
                    manager: "tool".into(),
                    name: None,
                    requested: tool.requested,
                });
            }
        }
        let limit = request.limit.unwrap_or(u64::MAX);
        let page = matching::page(items, request.cursor.as_deref(), limit)?;
        Ok(Outcome::new(page))
    }
}
