//! What pkg asks mise: where it writes, what is declared and installed, and what is outdated.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::Mise;
use crate::environment::real_path;
use crate::error::{Error as Failure, ErrorDetails, ErrorKind, Recovery};

/// The config file mise writes a new declaration to.
#[derive(Clone, Debug)]
pub(crate) struct ConfigTarget {
    /// The `write_targets.*` setting as mise reports it; `None` when unset, which means the
    /// global config.
    pub setting: Option<String>,
    /// The file, as mise loads it.
    pub path: PathBuf,
    /// The same file with symlinks resolved: the checkout the declaration lands in.
    pub real_path: PathBuf,
}

/// One `[bootstrap.packages]` entry, as `mise bootstrap packages status --json` reports it.
#[derive(Clone, Debug)]
pub(crate) struct PackageState {
    /// `brew:wget`.
    pub key: String,
    pub manager: String,
    pub requested: String,
    /// `present`, or `absent` for a package the config wants removed.
    pub desired: String,
    /// `installed`, `missing`, or another state mise reports.
    pub state: String,
    pub installed: Option<String>,
    /// The app's name, which mise knows for `mas:` packages.
    pub name: Option<String>,
    /// Whether the manager can install on this machine; mise skips the rest.
    pub available: bool,
}

/// One `[tools]` entry of the global config, as `mise ls --global --json` reports it.
#[derive(Clone, Debug)]
pub(crate) struct ToolState {
    /// `jq`, `npm:prettier`.
    pub key: String,
    pub requested: String,
    pub version: Option<String>,
    pub installed: bool,
    /// The config file that requests it.
    pub config: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub(crate) struct OutdatedTool {
    pub key: String,
    pub current: Option<String>,
    pub latest: String,
}

/// A tool in mise's registry.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct RegistryTool {
    pub short: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// `mise tool <name> --json`.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ToolInfo {
    pub backend: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub requested_versions: Vec<String>,
}

#[derive(Deserialize)]
struct ManagerStatus {
    #[serde(default)]
    available: bool,
    #[serde(default)]
    packages: Vec<PackageStatus>,
}

#[derive(Deserialize)]
struct PackageStatus {
    package: String,
    #[serde(default)]
    requested_version: Option<String>,
    #[serde(default)]
    desired_state: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    installed_version: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct ToolVersion {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    requested_version: Option<String>,
    #[serde(default)]
    installed: bool,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    source: Option<Source>,
}

#[derive(Deserialize)]
struct Source {
    path: PathBuf,
}

#[derive(Deserialize)]
struct Outdated {
    #[serde(default)]
    current: Option<String>,
    latest: String,
}

#[derive(Deserialize)]
struct ConfigFile {
    path: PathBuf,
}

impl Mise<'_> {
    /// The file mise writes new `[bootstrap.packages]` (`setting` `packages`) or `[tools]`
    /// (`tools`) declarations to with `-g`: its `write_targets` setting, else `global_config`.
    ///
    /// # Errors
    ///
    /// `MISE_FAILED` when mise cannot read its settings.
    pub(crate) fn write_target(
        &self,
        setting: &str,
        global_config: &Path,
    ) -> Result<ConfigTarget, Failure> {
        let key = format!("write_targets.{setting}");
        let output = self.capture(&["settings", "get", &key])?;
        let setting = output.stdout.trim().to_string();
        if !output.success && !output.stderr.contains("is not set") {
            return Err(self.failure(&["settings", "get", &key], &output, true));
        }
        let (setting, path) = if output.success && !setting.is_empty() {
            let path = self.expand(&setting);
            (Some(setting), path)
        } else {
            (None, global_config.to_path_buf())
        };
        Ok(ConfigTarget { setting, real_path: real_path(&path), path })
    }

    fn expand(&self, path: &str) -> PathBuf {
        match path.strip_prefix("~/") {
            Some(rest) => self.home.join(rest),
            None => PathBuf::from(path),
        }
    }

    /// The prefixes that name a tool backend, such as `npm` and `github`.
    ///
    /// # Errors
    ///
    /// `MISE_FAILED` when mise cannot list them.
    pub(crate) fn tool_backends(&self) -> Result<Vec<String>, Failure> {
        Ok(self.read(&["backends", "ls"])?.split_whitespace().map(str::to_string).collect())
    }

    /// The global config files mise loads, in its order.
    ///
    /// # Errors
    ///
    /// `MISE_FAILED` when mise cannot list them or its answer is not JSON.
    pub(crate) fn config_files(&self) -> Result<Vec<PathBuf>, Failure> {
        let files: Vec<ConfigFile> = parse(&self.read(&["config", "ls", "--json"])?, "config ls")?;
        Ok(files.into_iter().map(|file| file.path).collect())
    }

