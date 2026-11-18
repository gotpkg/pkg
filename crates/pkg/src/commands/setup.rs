//! `pkg setup`: install mise, put its tools on PATH, and hand pkg to mise.

use pkg_core::{
    Declaration, Handoff, Link, MiseAction, MiseSetup, PathAction, PathChange, ReceiptStatus,
    SetupReceipt, SetupRequest,
};
use schemars::schema_for;

use super::common::{execute, for_people};
use crate::failure::{ErrorCode, Failure};
use crate::invocation::Invocation;
use crate::registry::{
    ArgumentKind, ArgumentSpec, CommandSpec, DRY_RUN, Effect, Example, Requirement, ValueKind,
};
use crate::response::{RenderOutput, Response};

/// `--no-modify-path`, which `setup` and `bootstrap` share with the installer.
pub const NO_MODIFY_PATH: ArgumentSpec = ArgumentSpec {
    name: "no-modify-path",
    kind: ArgumentKind::Flag,
    value: ValueKind::Boolean,
    required: false,
    default: None,
    multiple: false,
    summary: "Leave shell startup files alone and print the PATH line to add instead",
};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "setup",
    path: &["setup"],
    aliases: &[],
    summary: "Install mise and get pkg ready on a machine with nothing to restore",
    description: "Gets a new Mac or Linux machine ready for `pkg install` when there is no \
        configuration to restore (use `pkg bootstrap --from` for that). When pkg finds no mise \
        (`PKG_MISE`, `mise` on PATH, `~/.local/bin/mise`), it runs mise's own installer \
        (https://mise.run, or `PKG_MISE_INSTALLER`) for `~/.local/bin/mise`, and it refuses a \
        mise older than 2026.11.9. When no global mise config exists, so nothing else will put \
        mise's tools on PATH, it adds mise's shims directory to pkg's block in the startup file \
        of your login shell (`~/.zshenv`, `~/.bashrc` and `~/.profile`, or fish's \
        `conf.d/pkg.fish`) and prints the command that loads it now; --no-modify-path prints \
        the line instead. Then it hands pkg to mise: it declares and installs \
        `github:gotpkg/pkg` as `pkg install` would, unless a config declares it, and replaces \
        the installer's `~/.local/bin/pkg` with a link to mise's shim, never with an older \
        release. Its declaration creates a global config on a machine without one, which \
        `pkg bootstrap --from` then refuses to link over. Running it again changes nothing \
        already in place. Fails on Windows.",
    arguments: &[NO_MODIFY_PATH, DRY_RUN],
    effect: Effect::Modifies("mise, shell startup files, and pkg's own installation"),
    requires: &[Requirement::Network],
    dry_run: true,
    idempotent: true,
    errors: &[ErrorCode::PreconditionFailed, ErrorCode::Unavailable, ErrorCode::MiseFailed],
    examples: &[
        Example { summary: "See what setup would do", args: &["setup", "--dry-run"] },
        Example { summary: "Install mise and hand pkg to it", args: &["setup"] },
    ],
    synonyms: &["first run", "new machine", "install mise", "init", "onboard", "path", "shims"],
    output_schema: || schema_for!(SetupReceipt),
    run: setup,
}];

impl RenderOutput for SetupReceipt {
    fn render_text(&self) -> String {
        let dry = self.status == ReceiptStatus::DryRun;
        let mut text = mise_line(&self.mise, dry);
        if let Some(path) = &self.path {
            text.push_str(&path_lines(path, dry));
        }
        text.push_str(&handoff_lines(&self.handoff, dry));
        if dry {
            text.push_str("Dry run: nothing was installed or written.\n");
        }
        text
    }
}

pub fn mise_line(mise: &MiseSetup, dry: bool) -> String {
    let program = for_people(&mise.program);
    let version = mise.version.as_deref().map(|version| format!(" {version}")).unwrap_or_default();
    match (mise.action, dry) {
        (MiseAction::Found, _) => format!("mise: using {program}{version}\n"),
        (MiseAction::Installed, true) => format!("mise: would install {program} with mise.run\n"),
        (MiseAction::Installed, false) => format!("mise: installed {program}{version}\n"),
    }
}

fn path_lines(path: &PathChange, dry: bool) -> String {
    let files: Vec<String> = path.files.iter().map(for_people).collect();
    let shims = for_people(&path.directory);
    match path.action {
        PathAction::Printed => {
            format!("PATH: add this line to your shell's startup file:\n  {}\n", path.line)
        }
        PathAction::Unchanged => format!("PATH: {shims} is already in {}\n", files.join(" and ")),
        PathAction::Added => {
            let verb = if dry { "would add" } else { "added" };
            let mut text = format!("PATH: {verb} {shims} to {}\n", files.join(" and "));
            if let (Some(load), false) = (&path.load, dry) {
                text.push_str(&format!("  Load it in this terminal: {load}\n"));
            }
            text
        }
    }
}

pub fn handoff_lines(handoff: &Handoff, dry: bool) -> String {
    let declared = match (handoff.declaration, dry) {
        (Declaration::Existing, _) => "already declared".to_string(),
        (Declaration::Added, true) => "would declare github:gotpkg/pkg".to_string(),
        (Declaration::Added, false) => "declared github:gotpkg/pkg".to_string(),
    };
    let config = handoff.config.as_ref().map(|config| format!(" in {}", for_people(config)));
    let then =
        if dry { "mise would update pkg from then on" } else { "mise updates pkg from now on" };
    let mut text = format!("pkg: {declared}{}; {then}\n", config.unwrap_or_default());
    let path = for_people(&handoff.path);
    match handoff.link {
        Link::Linked if dry => text.push_str(&format!("  would link {path} to mise's shim\n")),
        Link::Linked => text.push_str(&format!("  {path} now links to mise's shim\n")),
        Link::AlreadyLinked | Link::None => {}
        Link::Kept => text.push_str(&format!(
            "  kept {path}: {}\n",
            handoff.reason.as_deref().unwrap_or("it is not the installer's copy")
        )),
    }
    for other in &handoff.others {
        text.push_str(&format!(
            "  also on PATH: {}; a shell may run it instead\n",
            for_people(other)
        ));
    }
    text
}

fn setup(invocation: &Invocation) -> Result<Response, Failure> {
    let request = SetupRequest {
        mode: invocation.mode(),
        modify_path: !invocation.flag("no-modify-path"),
        pkg_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    execute(invocation, |client, progress| client.setup(request, progress))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn a_new_machine_hears_where_mise_went_and_how_to_load_path() {
        let receipt = SetupReceipt {
            status: ReceiptStatus::Completed,
            mise: MiseSetup {
                program: PathBuf::from("/m/.local/bin/mise"),
                version: Some("2026.11.9".into()),
                action: MiseAction::Installed,
            },
            path: Some(PathChange {
                action: PathAction::Added,
                directory: PathBuf::from("/m/.local/share/mise/shims"),
                files: vec![PathBuf::from("/m/.zshenv")],
                line: String::new(),
                load: Some("source ~/.zshenv".into()),
            }),
            handoff: Handoff {
                declaration: Declaration::Added,
                config: Some(PathBuf::from("/m/.config/mise/config.toml")),
                version: Some("0.3.0".into()),
                link: Link::Linked,
                path: PathBuf::from("/m/.local/bin/pkg"),
                reason: None,
                others: Vec::new(),
            },
        };
        let text = receipt.render_text();
        assert!(text.contains("installed /m/.local/bin/mise 2026.11.9"), "{text}");
        assert!(text.contains("source ~/.zshenv"), "{text}");
        assert!(text.contains("links to mise's shim"), "{text}");
    }
}
