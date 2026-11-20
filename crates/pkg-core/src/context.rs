//! Which mise, which configuration files, and which catalogs the client uses.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog;
use crate::catalog::homebrew::Homebrew;
use crate::common::{Machine, absolute};
use crate::environment::real_path;
use crate::mise::{ConfigTarget, Mise};
use crate::onboarding::handoff::PKG_TOOL;
use crate::{Client, Error, Outcome, Progress, Recovery};

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
    /// pkg itself as a mise tool (`github:gotpkg/pkg`), when a config declares it.
    pub pkg_tool: Option<PkgTool>,
    pub catalogs: Catalogs,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PkgTool {
    /// The version the config asks for, such as `latest`.
    pub requested: String,
    /// The version mise has installed; null when none is.
    pub installed: Option<String>,
    /// The config file that declares it.
    pub config: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MiseContext {
    /// The executable the client runs.
    pub program: PathBuf,
    /// Whether that executable exists and runs.
    pub found: bool,
    /// What `mise version` prints; null when mise is not found.
    pub version: Option<String>,
    /// Whether mise can replace itself, which `update` has it do; false for a mise a package
    /// manager installed.
    pub self_updates: bool,
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
    /// the catalog URLs, and the cached Homebrew search index. Without mise, it reports mise as
    /// not found, the global configuration as both targets, and no configuration files.
    ///
    /// # Errors
    ///
    /// `PreconditionFailed` when mise exists but cannot run or a configuration file cannot be
    /// read, and `MiseFailed` when mise cannot report its version, files, or settings.
    pub fn context(
        &mut self,
        _progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<Context>, Error> {
        let mise = Mise::new(&self.settings);
        let release = match mise.release() {
            Ok(release) => Some(release),
            Err(error) if error.recovery.contains(&Recovery::InstallMise) => None,
            Err(error) => return Err(error),
        };
        let client = catalog::Client::new(&self.settings.child_env)?;
        let homebrew =
            Homebrew::new(&client, &self.settings.homebrew_api, &self.settings.cache_dir);
        let describe = |target: ConfigTarget| ConfigContext {
            setting: target.setting,
            path: target.path,
            real_path: target.real_path,
        };
        let (packages_config, tools_config, config_files, pkg_tool) = if release.is_some() {
            let machine = Machine::load(&self.settings)?;
            let pkg_tool = machine
                .environment
                .mise
                .tool_states()?
                .into_iter()
                .find(|tool| tool.key == PKG_TOOL)
                .map(|tool| PkgTool {
                    requested: tool.requested,
                    installed: tool.installed.then_some(tool.version).flatten(),
                    config: tool.config,
                });
            (
                describe(machine.target(false)?),
                describe(machine.target(true)?),
                machine.config_files.iter().map(|path| absolute(path)).collect(),
                pkg_tool,
            )
        } else {
            let global = self.settings.global_config();
            let target = || ConfigContext {
                setting: None,
                real_path: real_path(&global),
                path: global.clone(),
            };
            (target(), target(), Vec::new(), None)
        };
        Ok(Outcome::new(Context {
            mise: MiseContext {
                program: mise.executable().unwrap_or_else(|| PathBuf::from(&self.settings.mise)),
                found: release.is_some(),
                self_updates: release.is_some() && mise.updates_itself(),
                version: release.map(|release| release.description),
            },
            home: self.settings.home.clone(),
            packages_config,
            tools_config,
            config_files,
            pkg_tool,
            catalogs: Catalogs {
                homebrew_api: self.settings.homebrew_api.clone(),
                app_store_api: self.settings.app_store_api.clone(),
                homebrew_index: homebrew.index_path(),
                homebrew_index_fetched_at: homebrew.cached_index().map(|index| index.fetched_at),
            },
        }))
    }
}
