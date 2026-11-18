//! Putting mise's shims on `PATH` for a person with no configuration that would.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;

use super::mise::cannot_write;
use crate::startup::{self, Shell};
use crate::{Error, Mode, Settings};

/// What `setup` did about mise's shims being on `PATH`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct PathChange {
    /// `added` when pkg wrote its block, `unchanged` when the block already had the shims, and
    /// `printed` when the line is for you to add (another shell, a linked startup file, or
    /// `--no-modify-path`).
    pub action: PathAction,
    /// mise's shims directory.
    pub directory: PathBuf,
    /// The startup files pkg's block is in.
    pub files: Vec<PathBuf>,
    /// The line that puts `~/.local/bin` and the shims first on `PATH`.
    pub line: String,
    /// The command that loads the change in the current terminal, when a file changed.
    pub load: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PathAction {
    Added,
    Unchanged,
    Printed,
}

/// Whether a global mise configuration exists: then it, not pkg, decides what is on `PATH`.
pub(crate) fn global_config_exists(settings: &Settings) -> bool {
    if settings.global_config_override.as_ref().is_some_and(|path| path.exists()) {
        return true;
    }
    let is_config = |path: &Path| {
        let name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
        name.starts_with("config") && name.ends_with(".toml")
    };
    let has_toml = |directory: &Path, matches: &dyn Fn(&Path) -> bool| {
        std::fs::read_dir(directory)
            .map(|entries| entries.flatten().any(|entry| matches(&entry.path())))
            .unwrap_or(false)
    };
    has_toml(&settings.config_dir, &is_config)
        || has_toml(&settings.config_dir.join("conf.d"), &|path| {
            path.extension().is_some_and(|extension| extension == "toml")
        })
}

/// Adds mise's shims to pkg's block when no configuration exists and they are not on `PATH`.
pub(crate) fn put_on_path(
    settings: &Settings,
    has_config: bool,
    modify_path: bool,
    mode: Mode,
) -> Result<Option<PathChange>, Error> {
    let directory = settings.mise_data_dir().join("shims");
    if has_config || on_path(settings, &directory) {
        return Ok(None);
    }
    let directories = [settings.local_bin(), directory.clone()];
    let home = &settings.home;
    let printed = |shell: Shell| PathChange {
        action: PathAction::Printed,
        directory: directory.clone(),
        files: Vec::new(),
        line: shell.path_line(&directories, home),
        load: None,
    };
    let Some(shell) = Shell::login(&settings.child_env) else {
        return Ok(Some(printed(Shell::Zsh)));
    };
    let files = shell.files(home, &settings.child_env);
    let linked =
        files.iter().any(|file| file.symlink_metadata().is_ok_and(|meta| meta.is_symlink()));
    if !modify_path || linked {
        return Ok(Some(printed(shell)));
    }
    let block = shell.block(&directories, home);
    let mut changed = false;
    for file in &files {
        let contents = std::fs::read_to_string(file).unwrap_or_default();
        let updated = startup::with_block(&contents, &block);
        if updated == contents {
            continue;
        }
        changed = true;
        if !mode.is_dry_run() {
            write(file, &updated)?;
        }
    }
    Ok(Some(PathChange {
        action: if changed { PathAction::Added } else { PathAction::Unchanged },
        directory,
        load: changed.then(|| shell.load_command(&files[0], home)),
        files,
        line: shell.path_line(&directories, home),
    }))
}

fn on_path(settings: &Settings, directory: &Path) -> bool {
    settings
        .child_env
        .get(OsStr::new("PATH"))
        .is_some_and(|path| std::env::split_paths(path).any(|entry| entry == directory))
}

pub(crate) fn write(file: &Path, contents: &str) -> Result<(), Error> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|error| cannot_write(parent, &error))?;
    }
    std::fs::write(file, contents).map_err(|error| cannot_write(file, &error))
}
