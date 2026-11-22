//! `pkg shims`: small executables named for other package managers, which stop a global install
//! and name the `pkg` command that does it.

use std::path::{Path, PathBuf};

use pkg_core::ReceiptStatus;
use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::json;

use super::common::{for_people, table};
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::registry::{
    ArgumentKind, ArgumentSpec, CommandSpec, DRY_RUN, Effect, Example, ValueKind,
};
use crate::response::{RenderOutput, Response};
use crate::shims::catalog::{Action, System};
use crate::shims::decide::{Call, Refusal, Verdict, decide};
use crate::shims::directory::{self, Change, InstallError, Listed, Resolution, ShimFile};
use crate::shims::{self, passthrough, path};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "shims.install",
        path: &["shims", "install"],
        aliases: &[],
        summary: "Write shims that point other package managers' global installs at pkg",
        description: "Writes one small executable per package manager mise can drive into the shim \
            directory (`$XDG_DATA_HOME/pkg/shims`, `~/.local/share/pkg/shims` by default, \
            `%LOCALAPPDATA%\\pkg\\shims` on Windows), each marked `pkg-shim`: `npm`, `pip`, `cargo`, \
            `mise` and the other tool managers everywhere, and `brew`, `apt`, `winget` and the \
            rest on the systems mise runs them on. With the directory first on PATH, a global \
            install through one runs nothing and names the `pkg` command that does it; everything \
            else runs the real tool. Replaces only files that carry the marker; when another file \
            has a shim's name it writes nothing and fails with PRECONDITION_FAILED, naming the \
            files. Never edits a startup file: when the directory is not on PATH it reports the \
            line that puts it there. --dry-run shows what would be written.",
        arguments: &[DRY_RUN],
        effect: Effect::Modifies("the shim directory"),
        requires: &[],
        dry_run: true,
        idempotent: true,
        errors: &[ErrorCode::PreconditionFailed],
        examples: &[
            Example { summary: "Preview the shims", args: &["shims", "install", "--dry-run"] },
            Example { summary: "Write the shims", args: &["shims", "install"] },
        ],
        synonyms: &[
            "agents",
            "redirect",
            "wrapper",
            "brew",
            "apt",
            "npm",
            "intercept",
            "guard",
            "path",
        ],
        output_schema: || schema_for!(ShimsReceipt),
        run: install,
    },
    CommandSpec {
        id: "shims.remove",
        path: &["shims", "remove"],
        aliases: &["rm"],
        summary: "Delete the shims",
        description: "Deletes every file in the shim directory that carries the `pkg-shim` marker, \
            then the directory when nothing else is in it. Leaves other files alone. Does not \
            touch the line that puts the directory on PATH. --dry-run lists what would be deleted.",
        arguments: &[DRY_RUN],
        effect: Effect::Modifies("the shim directory"),
        requires: &[],
        dry_run: true,
        idempotent: true,
        errors: &[ErrorCode::PreconditionFailed],
        examples: &[
            Example {
                summary: "Preview deleting the shims",
                args: &["shims", "remove", "--dry-run"],
            },
            Example { summary: "Delete the shims", args: &["shims", "remove"] },
        ],
        synonyms: &["uninstall", "delete", "undo", "disable"],
        output_schema: || schema_for!(ShimsReceipt),
        run: remove,
    },
    CommandSpec {
        id: "shims.list",
        path: &["shims", "list"],
        aliases: &["ls"],
        summary: "Show the shims, whether the directory is on PATH, and what each name runs",
        description: "For each shim of this system: whether it is installed and what its name \
            resolves to on the current PATH: the shim, another executable that comes first (its \
            path), or nothing. Reports whether the shim directory is on PATH and, when it is \
            not, the line that puts it first, for the shell named by $SHELL (bash, zsh or fish) \
            or for PowerShell on Windows. Never changes anything.",
        arguments: &[],
        effect: Effect::Reads,
        requires: &[],
        dry_run: false,
        idempotent: true,
        errors: &[ErrorCode::PreconditionFailed],
        examples: &[Example { summary: "List the shims", args: &["shims", "list"] }],
        synonyms: &["status", "which", "path", "check", "show"],
        output_schema: || schema_for!(ShimsList),
        run: list,
    },
    CommandSpec {
        id: "shims.run",
        path: &["shims", "run"],
        aliases: &[],
        summary: "What every shim calls: refuse a global install or run the real tool",
        description: "Called by each shim as `pkg shims run <tool> -- <arguments>`. When the \
            arguments are a global install or removal that `pkg` does, fails with \
            INVALID_ARGUMENT (exit status 2), naming the `pkg` command to run instead. Otherwise \
            runs the first executable called <tool> on PATH that is not a shim, with the same \
            arguments, byte for byte, and its exit status, and writes nothing of its own. \
            Everything after `--` is the tool's, `--output` included. When there is none, \
            says so and exits 127. The `mise` shim runs the real mise without the shim directory \
            on PATH, except for `exec`, `x`, `run`, `r`, `watch` and `w`.",
        arguments: &[
            ArgumentSpec {
                name: "tool",
                kind: ArgumentKind::Positional,
                value: ValueKind::String,
                required: true,
                default: None,
                multiple: false,
                summary: "The shim's name: `npm`, `brew`, `apt`, ...",
            },
            ArgumentSpec {
                name: "arguments",
                kind: ArgumentKind::Positional,
                value: ValueKind::Raw,
                required: false,
                default: None,
                multiple: true,
                summary: "The tool's arguments, after `--`",
            },
        ],
        effect: Effect::Runs("the real package manager"),
        requires: &[],
        dry_run: false,
        idempotent: false,
        errors: &[ErrorCode::InvalidArgument],
        examples: &[Example {
            summary: "What a shim does with a global npm install",
            args: &["shims", "run", "npm", "--", "install", "-g", "prettier"],
        }],
        synonyms: &["launcher", "dispatch", "shim", "pass", "through"],
        output_schema: || schema_for!(NoResult),
        run: run_shim,
    },
];

