//! pkg's block of `PATH` lines in shell startup files: the installer writes it, `setup` adds
//! mise's shims to it, and `bootstrap` moves it aside while a configuration deploys those files.
//! The installer (`installer/install.sh`) writes the same lines byte for byte.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

pub(crate) const BEGIN: &str = "# >>> pkg >>>";
pub(crate) const END: &str = "# <<< pkg <<<";

/// A shell whose startup files pkg knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shell {
    Zsh,
    Bash,
    Fish,
}

impl Shell {
    /// The login shell `$SHELL` names, when pkg knows its startup files.
    pub(crate) fn login(environment: &BTreeMap<OsString, OsString>) -> Option<Shell> {
        let shell = Path::new(environment.get(OsStr::new("SHELL"))?);
        match shell.file_name()?.to_str()? {
            "zsh" => Some(Shell::Zsh),
            "bash" => Some(Shell::Bash),
            "fish" => Some(Shell::Fish),
            _ => None,
        }
    }

    /// The files every new shell reads, which pkg's block goes at the end of.
    pub(crate) fn files(
        self,
        home: &Path,
        environment: &BTreeMap<OsString, OsString>,
    ) -> Vec<PathBuf> {
        let variable = |key: &str| {
            environment.get(OsStr::new(key)).map(PathBuf::from).filter(|path| path.is_absolute())
        };
        match self {
            Shell::Zsh => {
                vec![variable("ZDOTDIR").unwrap_or_else(|| home.to_path_buf()).join(".zshenv")]
            }
            Shell::Bash => {
                let login = home.join(".bash_profile");
                let login = if login.exists() { login } else { home.join(".profile") };
                vec![home.join(".bashrc"), login]
            }
            Shell::Fish => vec![
                variable("XDG_CONFIG_HOME")
                    .unwrap_or_else(|| home.join(".config"))
                    .join("fish/conf.d/pkg.fish"),
            ],
        }
    }

    /// The command that loads `file` into the shell that is already running.
    pub(crate) fn load_command(self, file: &Path, home: &Path) -> String {
        format!("source {}", under_home(file, home, "~"))
    }

    /// The line that puts `directories` first on `PATH`, in that order.
    pub(crate) fn path_line(self, directories: &[PathBuf], home: &Path) -> String {
        let words = directories.iter().map(|directory| under_home(directory, home, "$HOME"));
        match self {
            Shell::Zsh | Shell::Bash => {
                format!("export PATH=\"{}:$PATH\"", words.collect::<Vec<_>>().join(":"))
            }
            Shell::Fish => {
                let quoted: Vec<String> = words.map(|word| format!("\"{word}\"")).collect();
                format!("set -gx PATH {} $PATH", quoted.join(" "))
            }
        }
    }

    pub(crate) fn block(self, directories: &[PathBuf], home: &Path) -> String {
        format!("{BEGIN}\n{}\n{END}\n", self.path_line(directories, home))
    }
}

/// Every file the installer or `setup` may have put pkg's block in, whatever the login shell.
pub(crate) fn candidate_files(
    home: &Path,
    environment: &BTreeMap<OsString, OsString>,
) -> Vec<PathBuf> {
    let mut files = Shell::Zsh.files(home, environment);
    files.extend([home.join(".bashrc"), home.join(".bash_profile"), home.join(".profile")]);
    files.extend(Shell::Fish.files(home, environment));
    files
}

/// `contents` with `block` in place of pkg's block, or appended after a final newline.
pub(crate) fn with_block(contents: &str, block: &str) -> String {
    if let Some((before, after)) = split_block(contents) {
        return format!("{before}{block}{after}");
    }
    let mut updated = contents.to_string();
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(block);
    updated
}

/// Whether `contents` is pkg's block and nothing else but whitespace: a file the installer made.
pub(crate) fn is_only_block(contents: &str) -> bool {
    split_block(contents)
        .is_some_and(|(before, after)| before.trim().is_empty() && after.trim().is_empty())
}

