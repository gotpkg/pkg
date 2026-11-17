//! `pkg list`: what the mise config declares, and whether each one is installed.

use schemars::{JsonSchema, schema_for};
use serde::Serialize;

use super::common::{MISE_ERRORS, Machine, absolute, table};
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::registry::{
    ArgumentKind, ArgumentSpec, CommandSpec, Effect, Example, Requirement, ValueKind,
};
use crate::response::{RenderText, Response};
use crate::search::{self, Page};

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

/// One declared package or tool.
#[derive(Clone, Serialize, JsonSchema)]
pub struct Listed {
    pub package: String,
    /// The package manager, such as `brew`, or `tool` for a mise tool.
    pub manager: String,
    /// A name mise knows besides the package, such as an App Store app's.
    pub name: Option<String>,
    /// The version the config asks for, usually `latest`.
    pub requested: String,
    pub installed: Option<String>,
    /// `installed`, `missing`, or another state mise reports, such as `unavailable` for a
    /// manager this machine does not have.
    pub state: String,
    /// The config file that declares it, symlinks resolved.
    pub config: Option<String>,
}

impl RenderText for Page<Listed> {
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
}

fn list(invocation: &Invocation) -> Result<Response, Failure> {
    let machine = Machine::load(invocation)?;
    let managers = invocation.strings("manager");
    let wants =
        |manager: &str| managers.is_empty() || managers.iter().any(|wanted| wanted == manager);
    let mise = &machine.environment.mise;
    let mut items: Vec<Listed> = Vec::new();
    if managers.is_empty() || managers.iter().any(|manager| manager != "tool") {
        for state in mise.package_states()? {
            if !wants(&state.manager) {
                continue;
            }
            items.push(Listed {
                config: machine.declarations.package_file(&state.key).map(absolute),
                state: if state.available { state.state } else { "unavailable".into() },
                package: state.key,
                manager: state.manager,
                name: state.name,
                requested: state.requested,
                installed: state.installed,
            });
        }
    }
    if wants("tool") {
        for tool in mise.tool_states()? {
            items.push(Listed {
                state: if tool.installed { "installed" } else { "missing" }.into(),
                installed: tool.installed.then_some(tool.version).flatten(),
                config: tool.config.as_deref().map(absolute),
                package: tool.key,
                manager: "tool".into(),
                name: None,
                requested: tool.requested,
            });
        }
    }
    let limit = invocation.integer("limit").unwrap_or(u64::MAX);
    let page = search::page(items, invocation.string("cursor").as_deref(), limit)?;
    Ok(Response::new(&page))
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
