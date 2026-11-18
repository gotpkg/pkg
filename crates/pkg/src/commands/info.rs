//! `pkg info`: one package, from its catalog and from mise.

use std::path::Path;

use pkg_core::{InfoRequest, PackageInfo};
use schemars::schema_for;
use serde_json::{Value, json};

use super::common::{PACKAGE, execute, for_people, table};
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::registry::{CommandSpec, Effect, Example, Requirement};
use crate::response::{RenderOutput, Response};

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

impl RenderOutput for PackageInfo {
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

    fn render_json(&self) -> Value {
        json!({
            "package": self.package,
            "manager": self.manager,
            "name": self.name,
            "description": self.description,
            "homepage": self.homepage,
            "version": self.version,
            "license": self.license,
            "dependencies": self.dependencies,
            "apps": self.apps,
            "caveats": self.caveats,
            "backend": self.backend,
            "deprecated": self.deprecated,
            "config": self.config.as_deref().map(Path::to_string_lossy),
            "installed": self.installed,
        })
    }
}

fn info(invocation: &Invocation) -> Result<Response, Failure> {
    let request = InfoRequest { package: invocation.string("package").unwrap_or_default() };
    execute(invocation, |client, progress| client.info(request, progress))
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
