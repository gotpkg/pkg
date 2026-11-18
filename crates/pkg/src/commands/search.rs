//! `pkg search`: find packages in Homebrew, mise's registry, and the App Store.

use pkg_core::matching::Page;
use pkg_core::{Found, SearchRequest, SearchSource};
use schemars::schema_for;

use super::common::{execute, table};
use super::list::CURSOR;
use crate::NAME;
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::registry::{
    ArgumentKind, ArgumentSpec, CommandSpec, Effect, Example, Requirement, ValueKind,
};
use crate::response::{RenderOutput, Response};

/// Where search looks, in the order ties are listed.
const SOURCES: &[&str] = &["brew", "brew-cask", "tool", "mas"];

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "search",
    path: &["search"],
    aliases: &[],
    summary: "Find packages in Homebrew, mise's registry, and the App Store",
    description: "Searches Homebrew's formulae (`brew:`) and casks (`brew-cask:`), the tools in \
        mise's registry, and Mac apps in the App Store (`mas:`) by name and description, and \
        returns the best matches with the exact `package` to pass to `pkg install`, its version, \
        and whether your config already declares it. Homebrew's catalog is downloaded once a day \
        into pkg's cache (about 7 MB); when Homebrew cannot be reached an older copy answers with \
        a warning. The App Store is asked on every search; if it cannot be reached the other \
        sources still answer, with a warning. Pass --manager to search fewer sources. Changes \
        nothing but the cache.",
    arguments: &[
        ArgumentSpec {
            name: "query",
            kind: ArgumentKind::Positional,
            value: ValueKind::String,
            required: true,
            default: None,
            multiple: true,
            summary: "What to look for, such as `ffmpeg` or `video player`; several words need no \
                      quotes",
        },
        ArgumentSpec {
            name: "manager",
            kind: ArgumentKind::Option,
            value: ValueKind::Choice(SOURCES),
            required: false,
            default: None,
            multiple: true,
            summary: "Search only this source: `brew`, `brew-cask`, `tool` (mise's registry), or \
                      `mas`; repeat for several",
        },
        ArgumentSpec {
            name: "limit",
            kind: ArgumentKind::Option,
            value: ValueKind::Integer { min: 1, max: Some(50) },
            required: false,
            default: Some("10"),
            multiple: false,
            summary: "The most matches to return",
        },
        CURSOR,
    ],
    effect: Effect::Reads,
    requires: &[Requirement::Mise, Requirement::Network],
    dry_run: false,
    idempotent: true,
    errors: &[
        ErrorCode::InvalidArgument,
        ErrorCode::PreconditionFailed,
        ErrorCode::Unavailable,
        ErrorCode::MiseFailed,
    ],
    examples: &[
        Example { summary: "Find ffmpeg everywhere", args: &["search", "ffmpeg"] },
        Example {
            summary: "Find a video player app among the casks",
            args: &["search", "video", "player", "--manager", "brew-cask"],
        },
    ],
    synonyms: &["find", "look", "lookup", "query", "available", "discover", "catalog", "package"],
    output_schema: || schema_for!(Page<Found>),
    run: search_packages,
}];

impl RenderOutput for Page<Found> {
    fn render_text(&self) -> String {
        if self.items.is_empty() {
            return "Nothing matches. Try other words, or fewer.".into();
        }
        let rows: Vec<Vec<String>> = self
            .items
            .iter()
            .map(|found| {
                let mut description = shorten(&found.description, 70);
                if found.name != found.package.rsplit(':').next().unwrap_or_default()
                    && found.manager != "tool"
                {
                    description = format!("{}: {description}", found.name);
                }
                if found.declared {
                    description.push_str(" [in your config]");
                }
                vec![found.package.clone(), found.version.clone().unwrap_or_default(), description]
            })
            .collect();
        let mut text = table(&rows);
        if self.has_more {
            text.push_str(&format!(
                "More matches follow; pass --cursor {} or raise --limit.\n",
                self.next_cursor.as_deref().unwrap_or_default()
            ));
        }
        text.push_str(&format!("Install one with `{NAME} install <package>`.\n"));
        text
    }
}

fn shorten(text: &str, limit: usize) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    if line.chars().count() <= limit {
        return line.to_string();
    }
    let cut: String = line.chars().take(limit - 1).collect();
    format!("{}…", cut.trim_end())
}

fn search_packages(invocation: &Invocation) -> Result<Response, Failure> {
    let managers = invocation
        .strings("manager")
        .iter()
        .map(|value| source(value))
        .collect::<Result<Vec<SearchSource>, Failure>>()?;
    let request = SearchRequest::new(
        invocation.strings("query").join(" "),
        managers,
        invocation.integer("limit").unwrap_or(10),
        invocation.string("cursor"),
    )?;
    execute(invocation, |client, progress| client.search(request, progress))
}

/// The catalog a `--manager` choice names.
fn source(value: &str) -> Result<SearchSource, Failure> {
    [SearchSource::Brew, SearchSource::Cask, SearchSource::Tool, SearchSource::AppStore]
        .into_iter()
        .find(|source| source.as_str() == value)
        .ok_or_else(|| Failure::invalid_argument(format!("`{value}` is not a search source")))
}
