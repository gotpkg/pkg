//! `pkg info`: one package, from its catalog and from mise.

use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::json;

use super::common::{Machine, PACKAGE, absolute, for_people, table};
use crate::NAME;
use crate::catalog::Client;
use crate::catalog::app_store::AppStore;
use crate::catalog::homebrew::Homebrew;
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::package::Package;
use crate::registry::{CommandSpec, Effect, Example, Requirement};
use crate::response::{RenderText, Response};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "info",
    path: &["info"],
    aliases: &[],
    summary: "Describe one package and whether it is installed",
    description: "Describes a package from its catalog: Homebrew's API for `brew:` and \
        `brew-cask:` (description, homepage, version, license, dependencies, the apps a cask \
        installs, caveats), the App Store for `mas:`, and mise for tools (backend, description, \
        versions). Then adds what mise knows locally: the config file that declares it and the \
        installed version. Formulae and casks from third-party taps are not in Homebrew's API, \
        so only the local part is shown for them. A package no catalog knows and no config \
        declares fails with NOT_FOUND. Never changes anything.",
    arguments: &[PACKAGE],
    effect: Effect::Reads,
    requires: &[Requirement::Mise, Requirement::Network],
    dry_run: false,
    idempotent: true,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::NotFound,
        ErrorCode::PreconditionFailed,
        ErrorCode::Unavailable,
        ErrorCode::MiseFailed,
    ],
    examples: &[
        Example { summary: "Describe a formula", args: &["info", "brew:wget"] },
        Example { summary: "Describe a cask", args: &["info", "brew-cask:firefox"] },
        Example { summary: "Describe an App Store app", args: &["info", "mas:937984704"] },
    ],
    synonyms: &["show", "details", "describe", "about", "homepage", "version", "dependencies"],
    output_schema: || schema_for!(PackageInfo),
    run: info,
}];

/// Everything pkg knows about one package.
#[derive(Default, Serialize, JsonSchema)]
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
    pub config: Option<String>,
    /// The installed version; null when it is not installed.
    pub installed: Option<String>,
}

impl RenderText for PackageInfo {
    fn render_text(&self) -> String {
        let mut text = self.package.clone();
        if let Some(name) = &self.name {
            text.push_str(&format!(" ({name})"));
        }
        if let Some(description) = &self.description {
            text.push_str(&format!(": {description}"));
        }
        text.push('\n');
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut row = |label: &str, value: Option<String>| {
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                rows.push(vec![label.to_string(), value]);
            }
        };
        row("version", self.version.clone());
        row("installed", Some(self.installed.clone().unwrap_or_else(|| "no".into())));
        row("declared in", Some(self.config.as_deref().map_or("no config".into(), for_people)));
        row("backend", self.backend.clone());
        row("homepage", self.homepage.clone());
        row("license", self.license.clone());
        row("depends on", Some(self.dependencies.join(", ")));
        row("apps", Some(self.apps.join(", ")));
        row("deprecated", self.deprecated.then(|| "yes".to_string()));
        text.push_str(&table(&rows));
        if let Some(caveats) = &self.caveats {
            text.push_str("\nCaveats:\n");
            for line in caveats.trim_end().lines() {
                text.push_str(&format!("  {line}\n"));
            }
        }
        text
    }
}

fn info(invocation: &Invocation) -> Result<Response, Failure> {
    let machine = Machine::load(invocation)?;
    let raw = invocation.string("package").unwrap_or_default();
    let package = machine.package(&raw);
    let client = Client::default();
    let environment = &machine.environment;
    let mut info = PackageInfo {
        package: package.key.clone(),
        manager: package.manager().unwrap_or("tool").to_string(),
        ..PackageInfo::default()
    };
    let cataloged = describe(&machine, &client, &package, &mut info)?;

    info.config = machine.declared_in(&package).map(absolute);
    info.installed = if package.is_tool() {
        environment
            .mise
            .tool_states()?
            .into_iter()
            .find(|tool| tool.key == package.key && tool.installed)
            .and_then(|tool| tool.version)
    } else if info.config.is_some() {
        environment
            .mise
            .package_states()?
            .into_iter()
            .find(|state| state.key == package.key && state.state == "installed")
            .and_then(|state| state.installed)
    } else {
        None
    };
    if !cataloged && info.config.is_none() && info.installed.is_none() {
        return Err(Failure::not_found(format!(
            "no catalog knows {package}, and no config declares it"
        ))
        .with_details(json!({ "package": package.key }))
        .suggest(format!("{NAME} search {}", package.name())));
    }
    Ok(Response::new(&info))
}

/// Fills `info` from the package's catalog; `false` when the catalog does not know it.
fn describe(
    machine: &Machine,
    client: &Client,
    package: &Package,
    info: &mut PackageInfo,
) -> Result<bool, Failure> {
    let environment = &machine.environment;
    let homebrew = Homebrew::new(client, &environment.homebrew_api, &environment.cache_dir);
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
            let store = AppStore::new(client, &environment.app_store_api);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_leads_with_the_package_and_skips_empty_facts() {
        let info = PackageInfo {
            package: "brew:wget".into(),
            manager: "brew".into(),
            description: Some("Internet file retriever".into()),
            version: Some("1.25.0_2".into()),
            dependencies: vec!["libidn2".into(), "openssl@4".into()],
            caveats: Some("Read the manual.\n".into()),
            ..PackageInfo::default()
        };
        assert_eq!(
            info.render_text(),
            "brew:wget: Internet file retriever\n  version      1.25.0_2\n  installed    no\n  declared in  no config\n  depends on   libidn2, openssl@4\n\nCaveats:\n  Read the manual.\n"
        );
    }
}
