//! What the package commands share: their arguments, receipts, and a view of the machine.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::json;

use crate::NAME;
use crate::declarations::{Declarations, same_file};
use crate::environment::{Environment, real_path};
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::mise::ConfigTarget;
use crate::package::Package;
use crate::registry::{ArgumentKind, ArgumentSpec, ValueKind};

/// The packages a command acts on.
pub const PACKAGES: ArgumentSpec = ArgumentSpec {
    name: "packages",
    kind: ArgumentKind::Positional,
    value: ValueKind::Package,
    required: true,
    default: None,
    multiple: true,
    summary: "One or more packages: `brew:wget`, `brew-cask:firefox`, `mas:497799835`, \
              `npm:prettier`, or a tool from mise's registry such as `ripgrep`",
};

/// The one package a command describes.
pub const PACKAGE: ArgumentSpec = ArgumentSpec {
    name: "package",
    kind: ArgumentKind::Positional,
    value: ValueKind::Package,
    required: true,
    default: None,
    multiple: false,
    summary: "The package: `brew:wget`, `brew-cask:firefox`, `mas:497799835`, `npm:prettier`, \
              or a tool from mise's registry such as `ripgrep`",
};

/// The errors every command that runs mise can report.
pub const MISE_ERRORS: &[ErrorCode] =
    &[ErrorCode::InvalidArgument, ErrorCode::PreconditionFailed, ErrorCode::MiseFailed];

/// Whether a modifying command made its change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ReceiptStatus {
    /// The change is made.
    Completed,
    /// Nothing changed; the receipt shows what running without --dry-run would do.
    DryRun,
}

/// The machine as pkg sees it: where things are, mise's tool backends, and every declaration in
/// the global config.
pub struct Machine {
    pub environment: Environment,
    pub backends: Vec<String>,
    pub config_files: Vec<PathBuf>,
    pub declarations: Declarations,
}

impl Machine {
    /// # Errors
    ///
    /// `PRECONDITION_FAILED` without a home directory, mise, or readable config files, and
    /// `MISE_FAILED` when mise cannot answer.
    pub fn load(invocation: &Invocation) -> Result<Machine, Failure> {
        let environment = invocation.environment()?;
        let backends = environment.mise.tool_backends()?;
        let config_files = environment.mise.config_files()?;
        let declarations = Declarations::load(&config_files)?;
        Ok(Machine { environment, backends, config_files, declarations })
    }

    pub fn package(&self, value: &str) -> Package {
        Package::parse(value, &self.backends)
    }

    /// The command's packages, each once, in the order given.
    pub fn packages(&self, invocation: &Invocation, argument: &str) -> Vec<Package> {
        let mut packages: Vec<Package> = Vec::new();
        for value in invocation.strings(argument) {
            let package = self.package(&value);
            if !packages.iter().any(|seen| seen.key == package.key) {
                packages.push(package);
            }
        }
        packages
    }

    /// The config file that declares `package`, if any does.
    pub fn declared_in(&self, package: &Package) -> Option<&Path> {
        if package.is_tool() {
            self.declarations.tool_file(&package.key)
        } else {
            self.declarations.package_file(&package.key)
        }
    }

    /// The file mise writes `package`'s kind of declaration to.
    ///
    /// # Errors
    ///
    /// `MISE_FAILED` when mise cannot read its settings.
    pub fn target(&self, tool: bool) -> Result<ConfigTarget, Failure> {
        let setting = if tool { "tools" } else { "packages" };
        self.environment.mise.write_target(setting, &self.environment.global_config)
    }

    /// The file pkg edits to remove `package`: the one that declares it, when that is mise's
    /// write target.
    ///
    /// # Errors
    ///
    /// `NOT_FOUND` when no config declares it, and `PRECONDITION_FAILED` when another file than
    /// the write target does.
    pub fn editable_declaration(&self, package: &Package) -> Result<ConfigTarget, Failure> {
        let target = self.target(package.is_tool())?;
        let Some(declared) = self.declared_in(package) else {
            return Err(Failure::not_found(format!("{package} is not in your mise config"))
                .with_details(json!({
                    "package": package.key,
                    "configs": self.config_files.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
                }))
                .suggest(format!("{NAME} list")));
        };
        if !same_file(declared, &target.path) {
            let declared_display = self.environment.tilde(&real_path(declared));
            let target_display = self.environment.tilde(&target.real_path);
            return Err(Failure::precondition(format!(
                "{package} is declared in {declared_display}, and pkg only edits {target_display}"
            ))
            .with_details(json!({
                "package": package.key,
                "config": real_path(declared).display().to_string(),
                "editableConfig": target.real_path.display().to_string(),
            }))
            .suggest(format!(
                "Remove `{}` from {declared_display} by hand; it may be shared with other machines",
                package.key
            )));
        }
        Ok(target)
    }

    /// `path` for people, symlinks resolved: `~/.dotfiles/mise/config.macos.toml`.
    pub fn show(&self, path: &Path) -> String {
        self.environment.tilde(&real_path(path))
    }
}

/// An absolute path with symlinks resolved, for JSON results.
pub fn absolute(path: &Path) -> String {
    real_path(path).display().to_string()
}

/// An absolute path from a result, for text: under the home directory, written as mise resolved
/// it or with its symlinks resolved, it starts with `~`.
pub fn for_people(path: &str) -> String {
    let home =
        std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap_or_default();
    if home.is_empty() {
        return path.to_string();
    }
    let resolved = real_path(Path::new(&home)).display().to_string();
    for home in [home.as_str(), resolved.as_str()] {
        if let Some(rest) = path.strip_prefix(home)
            && (rest.is_empty() || rest.starts_with(['/', '\\']))
        {
            return format!("~{rest}");
        }
    }
    path.to_string()
}

/// `values` as `a`, `a and b`, or `a, b, and c`.
pub fn join_words(values: &[String]) -> String {
    match values {
        [] => String::new(),
        [one] => one.clone(),
        [one, two] => format!("{one} and {two}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// Rows of cells, each column padded to its widest cell, indented by two spaces.
pub fn table(rows: &[Vec<String>]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..columns)
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.get(column))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut text = String::new();
    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(column, cell)| {
                if column + 1 == row.len() {
                    cell.clone()
                } else {
                    format!("{cell:width$}", width = widths[column])
                }
            })
            .collect();
        text.push_str(&format!("  {}\n", cells.join("  ").trim_end()));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_words_as_a_sentence_would() {
        let words =
            |values: &[&str]| values.iter().map(|value| (*value).to_string()).collect::<Vec<_>>();
        assert_eq!(join_words(&words(&["a"])), "a");
        assert_eq!(join_words(&words(&["a", "b"])), "a and b");
        assert_eq!(join_words(&words(&["a", "b", "c"])), "a, b, and c");
    }

    #[test]
    fn tables_pad_every_column_but_the_last() {
        let rows = vec![
            vec!["wget".to_string(), "1.25.0".to_string(), String::new()],
            vec!["coreutils".to_string(), "9.12".to_string(), "GNU".to_string()],
        ];
        assert_eq!(table(&rows), "  wget       1.25.0\n  coreutils  9.12    GNU\n");
    }
}