/// `shims run` writes no result of its own: it refuses, or becomes the real tool.
#[derive(Serialize, JsonSchema)]
pub struct NoResult {}

/// What `shims install` and `shims remove` did, or would do with `--dry-run`.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShimsReceipt {
    pub status: ReceiptStatus,
    /// `installed` or `removed`.
    pub action: String,
    pub directory: String,
    pub shims: Vec<ShimChange>,
    /// Whether the shim directory is on the current PATH.
    pub on_path: bool,
    /// The line that puts the directory first on PATH, when it is not there. Never set by `remove`.
    pub path_line: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct ShimChange {
    pub name: String,
    pub file: String,
    /// `created`, `updated`, `unchanged` or `removed`.
    pub change: String,
}

/// The shims of this system and what each runs.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShimsList {
    pub directory: String,
    pub on_path: bool,
    /// The line that puts the directory first on PATH, when it is not there.
    pub path_line: Option<String>,
    pub shims: Vec<ListedShim>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListedShim {
    pub name: String,
    pub file: String,
    pub installed: bool,
    pub resolves_to: ResolvesTo,
}

#[derive(Serialize, JsonSchema)]
pub struct ResolvesTo {
    /// `shim`, `other` (another executable comes first) or `none`.
    pub kind: String,
    /// The other executable's path.
    pub path: Option<String>,
}

impl RenderOutput for ShimsReceipt {
    fn render_text(&self) -> String {
        let directory = for_people(&self.directory);
        let count = self.shims.len();
        let head = match (self.status, self.action.as_str()) {
            (_, "removed") if count == 0 => format!("No shims in {directory}."),
            (ReceiptStatus::Completed, "removed") => {
                format!("Removed {count} shims from {directory}.")
            }
            (ReceiptStatus::DryRun, "removed") => {
                format!("Would remove {count} shims from {directory}.")
            }
            (ReceiptStatus::Completed, _) => {
                format!("Wrote {count} shims to {directory} ({}).", self.counts())
            }
            (ReceiptStatus::DryRun, _) => {
                format!("Would write {count} shims to {directory} ({}).", self.counts())
            }
        };
        format!("{head}\n{}", path_advice(self.path_line.as_deref()))
    }
}