/// The text before pkg's block and after it, including the end marker's newline.
fn split_block(contents: &str) -> Option<(&str, &str)> {
    let start = contents.lines().scan(0, |offset, line| {
        let at = *offset;
        *offset += line.len() + 1;
        Some((at, line))
    });
    let mut begin = None;
    for (at, line) in start {
        match (begin, line.trim_end()) {
            (None, BEGIN) => begin = Some(at),
            (Some(from), END) => {
                let end = (at + line.len() + 1).min(contents.len());
                return Some((&contents[..from], &contents[end..]));
            }
            _ => {}
        }
    }
    None
}

/// `path` under the home directory written from `home_word` (`$HOME` in a startup file, `~` in a
/// command a person types), else as is.
fn under_home(path: &Path, home: &Path, home_word: &str) -> String {
    match path.strip_prefix(home) {
        Ok(rest) => format!("{home_word}/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("/home/mark")
    }

    #[test]
    fn the_blocks_match_what_the_installer_writes() {
        let local_bin = [home().join(".local/bin")];
        assert_eq!(
            Shell::Zsh.block(&local_bin, &home()),
            "# >>> pkg >>>\nexport PATH=\"$HOME/.local/bin:$PATH\"\n# <<< pkg <<<\n"
        );
        assert_eq!(
            Shell::Fish.block(&local_bin, &home()),
            "# >>> pkg >>>\nset -gx PATH \"$HOME/.local/bin\" $PATH\n# <<< pkg <<<\n"
        );
        let both = [home().join(".local/bin"), PathBuf::from("/data/mise/shims")];
        assert_eq!(
            Shell::Bash.path_line(&both, &home()),
            "export PATH=\"$HOME/.local/bin:/data/mise/shims:$PATH\""
        );
    }

    #[test]
    fn a_block_is_replaced_in_place_or_appended_after_a_newline() {
        let old = Shell::Zsh.block(&[home().join(".local/bin")], &home());
        let new = Shell::Zsh.block(&[home().join(".local/bin"), home().join("shims")], &home());
        let file = format!("export EDITOR=vi\n{old}alias ll='ls -l'\n");
        assert_eq!(with_block(&file, &new), format!("export EDITOR=vi\n{new}alias ll='ls -l'\n"));
        assert_eq!(with_block("export EDITOR=vi", &new), format!("export EDITOR=vi\n{new}"));
        assert_eq!(with_block("", &new), new);
    }

    #[test]
    fn only_a_file_holding_just_the_block_counts_as_the_installers() {
        let block = Shell::Zsh.block(&[home().join(".local/bin")], &home());
        assert!(is_only_block(&block));
        assert!(is_only_block(&format!("\n{block}\n")));
        assert!(!is_only_block(&format!("export EDITOR=vi\n{block}")));
        assert!(!is_only_block("export EDITOR=vi\n"));
        assert!(!is_only_block(&format!("{BEGIN}\nexport PATH=x\n")), "an unterminated block");
    }

    #[test]
    fn zsh_honors_zdotdir_and_bash_prefers_an_existing_bash_profile() {
        let mut environment = BTreeMap::new();
        environment.insert(OsString::from("ZDOTDIR"), OsString::from("/home/mark/.config/zsh"));
        assert_eq!(
            Shell::Zsh.files(&home(), &environment),
            [PathBuf::from("/home/mark/.config/zsh/.zshenv")]
        );
        assert_eq!(
            Shell::Bash.files(Path::new("/nonexistent-home"), &BTreeMap::new()),
            [
                PathBuf::from("/nonexistent-home/.bashrc"),
                PathBuf::from("/nonexistent-home/.profile")
            ]
        );
        assert_eq!(Shell::Zsh.load_command(&home().join(".zshenv"), &home()), "source ~/.zshenv");
    }
}
