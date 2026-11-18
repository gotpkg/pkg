//! `pkg outdated`: installed packages and tools with a newer version, which `update` installs.

use pkg_core::OutdatedReport;
use schemars::schema_for;

use super::common::{execute, table};
use crate::NAME;
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::registry::{CommandSpec, Effect, Example, Requirement};
use crate::response::{RenderOutput, Response};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "outdated",
    path: &["outdated"],
    aliases: &[],
    summary: "List installed packages that have a newer version",
    description: "Lists every installed package and tool that `pkg update` would update, with \
        the installed and the newest version, mise itself first. Homebrew formulae and casks \
        come from `mise bootstrap packages upgrade --dry-run`, which refreshes Homebrew's \
        metadata; App Store apps are compared with the App Store's current version; tools come \
        from `mise outdated --global`; mise's newest release comes from `mise version`, before \
        mise's minimum release age, so `pkg update` can leave a very new mise release for \
        later. Casks mise will not update, such as apps that update themselves, and a newer \
        mise that a package manager installed are listed under `skipped` with the reason. The \
        whole list is returned at once. Never changes anything.",
    arguments: &[],
    effect: Effect::Reads,
    requires: &[Requirement::Mise, Requirement::Network],
    dry_run: false,
    idempotent: true,
    errors: &[ErrorCode::PreconditionFailed, ErrorCode::MiseFailed],
    examples: &[Example { summary: "See what update would update", args: &["outdated"] }],
    synonyms: &["old", "stale", "newer", "updates", "behind", "upgradable", "available"],
    output_schema: || schema_for!(OutdatedReport),
    run: outdated,
}];

impl RenderOutput for OutdatedReport {
    fn render_text(&self) -> String {
        let mut text = if self.items.is_empty() {
            "Everything is up to date.\n".to_string()
        } else {
            let rows: Vec<Vec<String>> = self
                .items
                .iter()
                .map(|item| {
                    let installed = item.installed.clone().unwrap_or_else(|| "?".into());
                    vec![item.package.clone(), format!("{installed} -> {}", item.latest)]
                })
                .collect();
            format!("{}Update with `{NAME} update`.\n", table(&rows))
        };
        if !self.skipped.is_empty() {
            text.push_str("\nmise leaves these alone:\n");
            let rows: Vec<Vec<String>> = self
                .skipped
                .iter()
                .map(|skip| vec![skip.package.clone(), skip.reason.clone()])
                .collect();
            text.push_str(&table(&rows));
        }
        text
    }
}

fn outdated(invocation: &Invocation) -> Result<Response, Failure> {
    execute(invocation, |client, progress| client.outdated(progress))
}

#[cfg(test)]
mod tests {
    use pkg_core::{OutdatedPackage, Skipped};

    use super::*;

    #[test]
    fn text_shows_each_update_and_what_mise_skips() {
        let report = OutdatedReport {
            items: vec![OutdatedPackage {
                package: "brew:wget".into(),
                installed: Some("1.24.5".into()),
                latest: "1.25.0".into(),
            }],
            skipped: vec![Skipped {
                package: "brew-cask:1password".into(),
                reason: "updates itself".into(),
            }],
        };
        assert_eq!(
            report.render_text(),
            "  brew:wget  1.24.5 -> 1.25.0\nUpdate with `pkg update`.\n\nmise leaves these alone:\n  brew-cask:1password  updates itself\n"
        );
    }
}
