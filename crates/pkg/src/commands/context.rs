//! `pkg context show`: which mise, which config files, and which catalogs pkg uses.

use std::path::PathBuf;

use pkg_core::{ConfigContext, Outcome, PkgOwner, PkgTool, pkg_owner};
use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::{Value, json};

use super::common::{MISE_ERRORS, execute, for_people, table};
use crate::NAME;
use crate::environment::{APP_STORE_API_VARIABLE, HOMEBREW_API_VARIABLE, MISE_VARIABLE};
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::registry::{CommandSpec, Effect, Example};
use crate::response::{RenderOutput, Response};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "context.show",
    path: &["context", "show"],
    aliases: &[],
    summary: "Show the mise config files, mise, and catalogs pkg uses",
    description: "Reports where pkg takes effect: the mise executable (`PKG_MISE`, else `mise` on \
        PATH, else `~/.local/bin/mise`), its version, and whether `pkg update` updates it (not \
        when a package manager installed it), the home directory pkg runs mise in, the file new \
        `[bootstrap.packages]` entries go to (mise's `write_targets.packages` setting, else its \
        global config) and the file new `[tools]` entries go to (`write_targets.tools`), each \
        with symlinks resolved to the checkout it lands in, every global config file mise loads, \
        the Homebrew and App Store catalog URLs, and the cached Homebrew search index. Without \
        mise it says so and reports the global config. Never changes anything.",
    arguments: &[],
    effect: Effect::Reads,
    requires: &[],
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
    pub pkg: PkgContext,
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
pub struct PkgContext {
    /// The executable answering.
    pub executable: PathBuf,
    /// Who put it there: `installer` (rerun the installer to update it), `mise` (`pkg update`
    /// updates it), or `other`.
    pub owner: PkgOwner,
    /// pkg as a mise tool, when a config declares `github:gotpkg/pkg`.
    pub tool: Option<PkgTool>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MiseContext {
    /// The executable pkg runs.
    pub program: PathBuf,
    /// The environment variable that overrides it.
    pub environment_variable: String,
    /// Whether that executable exists and runs.
    pub found: bool,
    /// What `mise version` prints; null when mise is not found.
    pub version: Option<String>,
    /// Whether `pkg update` updates mise with `mise self-update`; false for a mise a package
    /// manager installed.
    pub self_updates: bool,
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

impl Context {
    /// Adds the CLI's version and executable and the environment variables that select mise and
    /// the catalogs.
    fn new(context: pkg_core::Context, executable: PathBuf, owner: PkgOwner) -> Context {
        let catalogs = context.catalogs;
        Context {
            version: env!("CARGO_PKG_VERSION").to_string(),
            pkg: PkgContext { executable, owner, tool: context.pkg_tool },
            mise: MiseContext {
                program: context.mise.program,
                environment_variable: MISE_VARIABLE.to_string(),
                found: context.mise.found,
                version: context.mise.version,
                self_updates: context.mise.self_updates,
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
            vec!["pkg".to_string(), self.pkg_row()],
            vec!["mise".to_string(), self.mise_row()],
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
            "pkg": {
                "executable": self.pkg.executable.to_string_lossy(),
                "owner": self.pkg.owner,
                "tool": self.pkg.tool.as_ref().map(|tool| json!({
                    "requested": tool.requested,
                    "installed": tool.installed,
                    "config": tool.config.as_ref().map(|config| config.to_string_lossy()),
                })),
            },
            "mise": {
                "program": self.mise.program.to_string_lossy(),
                "environmentVariable": self.mise.environment_variable,
                "found": self.mise.found,
                "version": self.mise.version,
                "selfUpdates": self.mise.self_updates,
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

impl Context {
    fn pkg_row(&self) -> String {
        let executable = for_people(&self.pkg.executable);
        let owner = match self.pkg.owner {
            PkgOwner::Installer => "the installer's copy",
            PkgOwner::Mise => "managed by mise",
            PkgOwner::Other => "installed outside the installer and mise",
        };
        format!("{} at {executable}, {owner}", self.version)
    }

    fn mise_row(&self) -> String {
        let program = self.mise.program.display();
        match (&self.mise.version, self.mise.self_updates) {
            (None, _) => format!("not found ({program}); `{NAME} setup` installs it"),
            (Some(version), true) => format!("{program} {version}"),
            (Some(version), false) => {
                format!("{program} {version} (its package manager updates it)")
            }
        }
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
    let settings = invocation.environment()?;
    let executable = std::env::current_exe()
        .map_err(|error| Failure::internal(format!("cannot tell where pkg runs from: {error}")))?;
    let owner = pkg_owner(&settings, &executable);
    execute(invocation, |client, progress| {
        let outcome = client.context(progress)?;
        let context = Context::new(outcome.result, executable, owner);
        Ok(Outcome { result: context, warnings: outcome.warnings })
    })
}
