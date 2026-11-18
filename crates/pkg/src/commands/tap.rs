//! `pkg tap`: Homebrew tap URLs for taps that do not live at `github.com/<owner>/homebrew-<tap>`.

use pkg_core::{ReceiptStatus, TapAddRequest, TapList, TapReceipt, TapRemoveRequest};
use schemars::schema_for;
use serde_json::{Value, json};

use super::common::{MISE_ERRORS, execute, for_people, table};
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::registry::{
    ArgumentKind, ArgumentSpec, CommandSpec, DRY_RUN, Effect, Example, Requirement, ValueKind,
};
use crate::response::{RenderOutput, Response};

const TAP: ArgumentSpec = ArgumentSpec {
    name: "tap",
    kind: ArgumentKind::Positional,
    value: ValueKind::String,
    required: true,
    default: None,
    multiple: false,
    summary: "The tap, as `owner/name`, which packages name as `brew:owner/name/<formula>`",
};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "tap.list",
        path: &["tap", "list"],
        aliases: &["ls"],
        summary: "List the Homebrew tap URLs your mise config declares",
        description: "Lists `[bootstrap.brew.taps]` across the global config files: each tap with \
            its Git URL and the file that declares it. mise finds a tap at \
            `https://github.com/<owner>/homebrew-<name>` without an entry, so most taps are not \
            listed. Never changes anything.",
        arguments: &[],
        effect: Effect::Reads,
        requires: &[Requirement::Mise],
        dry_run: false,
        idempotent: true,
        errors: &[ErrorCode::PreconditionFailed, ErrorCode::MiseFailed],
        examples: &[Example { summary: "List tap URLs", args: &["tap", "list"] }],
        synonyms: &["taps", "repositories", "sources", "show"],
        output_schema: || schema_for!(TapList),
        run: list,
    },
    CommandSpec {
        id: "tap.add",
        path: &["tap", "add"],
        aliases: &[],
        summary: "Record where a Homebrew tap's Git repository lives",
        description: "Adds `\"<owner>/<name>\" = \"<url>\"` to `[bootstrap.brew.taps]` in the file \
            mise writes packages to, with `mise bootstrap packages brew tap`. Only taps whose \
            repository is not `https://github.com/<owner>/homebrew-<name>` need one, and only \
            GitHub URLs are supported. Then install from it with \
            `pkg install brew:<owner>/<name>/<formula>`. --dry-run shows the change without \
            writing it.",
        arguments: &[
            TAP,
            ArgumentSpec {
                name: "url",
                kind: ArgumentKind::Positional,
                value: ValueKind::String,
                required: true,
                default: None,
                multiple: false,
                summary: "The tap's GitHub repository URL",
            },
            DRY_RUN,
        ],
        effect: Effect::Modifies("your global mise config"),
        requires: &[Requirement::Mise],
        dry_run: true,
        idempotent: true,
        errors: MISE_ERRORS,
        examples: &[
            Example {
                summary: "Preview recording a tap's URL",
                args: &[
                    "tap",
                    "add",
                    "acme/tools",
                    "https://github.com/acme/brew-tools.git",
                    "--dry-run",
                ],
            },
            Example {
                summary: "Record a tap's URL",
                args: &["tap", "add", "acme/tools", "https://github.com/acme/brew-tools.git"],
            },
        ],
        synonyms: &["tap", "repository", "url", "source", "third-party"],
        output_schema: || schema_for!(TapReceipt),
        run: add,
    },
    CommandSpec {
        id: "tap.remove",
        path: &["tap", "remove"],
        aliases: &["rm"],
        summary: "Forget a Homebrew tap's URL",
        description: "Removes the tap's entry from `[bootstrap.brew.taps]` with \
            `mise bootstrap packages brew untap`. Packages from the tap stay declared and \
            installed; check `pkg list --manager brew` first. pkg edits only mise's write target, \
            so a tap declared in another file fails with PRECONDITION_FAILED. --dry-run shows the \
            change without writing it.",
        arguments: &[TAP, DRY_RUN],
        effect: Effect::Modifies("your global mise config"),
        requires: &[Requirement::Mise],
        dry_run: true,
        idempotent: false,
        errors: &[ErrorCode::NotFound, ErrorCode::PreconditionFailed, ErrorCode::MiseFailed],
        examples: &[
            Example {
                summary: "Preview forgetting a tap",
                args: &["tap", "remove", "acme/tools", "--dry-run"],
            },
            Example { summary: "Forget a tap", args: &["tap", "remove", "acme/tools"] },
        ],
        synonyms: &["untap", "delete", "forget", "drop"],
        output_schema: || schema_for!(TapReceipt),
        run: remove,
    },
];