    /// Every declared bootstrap package, in manager order, then config order.
    ///
    /// # Errors
    ///
    /// `MISE_FAILED` when mise cannot report them.
    pub(crate) fn package_states(&self) -> Result<Vec<PackageState>, Failure> {
        let output = self.read(&["bootstrap", "packages", "status", "--json"])?;
        let managers: BTreeMap<String, ManagerStatus> = if output.trim().is_empty() {
            BTreeMap::new()
        } else {
            parse(&output, "bootstrap packages status")?
        };
        Ok(managers
            .into_iter()
            .flat_map(|(manager, status)| {
                let available = status.available;
                status.packages.into_iter().map(move |package| PackageState {
                    key: format!("{manager}:{}", package.package),
                    manager: manager.clone(),
                    requested: package.requested_version.unwrap_or_else(|| "latest".into()),
                    desired: package.desired_state.unwrap_or_else(|| "present".into()),
                    state: package.state.unwrap_or_else(|| "unknown".into()),
                    installed: package.installed_version,
                    name: package.name,
                    available,
                })
            })
            .collect())
    }

    /// Every tool the global config requests, one entry per tool: the active request, else the
    /// first.
    ///
    /// # Errors
    ///
    /// `MISE_FAILED` when mise cannot list them.
    pub(crate) fn tool_states(&self) -> Result<Vec<ToolState>, Failure> {
        let tools: BTreeMap<String, Vec<ToolVersion>> =
            parse(&self.read(&["ls", "--global", "--json"])?, "ls --global")?;
        Ok(tools
            .into_iter()
            .filter_map(|(key, mut versions)| {
                let index = versions.iter().position(|version| version.active).unwrap_or(0);
                (index < versions.len()).then(|| {
                    let version = versions.swap_remove(index);
                    ToolState {
                        key,
                        requested: version.requested_version.unwrap_or_else(|| "latest".into()),
                        version: version.version,
                        installed: version.installed,
                        config: version.source.map(|source| source.path),
                    }
                })
            })
            .collect())
    }

    /// Global tools with a newer version than the installed one.
    ///
    /// # Errors
    ///
    /// `MISE_FAILED` when mise cannot check them.
    pub(crate) fn outdated_tools(&self) -> Result<Vec<OutdatedTool>, Failure> {
        let output = self.read(&["outdated", "--global", "--json"])?;
        let outdated: BTreeMap<String, Outdated> =
            if output.trim().is_empty() { BTreeMap::new() } else { parse(&output, "outdated")? };
        Ok(outdated
            .into_iter()
            .map(|(key, tool)| OutdatedTool { key, current: tool.current, latest: tool.latest })
            .collect())
    }

    /// mise's registry of tool names.
    ///
    /// # Errors
    ///
    /// `MISE_FAILED` when mise cannot list it.
    pub(crate) fn registry(&self) -> Result<Vec<RegistryTool>, Failure> {
        parse(&self.read(&["registry", "--json"])?, "registry")
    }

    /// What mise knows about a tool, or `None` when it knows no such tool.
    ///
    /// # Errors
    ///
    /// `PRECONDITION_FAILED` when mise cannot run.
    pub(crate) fn tool_info(&self, key: &str) -> Result<Option<ToolInfo>, Failure> {
        let output = self.capture(&["tool", key, "--json"])?;
        if !output.success {
            return Ok(None);
        }
        parse(&output.stdout, "tool").map(Some)
    }

    /// Where an installed Homebrew formula (`brew:<name>`) or tool lives.
    ///
    /// # Errors
    ///
    /// `MISE_FAILED` when it is not installed.
    pub(crate) fn location(&self, key: &str, tool: bool) -> Result<PathBuf, Failure> {
        let output = if tool {
            self.read(&["where", key])?
        } else {
            self.read(&["bootstrap", "packages", "where", key])?
        };
        Ok(PathBuf::from(output.trim()))
    }
}

fn parse<T: for<'de> Deserialize<'de>>(text: &str, command: &str) -> Result<T, Failure> {
    serde_json::from_str(text).map_err(|error| {
        Failure::new(
            ErrorKind::MiseFailed,
            format!("`mise {command}` printed something pkg cannot read: {error}"),
        )
        .with_details(ErrorDetails { command: Some(command.to_string()), ..Default::default() })
        .recover(Recovery::CheckMiseVersion)
    })
}
