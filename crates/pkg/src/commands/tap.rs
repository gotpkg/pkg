//! `pkg tap`: Homebrew tap URLs for taps that do not live at `github.com/<owner>/homebrew-<tap>`.

use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::json;

use super::common::{MISE_ERRORS, Machine, ReceiptStatus, absolute, for_people, table};
use crate::NAME;
use crate::declarations::same_file;
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::registry::{
    ArgumentKind, ArgumentSpec, CommandSpec, DRY_RUN, Effect, Example, Requirement, ValueKind,
};
use crate::response::{RenderText, Response};

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

/// Every declared tap URL.
#[derive(Serialize, JsonSchema)]
pub struct TapList {
    pub items: Vec<TapEntry>,
}

#[derive(Serialize, JsonSchema)]
pub struct TapEntry {
    pub tap: String,
    pub url: String,
    /// The config file that declares it, symlinks resolved.
    pub config: String,
}

/// The outcome of `pkg tap add` and `pkg tap remove`.
#[derive(Serialize, JsonSchema)]
pub struct TapReceipt {
    pub status: ReceiptStatus,
    /// `added` or `removed`; for `dry-run`, what would happen.
    pub action: String,
    pub tap: String,
    pub url: Option<String>,
    /// The config file changed, symlinks resolved.
    pub config: String,
}

impl RenderText for TapList {
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
}

impl RenderText for TapReceipt {
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
}

fn list(invocation: &Invocation) -> Result<Response, Failure> {
    let machine = Machine::load(invocation)?;
    let items = machine
        .declarations
        .taps()
        .into_iter()
        .map(|tap| TapEntry { tap: tap.name, url: tap.url, config: absolute(&tap.config) })
        .collect();
    Ok(Response::new(&TapList { items }))
}

fn add(invocation: &Invocation) -> Result<Response, Failure> {
    let machine = Machine::load(invocation)?;
    let tap = invocation.string("tap").unwrap_or_default();
    let url = invocation.string("url").unwrap_or_default();
    let target = machine.target(false)?;
    let path = target.path.display().to_string();
    let mut args = vec!["bootstrap", "packages", "brew", "tap", "-p", &path];
    if invocation.dry_run() {
        args.push("--dry-run");
    }
    args.extend([tap.as_str(), url.as_str()]);
    machine.environment.mise.query(&args)?;
    Ok(Response::new(&TapReceipt {
        status: status(invocation),
        action: "added".into(),
        tap,
        url: Some(url),
        config: absolute(&target.path),
    }))
}

fn remove(invocation: &Invocation) -> Result<Response, Failure> {
    let machine = Machine::load(invocation)?;
    let tap = invocation.string("tap").unwrap_or_default();
    let target = machine.target(false)?;
    let declared = machine.declarations.taps().into_iter().find(|entry| entry.name == tap);
    let Some(declared) = declared else {
        return Err(Failure::not_found(format!("no config declares a URL for the tap {tap}"))
            .with_details(json!({ "tap": tap }))
            .suggest(format!("{NAME} tap list")));
    };
    if !same_file(&declared.config, &target.path) {
        return Err(Failure::precondition(format!(
            "{tap} is declared in {}, and pkg only edits {}",
            machine.show(&declared.config),
            machine.show(&target.path)
        ))
        .with_details(json!({ "tap": tap, "config": absolute(&declared.config) }))
        .suggest(format!("Remove `{tap}` from {} by hand", machine.show(&declared.config))));
    }
    let path = target.path.display().to_string();
    let mut args = vec!["bootstrap", "packages", "brew", "untap", "-p", &path];
    if invocation.dry_run() {
        args.push("--dry-run");
    }
    args.push(&tap);
    machine.environment.mise.query(&args)?;
    Ok(Response::new(&TapReceipt {
        status: status(invocation),
        action: "removed".into(),
        url: Some(declared.url),
        tap,
        config: absolute(&target.path),
    }))
}

fn status(invocation: &Invocation) -> ReceiptStatus {
    if invocation.dry_run() { ReceiptStatus::DryRun } else { ReceiptStatus::Completed }
}