impl RenderOutput for TapList {
    fn render_text(&self) -> String {
        if self.items.is_empty() {
            return "No tap URLs; mise finds GitHub taps at github.com/<owner>/homebrew-<name>."
                .into();
        }
        let rows: Vec<Vec<String>> = self
            .items
            .iter()
            .map(|tap| vec![tap.tap.clone(), tap.url.clone(), for_people(&tap.config)])
            .collect();
        table(&rows)
    }

    fn render_json(&self) -> Value {
        let items: Vec<Value> = self
            .items
            .iter()
            .map(|tap| {
                json!({ "tap": tap.tap, "url": tap.url, "config": tap.config.to_string_lossy() })
            })
            .collect();
        json!({ "items": items })
    }
}

impl RenderOutput for TapReceipt {
    fn render_text(&self) -> String {
        let config = for_people(&self.config);
        let line = match (self.status, self.action.as_str()) {
            (ReceiptStatus::Completed, "added") => format!("Added {} to {config}", self.tap),
            (ReceiptStatus::DryRun, "added") => format!("Would add {} to {config}", self.tap),
            (ReceiptStatus::Completed, _) => format!("Removed {} from {config}", self.tap),
            (ReceiptStatus::DryRun, _) => format!("Would remove {} from {config}", self.tap),
        };
        format!("{line}\n")
    }

    fn render_json(&self) -> Value {
        json!({
            "status": self.status,
            "action": self.action,
            "tap": self.tap,
            "url": self.url,
            "config": self.config.to_string_lossy(),
        })
    }
}

fn list(invocation: &Invocation) -> Result<Response, Failure> {
    execute(invocation, |client, progress| client.tap_list(progress))
}

fn add(invocation: &Invocation) -> Result<Response, Failure> {
    let request = TapAddRequest {
        tap: invocation.string("tap").unwrap_or_default(),
        url: invocation.string("url").unwrap_or_default(),
        mode: invocation.mode(),
    };
    execute(invocation, |client, _| client.tap_add(request, &mut |_| {}))
}

fn remove(invocation: &Invocation) -> Result<Response, Failure> {
    let request = TapRemoveRequest {
        tap: invocation.string("tap").unwrap_or_default(),
        mode: invocation.mode(),
    };
    execute(invocation, |client, _| client.tap_remove(request, &mut |_| {}))
}

#[cfg(test)]
mod tests {
    use pkg_core::TapEntry;

    use super::*;

    #[test]
    fn list_json_matches_the_entry_fields_in_order() {
        let list = TapList {
            items: vec![TapEntry {
                tap: "acme/tools".into(),
                url: "https://git.example.com/acme/tools.git".into(),
                config: "/x/config.toml".into(),
            }],
        };
        assert_eq!(list.render_json().to_string(), serde_json::to_string(&list).unwrap());
    }

    #[test]
    fn receipt_json_matches_the_receipt_fields_in_order() {
        let receipt = TapReceipt {
            status: ReceiptStatus::DryRun,
            action: "removed".into(),
            tap: "acme/tools".into(),
            url: None,
            config: "/x/config.toml".into(),
        };
        assert_eq!(receipt.render_json().to_string(), serde_json::to_string(&receipt).unwrap());
    }
}
