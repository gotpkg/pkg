//! `pkg install`: install packages and record them in the mise config.

use pkg_core::{InstallAction, InstallReceipt, InstallRequest, ReceiptStatus};
use schemars::schema_for;
use serde_json::{Value, json};

use super::common::{PACKAGES, execute, for_people};
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::registry::{CommandSpec, DRY_RUN, Effect, Example, Requirement};
use crate::response::{RenderOutput, Response};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "install",
    path: &["install"],
    aliases: &["i", "add"],
    summary: "Install packages and record them in your mise config",
    description: "Installs each package with mise and declares it in your global mise config, so \
        `mise bootstrap` installs it again on a new machine. Homebrew formulae (`brew:`), casks \
        (`brew-cask:`), App Store apps (`mas:`), and every other mise package manager go in \
        `[bootstrap.packages]` of the file mise's `write_targets.packages` setting names; tools \
        (`npm:prettier`, `github:cli/cli`, or a registry name such as `ripgrep`) go in `[tools]` \
        of the `write_targets.tools` file, at `latest` unless you pass `@<version>`. A package \
        some config already declares is installed where it is declared, and one that is already \
        installed is left alone, so installing twice is safe. Packages install one at a time in \
        the order given; when mise fails, pkg takes that package's new declaration back out and \
        stops, and the error lists what was installed before it. mise's progress goes to stderr.",
    arguments: &[PACKAGES, DRY_RUN],
    effect: Effect::Modifies("installed software and your global mise config"),
    requires: &[Requirement::Mise],
    dry_run: true,
    idempotent: true,
    errors: &[ErrorCode::InvalidArgument, ErrorCode::PreconditionFailed, ErrorCode::MiseFailed],
    examples: &[
        Example {
            summary: "Preview installing GNU Hello, changing nothing",
            args: &["install", "brew:hello", "--dry-run"],
        },
        Example {
            summary: "Install a formula and a cask",
            args: &["install", "brew:hello", "brew-cask:iina"],
        },
        Example { summary: "Install ripgrep from mise's registry", args: &["install", "ripgrep"] },
    ],
    synonyms: &[
        "add", "get", "setup", "declare", "brew", "homebrew", "formula", "cask", "app", "tool",
    ],
    output_schema: || schema_for!(InstallReceipt),
    run: install,
}];

impl RenderOutput for InstallReceipt {
    fn render_text(&self) -> String {
        let dry = self.status == ReceiptStatus::DryRun;
        let mut text = String::new();
        for item in &self.packages {
            let package = &item.package;
            let version =
                item.version.as_ref().map(|version| format!(" {version}")).unwrap_or_default();
            let config = for_people(&item.config);
            let line = match (item.action, dry) {
                (InstallAction::Added, false) => {
                    format!("Installed {package}{version}, declared in {config}")
                }
                (InstallAction::Added, true) => {
                    format!("Would install {package} and declare it in {config}")
                }
                (InstallAction::Installed, false) => {
                    format!("Installed {package}{version}, already declared in {config}")
                }
                (InstallAction::Installed, true) => {
                    format!("Would install {package}, already declared in {config}")
                }
                (InstallAction::Declared, false) => format!(
                    "Declared {package} in {config}; its package manager is not available on this \
                     machine"
                ),
                (InstallAction::Declared, true) => format!("Would declare {package} in {config}"),
                (InstallAction::Unchanged, _) => {
                    format!("{package}{version} is already installed, declared in {config}")
                }
            };
            text.push_str(&line);
            text.push('\n');
        }
        if dry {
            if !self.plan.is_empty() {
                text.push_str("\nmise's plan:\n");
                for line in &self.plan {
                    text.push_str(&format!("  {line}\n"));
                }
            }
            text.push_str("Dry run: nothing was installed or written.\n");
        }
        text
    }

    fn render_json(&self) -> Value {
        let packages: Vec<Value> = self
            .packages
            .iter()
            .map(|item| {
                json!({
                    "package": item.package,
                    "kind": item.kind,
                    "action": item.action,
                    "config": item.config.to_string_lossy(),
                    "version": item.version,
                })
            })
            .collect();
        json!({ "status": self.status, "packages": packages, "plan": self.plan })
    }
}

fn install(invocation: &Invocation) -> Result<Response, Failure> {
    let request =
        InstallRequest { packages: invocation.strings("packages"), mode: invocation.mode() };
    execute(invocation, |client, progress| client.install(request, progress))
}

#[cfg(test)]
mod tests {
    use pkg_core::{InstallKind, Installed};

    use super::*;

    #[test]
    fn text_names_the_package_version_and_config() {
        let receipt = InstallReceipt {
            status: ReceiptStatus::Completed,
            packages: vec![Installed {
                package: "brew:wget".into(),
                kind: InstallKind::Package,
                action: InstallAction::Added,
                config: "/nowhere/config.macos.toml".into(),
                version: Some("1.25.0".into()),
            }],
            plan: Vec::new(),
        };
        assert_eq!(
            receipt.render_text(),
            "Installed brew:wget 1.25.0, declared in /nowhere/config.macos.toml\n"
        );
    }

    #[test]
    fn json_matches_the_receipt_fields_in_order() {
        let receipt = InstallReceipt {
            status: ReceiptStatus::DryRun,
            packages: vec![Installed {
                package: "npm:prettier".into(),
                kind: InstallKind::Tool,
                action: InstallAction::Installed,
                config: "/nowhere/config.toml".into(),
                version: None,
            }],
            plan: vec!["install npm:prettier@latest".into()],
        };
        assert_eq!(receipt.render_json().to_string(), serde_json::to_string(&receipt).unwrap());
    }
}
