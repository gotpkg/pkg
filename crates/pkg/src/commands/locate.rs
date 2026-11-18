//! `pkg where`: the directory an installed formula or tool lives in.

use pkg_core::{LocateRequest, Location};
use schemars::schema_for;
use serde_json::{Value, json};

use super::common::{MISE_ERRORS, PACKAGE, execute};
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::registry::{CommandSpec, Effect, Example, Requirement};
use crate::response::{RenderOutput, Response};

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

impl RenderOutput for Location {
    fn render_text(&self) -> String {
        format!("{}\n", self.path.display())
    }

    fn render_json(&self) -> Value {
        json!({ "package": self.package, "path": self.path.to_string_lossy() })
    }
}

fn locate(invocation: &Invocation) -> Result<Response, Failure> {
    let request = LocateRequest { package: invocation.string("package").unwrap_or_default() };
    execute(invocation, |client, progress| client.locate(request, progress))
}
