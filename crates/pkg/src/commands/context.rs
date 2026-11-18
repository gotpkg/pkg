//! `pkg context show`: which mise, which config files, and which catalogs pkg uses.

use std::path::PathBuf;

use pkg_core::{ConfigContext, Outcome};
use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::{Value, json};

use super::common::{MISE_ERRORS, execute, for_people, table};
use crate::environment::{APP_STORE_API_VARIABLE, HOMEBREW_API_VARIABLE, MISE_VARIABLE};
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::registry::{CommandSpec, Effect, Example, Requirement};
use crate::response::{RenderOutput, Response};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "context.show",
    path: &["context", "show"],
    aliases: &[],
    summary: "Show the mise config files, mise, and catalogs pkg uses",
    description: "Reports where pkg takes effect: the mise executable and version, the home \
        directory pkg runs mise in, the file new `[bootstrap.packages]` entries go to (mise's \
        `write_targets.packages` setting, else its global config) and the file new `[tools]` \
        entries go to (`write_targets.tools`), each with symlinks resolved to the checkout it \
        lands in, every global config file mise loads, the Homebrew and App Store catalog URLs, \
        and the cached Homebrew search index. Never changes anything.",
    arguments: &[],
    effect: Effect::Reads,
    requires: &[Requirement::Mise],
    dry_run: false,
    idempotent: true,
    errors: MISE_ERRORS,
    examples: &[Example {
        summary: "Show where installs are recorded",
        args: &["context", "show"],
    }],
    synonyms: &[
        "where",
        "config",
        "file",
        "dotfiles",
        "settings",
        "environment",
        "target",
        "status",
    ],
    output_schema: || schema_for!(Context),
    run: show_context,
}];

/// Where pkg takes effect.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// The pkg version answering.
    pub version: String,
    pub mise: MiseContext,
    /// The home directory pkg runs mise in.
    pub home: PathBuf,
    /// Where `pkg install` declares bootstrap packages.
    pub packages_config: ConfigContext,
    /// Where `pkg install` declares tools.
    pub tools_config: ConfigContext,
    /// Every global config file mise loads, symlinks resolved.
    pub config_files: Vec<PathBuf>,
    pub catalogs: Catalogs,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MiseContext {
    /// The executable pkg runs.
    pub program: PathBuf,
    /// The environment variable that overrides it.
    pub environment_variable: String,
    /// What `mise version` prints.
    pub version: String,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Catalogs {
    pub homebrew_api: String,
    pub homebrew_api_variable: String,
    pub app_store_api: String,
    pub app_store_api_variable: String,
    /// The cached Homebrew search index.
    pub homebrew_index: PathBuf,
    /// When the cached index was downloaded, in seconds since the Unix epoch; null when there is
    /// none yet.
    pub homebrew_index_fetched_at: Option<u64>,
}

impl From<pkg_core::Context> for Context {
    /// Adds the CLI's version and the environment variables that select mise and the catalogs.
    fn from(context: pkg_core::Context) -> Context {
        let catalogs = context.catalogs;
        Context {
            version: env!("CARGO_PKG_VERSION").to_string(),
            mise: MiseContext {
                program: context.mise.program,
                environment_variable: MISE_VARIABLE.to_string(),
                version: context.mise.version,
            },
            home: context.home,
            packages_config: context.packages_config,
            tools_config: context.tools_config,
            config_files: context.config_files,
            catalogs: Catalogs {
                homebrew_api: catalogs.homebrew_api,
                homebrew_api_variable: HOMEBREW_API_VARIABLE.to_string(),
                app_store_api: catalogs.app_store_api,
                app_store_api_variable: APP_STORE_API_VARIABLE.to_string(),
                homebrew_index: catalogs.homebrew_index,
                homebrew_index_fetched_at: catalogs.homebrew_index_fetched_at,
            },
        }
    }
}

impl RenderOutput for Context {
    fn render_text(&self) -> String {
        let config = |context: &ConfigContext| {
            if context.path.as_os_str() == context.real_path.as_os_str() {
                for_people(&context.path)
            } else {
                format!("{} -> {}", for_people(&context.path), for_people(&context.real_path))
            }
        };
        let files: Vec<String> = self.config_files.iter().map(for_people).collect();
        let rows = vec![
            vec!["pkg".to_string(), self.version.clone()],
            vec![
                "mise".to_string(),
                format!("{} {}", self.mise.program.display(), self.mise.version),
            ],
            vec!["packages go to".to_string(), config(&self.packages_config)],
            vec!["tools go to".to_string(), config(&self.tools_config)],
            vec!["config files".to_string(), files.join(", ")],
            vec!["Homebrew".to_string(), self.catalogs.homebrew_api.clone()],
            vec!["App Store".to_string(), self.catalogs.app_store_api.clone()],
            vec![
                "search index".to_string(),
                match self.catalogs.homebrew_index_fetched_at {
                    Some(_) => for_people(&self.catalogs.homebrew_index),
                    None => "not downloaded yet".to_string(),
                },
            ],
        ];
        table(&rows).lines().map(|line| format!("{}\n", line.trim_start())).collect()
    }

    fn render_json(&self) -> Value {
        let config_files: Vec<_> =
            self.config_files.iter().map(|file| file.to_string_lossy()).collect();
        json!({
            "version": self.version,
            "mise": {
                "program": self.mise.program.to_string_lossy(),
                "environmentVariable": self.mise.environment_variable,
                "version": self.mise.version,
            },
            "home": self.home.to_string_lossy(),
            "packagesConfig": config_json(&self.packages_config),
            "toolsConfig": config_json(&self.tools_config),
            "configFiles": config_files,
            "catalogs": {
                "homebrewApi": self.catalogs.homebrew_api,
                "homebrewApiVariable": self.catalogs.homebrew_api_variable,
                "appStoreApi": self.catalogs.app_store_api,
                "appStoreApiVariable": self.catalogs.app_store_api_variable,
                "homebrewIndex": self.catalogs.homebrew_index.to_string_lossy(),
                "homebrewIndexFetchedAt": self.catalogs.homebrew_index_fetched_at,
            },
        })
    }
}

fn config_json(config: &ConfigContext) -> Value {
    json!({
        "setting": config.setting,
        "path": config.path.to_string_lossy(),
        "realPath": config.real_path.to_string_lossy(),
    })
}

fn show_context(invocation: &Invocation) -> Result<Response, Failure> {
    execute(invocation, |client, progress| {
        let outcome = client.context(progress)?;
        Ok(Outcome { result: Context::from(outcome.result), warnings: outcome.warnings })
    })
}
