//! `pkg where`: the directory an installed formula or tool lives in.

use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::json;

use super::common::{MISE_ERRORS, Machine, PACKAGE};
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::registry::{CommandSpec, Effect, Example, Requirement};
use crate::response::{RenderText, Response};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "where",
    path: &["where"],
    aliases: &[],
    summary: "Print where a formula or tool is installed",
    description: "Prints the directory an installed Homebrew formula (its `opt` path, such as \
        /opt/homebrew/opt/wget, which keg-only formulae need on PATH by hand) or mise tool lives \
        in, as `brew --prefix` and `mise where` do. Casks, App Store apps, and other managers' \
        packages have no such directory and fail with INVALID_ARGUMENT; something not installed \
        fails with MISE_FAILED. Never changes anything.",
    arguments: &[PACKAGE],
    effect: Effect::Reads,
    requires: &[Requirement::Mise],
    dry_run: false,
    idempotent: true,
    errors: MISE_ERRORS,
    examples: &[
        Example { summary: "Find wget's prefix", args: &["where", "brew:wget"] },
        Example { summary: "Find jq's install directory", args: &["where", "jq"] },
    ],
    synonyms: &["path", "location", "prefix", "opt", "directory", "folder", "which"],
    output_schema: || schema_for!(Location),
    run: locate,
}];

#[derive(Serialize, JsonSchema)]
pub struct Location {
    pub package: String,
    /// The absolute directory.
    pub path: String,
}

impl RenderText for Location {
    fn render_text(&self) -> String {
        format!("{}\n", self.path)
    }
}

fn locate(invocation: &Invocation) -> Result<Response, Failure> {
    let machine = Machine::load(invocation)?;
    let package = machine.package(&invocation.string("package").unwrap_or_default());
    if !package.is_tool() && package.manager() != Some("brew") {
        return Err(Failure::invalid_argument(format!(
            "{package} has no install directory to show; `where` takes a Homebrew formula \
             (`brew:`) or a mise tool"
        ))
        .with_details(json!({ "package": package.key })));
    }
    let path = machine.environment.mise.location(&package.key, package.is_tool())?;
    Ok(Response::new(&Location { package: package.key, path: path.display().to_string() }))
}
