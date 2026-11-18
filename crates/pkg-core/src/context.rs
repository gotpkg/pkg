//! Which mise, which configuration files, and which catalogs the client uses.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog;
use crate::catalog::homebrew::Homebrew;
use crate::common::{Machine, absolute};
use crate::mise::ConfigTarget;
use crate::{Client, Error, Outcome, Progress};

/// Where the client's operations take effect.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub mise: MiseContext,
    /// The home directory mise runs in.
    pub home: PathBuf,
    /// Where installs declare bootstrap packages.
    pub packages_config: ConfigContext,
    /// Where installs declare tools.
    pub tools_config: ConfigContext,
    /// Every global config file mise loads, symlinks resolved.
    pub config_files: Vec<PathBuf>,
    pub catalogs: Catalogs,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MiseContext {
    /// The executable the client runs.
    pub program: PathBuf,
    /// What `mise version` prints.
    pub version: String,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigContext {
    /// mise's `write_targets` setting as written; null when unset, which means the global config.
    pub setting: Option<String>,
    /// The file as mise loads it.
    pub path: PathBuf,
    /// The file with symlinks resolved: the checkout new declarations land in.
    pub real_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Catalogs {
    pub homebrew_api: String,
    pub app_store_api: String,
    /// The cached Homebrew search index.
    pub homebrew_index: PathBuf,
    /// When the cached index was downloaded, in seconds since the Unix epoch; null when there is
    /// none yet.
    pub homebrew_index_fetched_at: Option<u64>,
}

impl Client {
    /// Reports the mise executable and version, the home directory mise runs in, the files new
    /// bootstrap-package and tool declarations go to (mise's `write_targets` settings, else the
    /// global configuration) with symlinks resolved, every global configuration file mise loads,
    /// the catalog URLs, and the cached Homebrew search index.
    ///
    /// # Errors
    ///
    /// `PreconditionFailed` when mise cannot run or a configuration file cannot be read, and
    /// `MiseFailed` when mise cannot report its version, files, or settings.
    pub fn context(
        &mut self,
        _progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<Context>, Error> {
        let machine = Machine::load(&self.settings)?;
        let environment = &machine.environment;
        let mise_version = environment.mise.read(&["version"])?.trim().to_string();
        let client = catalog::Client::new(&self.settings.child_env)?;
        let homebrew = Homebrew::new(&client, environment.homebrew_api, environment.cache_dir);
        let describe = |target: ConfigTarget| ConfigContext {
            setting: target.setting,
            path: target.path,
            real_path: target.real_path,
        };
        Ok(Outcome::new(Context {
            mise: MiseContext {
                program: PathBuf::from(&self.settings.mise),
                version: mise_version,
            },
            home: environment.home.to_path_buf(),
            packages_config: describe(machine.target(false)?),
            tools_config: describe(machine.target(true)?),
            config_files: machine.config_files.iter().map(|path| absolute(path)).collect(),
            catalogs: Catalogs {
                homebrew_api: environment.homebrew_api.to_string(),
                app_store_api: environment.app_store_api.to_string(),
                homebrew_index: homebrew.index_path(),
                homebrew_index_fetched_at: homebrew.cached_index().map(|index| index.fetched_at),
            },
        }))
    }
}