impl ShimsReceipt {
    fn counts(&self) -> String {
        ["created", "updated", "unchanged"]
            .iter()
            .filter_map(|change| {
                let count = self.shims.iter().filter(|shim| shim.change == *change).count();
                (count > 0).then(|| format!("{count} {change}"))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl RenderOutput for ShimsList {
    fn render_text(&self) -> String {
        let state = if self.on_path { "on PATH" } else { "not on PATH" };
        let rows: Vec<Vec<String>> = self
            .shims
            .iter()
            .map(|shim| {
                let runs = match shim.resolves_to.kind.as_str() {
                    "shim" => "the shim".to_string(),
                    "other" => shim.resolves_to.path.as_deref().map(for_people).unwrap_or_default(),
                    _ => "nothing".to_string(),
                };
                vec![
                    shim.name.clone(),
                    if shim.installed { "installed" } else { "not installed" }.to_string(),
                    runs,
                ]
            })
            .collect();
        format!(
            "Shim directory: {} ({state})\n{}{}",
            for_people(&self.directory),
            table(&rows),
            path_advice(self.path_line.as_deref())
        )
    }
}

fn path_advice(line: Option<&str>) -> String {
    line.map(|line| {
        format!(
            "The directory is not on PATH. Add this line to the startup file your shells read, \
             then start a new shell:\n  {line}\n"
        )
    })
    .unwrap_or_default()
}

fn shim_directory() -> Result<PathBuf, Failure> {
    shims::process_directory().ok_or_else(|| {
        Failure::precondition("no home directory is set, so pkg cannot find where shims go")
    })
}

fn change_name(change: Change) -> &'static str {
    match change {
        Change::Created => "created",
        Change::Updated => "updated",
        Change::Unchanged => "unchanged",
        Change::Removed => "removed",
    }
}

fn shim_changes(files: &[ShimFile]) -> Vec<ShimChange> {
    files
        .iter()
        .map(|shim| ShimChange {
            name: shim.name.clone(),
            file: shim.file.to_string_lossy().into_owned(),
            change: change_name(shim.change).to_string(),
        })
        .collect()
}

fn on_path(directory: &Path) -> bool {
    shims::process_variable("PATH")
        .is_some_and(|search_path| path::contains(&search_path, directory))
}

/// The line that puts `directory` on PATH, unless it is already there.
fn missing_path_line(directory: &Path) -> Option<String> {
    if on_path(directory) {
        return None;
    }
    let home = shims::process_variable(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from);
    let shell = shims::process_variable("SHELL");
    Some(path::path_line(System::current(), directory, home.as_deref(), shell.as_deref()))
}

fn receipt(
    invocation: &Invocation,
    action: &str,
    directory: &Path,
    files: &[ShimFile],
    path_line: Option<String>,
) -> ShimsReceipt {
    ShimsReceipt {
        status: if invocation.dry_run() { ReceiptStatus::DryRun } else { ReceiptStatus::Completed },
        action: action.to_string(),
        directory: directory.to_string_lossy().into_owned(),
        shims: shim_changes(files),
        on_path: on_path(directory),
        path_line,
    }
}

fn install(invocation: &Invocation) -> Result<Response, Failure> {
    let directory = shim_directory()?;
    let files = directory::install(System::current(), &directory, invocation.dry_run())
        .map_err(|error| install_failure(error, &directory))?;
    let receipt =
        receipt(invocation, "installed", &directory, &files, missing_path_line(&directory));
    Ok(Response::new(&receipt, invocation.output))
}

fn install_failure(error: InstallError, directory: &Path) -> Failure {
    match error {
        InstallError::Conflict(files) => Failure::precondition(
            "files that are not pkg shims have the names of shims, so nothing was written",
        )
        .with_details(json!({
            "directory": directory.to_string_lossy(),
            "files": files.iter().map(|file| file.to_string_lossy()).collect::<Vec<_>>(),
        }))
        .suggest("Move or delete those files, then run `pkg shims install` again"),
        InstallError::Io(error) => Failure::precondition(format!(
            "cannot write the shims to {}: {error}",
            directory.display()
        ))
        .with_details(json!({ "directory": directory.to_string_lossy() })),
    }
}

fn remove(invocation: &Invocation) -> Result<Response, Failure> {
    let directory = shim_directory()?;
    let files = directory::remove(&directory, invocation.dry_run()).map_err(|error| {
        Failure::precondition(format!(
            "cannot remove the shims from {}: {error}",
            directory.display()
        ))
        .with_details(json!({ "directory": directory.to_string_lossy() }))
    })?;
    Ok(Response::new(&receipt(invocation, "removed", &directory, &files, None), invocation.output))
}

fn list(invocation: &Invocation) -> Result<Response, Failure> {
    let directory = shim_directory()?;
    let search_path = shims::process_variable("PATH");
    let pathext = shims::process_variable("PATHEXT");
    let listed =
        directory::list(System::current(), &directory, search_path.as_deref(), pathext.as_deref());
    let list = ShimsList {
        directory: directory.to_string_lossy().into_owned(),
        on_path: on_path(&directory),
        path_line: missing_path_line(&directory),
        shims: listed.into_iter().map(listed_shim).collect(),
    };
    Ok(Response::new(&list, invocation.output))
}

fn listed_shim(shim: Listed) -> ListedShim {
    let (kind, other) = match shim.resolves_to {
        Resolution::Shim => ("shim", None),
        Resolution::Other(path) => ("other", Some(path.to_string_lossy().into_owned())),
        Resolution::Nothing => ("none", None),
    };
    ListedShim {
        name: shim.name,
        file: shim.file.to_string_lossy().into_owned(),
        installed: shim.installed,
        resolves_to: ResolvesTo { kind: kind.to_string(), path: other },
    }
}

fn run_shim(invocation: &Invocation) -> Result<Response, Failure> {
    let tool = invocation.string("tool").unwrap_or_default();
    let args = invocation.raw_strings("arguments");
    let words: Vec<String> = args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
    let in_virtual_environment =
        ["VIRTUAL_ENV", "CONDA_PREFIX"].iter().any(|key| shims::process_variable(key).is_some());
    if let Verdict::Refuse(refusal) =
        decide(&Call { tool: &tool, args: &words, in_virtual_environment })
    {
        return Err(refusal_failure(&refusal));
    }
    let Err(failure) = passthrough::run(
        &tool,
        &args,
        shims::process_directory().as_deref(),
        shims::process_variable("PATH"),
        shims::process_variable("PATHEXT"),
    );
    Err(failure)
}

fn refusal_failure(refusal: &Refusal) -> Failure {
    let (does, then) = match refusal.action {
        Action::Install => {
            ("installs", "which records it in your mise config so `mise bootstrap` restores it")
        }
        Action::Remove => ("removes", "which also takes it out of your mise config"),
    };
    let mut failure = Failure::invalid_argument(format!(
        "{} did not run: this machine {does} software with pkg, {then}",
        refusal.tool
    ))
    .with_details(json!({ "tool": refusal.tool, "command": refusal.command }))
    .suggest(refusal.command.clone());
    if let Some(alternative) = &refusal.alternative {
        failure =
            failure.suggest(format!("{alternative} (for packages in Arch's own repositories)"));
    }
    failure
        .suggest("Add --dry-run to preview it, and find a name with `pkg search <words>`")
        .suggest("`pkg --help` and `pkg cli search \"<task>\"` show what else pkg does")
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    #[test]
    fn a_refusal_names_the_command_the_alternative_and_where_to_learn_more() {
        let refusal = Refusal {
            tool: "yay".into(),
            action: Action::Install,
            command: "pkg install aur:firefox".into(),
            alternative: Some("pkg install pacman:firefox".into()),
        };
        let failure = refusal_failure(&refusal);
        assert_eq!(failure.code, ErrorCode::InvalidArgument);
        let text = failure.to_text();
        assert!(
            text.starts_with("error: yay did not run: this machine installs software with pkg"),
            "{text}"
        );
        assert!(text.contains("hint: pkg install aur:firefox\n"), "{text}");
        assert!(
            text.contains("hint: pkg install pacman:firefox (for packages in Arch's own"),
            "{text}"
        );
        assert!(text.contains("pkg search"), "{text}");
        assert_eq!(failure.details["command"], "pkg install aur:firefox");
    }

    #[test]
    fn a_removal_refusal_says_it_also_edits_the_config() {
        let refusal = Refusal {
            tool: "brew".into(),
            action: Action::Remove,
            command: "pkg remove brew:wget".into(),
            alternative: None,
        };
        let text = refusal_failure(&refusal).to_text();
        assert!(
            text.contains("removes software with pkg, which also takes it out of your mise config"),
            "{text}"
        );
    }

    #[test]
    fn the_text_reports_counts_and_the_line_to_add() {
        let receipt = ShimsReceipt {
            status: ReceiptStatus::Completed,
            action: "installed".into(),
            directory: "/x/shims".into(),
            shims: vec![
                ShimChange {
                    name: "npm".into(),
                    file: "/x/shims/npm".into(),
                    change: "created".into(),
                },
                ShimChange {
                    name: "uv".into(),
                    file: "/x/shims/uv".into(),
                    change: "unchanged".into(),
                },
            ],
            on_path: false,
            path_line: Some("export PATH=\"/x/shims:$PATH\"".into()),
        };
        let text = receipt.render_text();
        assert!(
            text.starts_with("Wrote 2 shims to /x/shims (1 created, 1 unchanged).\n"),
            "{text}"
        );
        assert!(text.contains("\n  export PATH=\"/x/shims:$PATH\"\n"), "{text}");
    }

    #[test]
    fn removing_nothing_says_so() {
        let receipt = ShimsReceipt {
            status: ReceiptStatus::Completed,
            action: "removed".into(),
            directory: "/x/shims".into(),
            shims: Vec::new(),
            on_path: false,
            path_line: None,
        };
        assert_eq!(receipt.render_text(), "No shims in /x/shims.\n");
    }

    #[test]
    fn the_json_matches_the_fields_in_order() {
        let receipt = ShimsReceipt {
            status: ReceiptStatus::DryRun,
            action: "installed".into(),
            directory: "/x/shims".into(),
            shims: Vec::new(),
            on_path: true,
            path_line: None,
        };
        let value: Value = receipt.render_json();
        assert_eq!(value["status"], "dry-run");
        assert_eq!(value["onPath"], true);
        assert!(value["pathLine"].is_null());
    }
}
