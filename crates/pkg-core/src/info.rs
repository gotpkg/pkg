//! One package, from its catalog and from mise.

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog;
use crate::catalog::app_store::AppStore;
use crate::catalog::homebrew::Homebrew;
use crate::common::{Machine, absolute};
use crate::package::Package;
use crate::requests::validate_package;
use crate::{Client, Error, ErrorDetails, InfoRequest, Outcome, Progress, Recovery};

/// Everything pkg knows about one package.
#[derive(Clone, Debug, Default, Serialize, JsonSchema)]
pub struct PackageInfo {
    pub package: String,
    /// The package manager, such as `brew`, or `tool` for a mise tool.
    pub manager: String,
    /// A display name, such as a cask's or an app's title.
    pub name: Option<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    /// The newest version the catalog offers.
    pub version: Option<String>,
    pub license: Option<String>,
    /// Formulae a Homebrew formula needs.
    pub dependencies: Vec<String>,
    /// Apps a cask puts in /Applications.
    pub apps: Vec<String>,
    pub caveats: Option<String>,
    /// For a tool, the mise backend that installs it, such as `aqua:jqlang/jq`.
    pub backend: Option<String>,
    /// Whether the catalog marks it deprecated.
    pub deprecated: bool,
    /// The config file that declares it, symlinks resolved; null when none does.
    pub config: Option<PathBuf>,
    /// The installed version; null when it is not installed.
    pub installed: Option<String>,
}

impl Client {
    /// Describes a package from its catalog (Homebrew's API, the App Store, or mise for tools),
    /// then adds the configuration file that declares it and its installed version. Packages
    /// from third-party Homebrew taps have no catalog entry, so only the local facts are filled.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for a name that is not a package, `NotFound` when no catalog knows the
    /// package and no configuration declares it, `Unavailable` when its catalog cannot be
    /// reached, `PreconditionFailed` when mise cannot run or a configuration file cannot be
    /// read, and `MiseFailed` when mise cannot answer.
    pub fn info(
        &mut self,
        request: InfoRequest,
        _progress: &mut dyn FnMut(Progress<'_>),
    ) -> Result<Outcome<PackageInfo>, Error> {
        validate_package(&request.package)?;
        let machine = Machine::load(&self.settings)?;
        let package = machine.package(&request.package);
        let client = catalog::Client::new(&self.settings.child_env)?;
        let mise = &machine.environment.mise;
        let mut info = PackageInfo {
            package: package.key.clone(),
            manager: package.manager().unwrap_or("tool").to_string(),
            ..PackageInfo::default()
        };
        let cataloged = describe(&machine, &client, &package, &mut info)?;

        info.config = machine.declared_in(&package).map(absolute);
        info.installed = if package.is_tool() {
            mise.tool_states()?
                .into_iter()
                .find(|tool| tool.key == package.key && tool.installed)
                .and_then(|tool| tool.version)
        } else if info.config.is_some() {
            mise.package_states()?
                .into_iter()
                .find(|state| state.key == package.key && state.state == "installed")
                .and_then(|state| state.installed)
        } else {
            None
        };
        if !cataloged && info.config.is_none() && info.installed.is_none() {
            return Err(Error::not_found(format!(
                "no catalog knows {package}, and no config declares it"
            ))
            .with_details(ErrorDetails { package: Some(package.key.clone()), ..Default::default() })
            .recover(Recovery::Search(package.name().to_string())));
        }
        Ok(Outcome::new(info))
    }
}

/// Fills `info` from the package's catalog; `false` when the catalog does not know it.
fn describe(
    machine: &Machine<'_>,
    client: &catalog::Client,
    package: &Package,
    info: &mut PackageInfo,
) -> Result<bool, Error> {
    let environment = &machine.environment;
    let homebrew = Homebrew::new(client, environment.homebrew_api, environment.cache_dir);
    let name = package.name();
    // Homebrew's API serves only its own taps; `owner/tap/name` lives elsewhere.
    let tapped = name.contains('/');
    match package.manager() {
        Some("brew") if !tapped => {
            let Some(formula) = homebrew.formula(name)? else { return Ok(false) };
            info.version = formula.version();
            info.description = formula.desc;
            info.homepage = formula.homepage;
            info.license = formula.license;
            info.dependencies = formula.dependencies;
            info.caveats = formula.caveats;
            info.deprecated = formula.deprecated || formula.disabled;
            Ok(true)
        }
        Some("brew-cask") if !tapped => {
            let Some(cask) = homebrew.cask(name)? else { return Ok(false) };
            info.apps = cask.apps();
            info.name = cask.name.first().cloned();
            info.version = cask.version;
            info.description = cask.desc;
            info.homepage = cask.homepage;
            info.caveats = cask.caveats;
            info.deprecated = cask.deprecated || cask.disabled;
            Ok(true)
        }
        Some("mas") if name.chars().all(|character| character.is_ascii_digit()) => {
            let store = AppStore::new(client, environment.app_store_api);
            let Some(app) = store.lookup(&[name])?.into_iter().next() else { return Ok(false) };
            info.name = Some(app.track_name);
            info.version = app.version;
            info.description = app.description.as_deref().map(first_sentence);
            info.homepage = app.track_view_url;
            Ok(true)
        }
        None => {
            let Some(tool) = environment.mise.tool_info(&package.key)? else { return Ok(false) };
            info.backend = Some(tool.backend);
            info.description = tool.description;
            info.homepage = tool.url;
            info.version = tool.requested_versions.first().cloned();
            Ok(true)
        }
        Some(_) => Ok(false),
    }
}

/// The first sentence of an App Store description, which runs to several paragraphs.
fn first_sentence(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    match line.find(". ") {
        Some(end) => line[..=end].to_string(),
        None => line.to_string(),
    }
}
