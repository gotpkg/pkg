//! `pkg list`: what the mise config declares, and whether each one is installed.

use std::path::Path;

use pkg_core::matching::Page;
use pkg_core::{ListRequest, Listed};
use schemars::schema_for;
use serde_json::{Value, json};

use super::common::{MISE_ERRORS, execute, table};
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::registry::{
    ArgumentKind, ArgumentSpec, CommandSpec, Effect, Example, Requirement, ValueKind,
};
use crate::response::{RenderOutput, Response};

/// `--manager`, shared with the commands that filter by package manager.
pub const MANAGER: ArgumentSpec = ArgumentSpec {
    name: "manager",
    kind: ArgumentKind::Option,
    value: ValueKind::String,
    required: false,
    default: None,
    multiple: true,
    summary: "Only this package manager, such as `brew`, `brew-cask`, or `mas`, or `tool` for \
              mise tools; repeat for several",
};

pub const LIMIT: ArgumentSpec = ArgumentSpec {
    name: "limit",
    kind: ArgumentKind::Option,
    value: ValueKind::Integer { min: 1, max: None },
    required: false,
    default: None,
    multiple: false,
    summary: "The most entries to return; all of them when absent",
};

pub const CURSOR: ArgumentSpec = ArgumentSpec {
    name: "cursor",
    kind: ArgumentKind::Option,
    value: ValueKind::String,
    required: false,
    default: None,
    multiple: false,
    summary: "Resume after a previous page: the nextCursor it returned",
};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "list",
    path: &["list"],
    aliases: &["ls"],
    summary: "List the packages and tools your mise config declares",
    description: "Lists every `[bootstrap.packages]` entry of the global mise config, grouped by \
        package manager, then every global `[tools]` entry, each with its installed version or \
        `missing`. JSON has one item per entry with its `state` and `config` file. Pass --manager \
        to show one manager (`tool` for tools), and --limit with --cursor to page; without \
        --limit every entry is returned. Never changes anything.",
    arguments: &[MANAGER, LIMIT, CURSOR],
    effect: Effect::Reads,
    requires: &[Requirement::Mise],
    dry_run: false,
    idempotent: true,
    errors: MISE_ERRORS,
    examples: &[
        Example { summary: "List everything", args: &["list"] },
        Example { summary: "List the casks", args: &["list", "--manager", "brew-cask"] },
    ],
    synonyms: &["installed", "declared", "show", "all", "every", "inventory", "config"],
    output_schema: || schema_for!(Page<Listed>),
    run: list,
}];

impl RenderOutput for Page<Listed> {
    fn render_text(&self) -> String {
        if self.items.is_empty() {
            return "Your mise config declares no packages.".into();
        }
        let mut text = String::new();
        let mut managers: Vec<&str> = Vec::new();
        for item in &self.items {
            if !managers.contains(&item.manager.as_str()) {
                managers.push(&item.manager);
            }
        }
        for manager in managers {
            let items: Vec<&Listed> =
                self.items.iter().filter(|item| item.manager == manager).collect();
            let heading = if manager == "tool" { "tools" } else { manager };
            text.push_str(&format!("{heading} ({})\n", items.len()));
            let rows: Vec<Vec<String>> = items
                .iter()
                .map(|item| {
                    let name = if manager == "tool" {
                        item.package.clone()
                    } else {
                        item.package
                            .split_once(':')
                            .map_or(item.package.clone(), |(_, name)| name.to_string())
                    };
                    let version = match item.state.as_str() {
                        "installed" => item.installed.clone().unwrap_or_default(),
                        state => state.to_string(),
                    };
                    vec![name, version, item.name.clone().unwrap_or_default()]
                })
                .collect();
            text.push_str(&table(&rows));
        }
        if let Some(cursor) = &self.next_cursor {
            text.push_str(&format!("More follow; pass --cursor {cursor}.\n"));
        }
        text
    }

    fn render_json(&self) -> Value {
        let items: Vec<Value> = self
            .items
            .iter()
            .map(|item| {
                json!({
                    "package": item.package,
                    "manager": item.manager,
                    "name": item.name,
                    "requested": item.requested,
                    "installed": item.installed,
                    "state": item.state,
                    "config": item.config.as_deref().map(Path::to_string_lossy),
                })
            })
            .collect();
        json!({ "items": items, "hasMore": self.has_more, "nextCursor": self.next_cursor })
    }
}

fn list(invocation: &Invocation) -> Result<Response, Failure> {
    let request = ListRequest {
        managers: invocation.strings("manager"),
        limit: invocation.integer("limit"),
        cursor: invocation.string("cursor"),
    };
    execute(invocation, |client, progress| client.list(request, progress))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(package: &str, manager: &str, installed: Option<&str>, name: Option<&str>) -> Listed {
        Listed {
            package: package.into(),
            manager: manager.into(),
            name: name.map(str::to_string),
            requested: "latest".into(),
            installed: installed.map(str::to_string),
            state: if installed.is_some() { "installed" } else { "missing" }.into(),
            config: None,
        }
    }

    #[test]
    fn text_groups_by_manager_and_marks_missing_packages() {
        let page = Page {
            items: vec![
                listed("brew:wget", "brew", Some("1.25.0"), None),
                listed("brew:tree", "brew", None, None),
                listed("mas:937984704", "mas", Some("5.3.2"), Some("Amphetamine")),
                listed("npm:prettier", "tool", Some("3.6.2"), None),
            ],
            has_more: false,
            next_cursor: None,
        };
        assert_eq!(
            page.render_text(),
            "brew (2)\n  wget  1.25.0\n  tree  missing\nmas (1)\n  937984704  5.3.2  Amphetamine\ntools (1)\n  npm:prettier  3.6.2\n"
        );
    }
}
