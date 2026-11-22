//! `PATH`: looking executables up on it, taking a directory off it, and the line that puts a
//! directory first on it.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use super::catalog::System;

/// The directories of a `PATH` value, without empty entries.
pub(crate) fn directories(path: &OsStr) -> Vec<PathBuf> {
    std::env::split_paths(path).filter(|directory| !directory.as_os_str().is_empty()).collect()
}

/// Whether two paths are one directory, whichever way each is spelled or linked.
pub(crate) fn same_directory(first: &Path, second: &Path) -> bool {
    first == second
        || matches!(
            (std::fs::canonicalize(first), std::fs::canonicalize(second)),
            (Ok(first), Ok(second)) if first == second
        )
}

pub(crate) fn contains(path: &OsStr, directory: &Path) -> bool {
    directories(path).iter().any(|entry| same_directory(entry, directory))
}

/// `path` without `directory`.
pub(crate) fn without(path: &OsStr, directory: &Path) -> OsString {
    let kept = directories(path).into_iter().filter(|entry| !same_directory(entry, directory));
    std::env::join_paths(kept).unwrap_or_else(|_| path.to_os_string())
}

/// The suffixes that make a file executable by name: `PATHEXT` on Windows, none elsewhere.
pub(crate) fn executable_extensions(pathext: Option<&OsStr>) -> Vec<String> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let pathext = pathext.map_or_else(
        || ".COM;.EXE;.BAT;.CMD".to_string(),
        |value| value.to_string_lossy().into_owned(),
    );
    pathext.split(';').filter(|extension| !extension.is_empty()).map(str::to_string).collect()
}

/// The first executable called `name` in `directories` that `skip` does not reject.
pub(crate) fn find_executable(
    directories: &[PathBuf],
    name: &str,
    extensions: &[String],
    skip: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    directories
        .iter()
        .flat_map(|directory| {
            if extensions.is_empty() {
                vec![directory.join(name)]
            } else {
                extensions
                    .iter()
                    .map(|extension| directory.join(format!("{name}{extension}")))
                    .collect()
            }
        })
        .find(|candidate| is_executable(candidate) && !skip(candidate))
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else { return false };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

/// The line that puts `directory` first on `PATH` for the shell named by `$SHELL`, or for
/// PowerShell on Windows. A directory under `home` is written with `$HOME`, so the line works on
/// every machine that shares the dotfiles it goes into.
pub(crate) fn path_line(
    system: System,
    directory: &Path,
    home: Option<&Path>,
    shell: Option<&OsStr>,
) -> String {
    if system == System::Windows {
        return format!(
            "[Environment]::SetEnvironmentVariable(\"Path\", \"{};\" + [Environment]::GetEnvironmentVariable(\"Path\", \"User\"), \"User\")",
            directory.display()
        );
    }
    let written = match home.and_then(|home| directory.strip_prefix(home).ok()) {
        Some(relative) => format!("$HOME/{}", relative.display()),
        None => directory.display().to_string(),
    };
    let fish = shell.map(Path::new).and_then(Path::file_name) == Some(OsStr::new("fish"));
    if fish {
        format!("set -gx PATH \"{written}\" $PATH")
    } else {
        format!("export PATH=\"{written}:$PATH\"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_comes_off_a_path_however_it_is_spelled() {
        let temporary = tempfile::tempdir().expect("a directory");
        let shims = temporary.path().join("shims");
        std::fs::create_dir(&shims).expect("a directory");
        let spelled = shims.join("..").join("shims");
        let path = std::env::join_paths([Path::new("/usr/bin"), &spelled, Path::new("/bin")])
            .expect("a path");
        assert!(contains(&path, &shims));
        let remaining = directories(&without(&path, &shims));
        assert_eq!(remaining, [PathBuf::from("/usr/bin"), PathBuf::from("/bin")]);
    }

    #[cfg(unix)]
    #[test]
    fn the_first_executable_is_found_past_what_is_skipped() {
        use std::os::unix::fs::PermissionsExt;
        let temporary = tempfile::tempdir().expect("a directory");
        let (first, second) = (temporary.path().join("first"), temporary.path().join("second"));
        for directory in [&first, &second] {
            std::fs::create_dir(directory).expect("a directory");
            let tool = directory.join("tool");
            std::fs::write(&tool, "#!/bin/sh\n").expect("a tool");
            std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).expect("mode");
        }
        std::fs::write(first.join("data"), "").expect("a file that is not executable");
        let directories = [first.clone(), second.clone()];

        let found = find_executable(&directories, "tool", &[], |_| false);
        assert_eq!(found, Some(first.join("tool")));
        let past = find_executable(&directories, "tool", &[], |path| path.starts_with(&first));
        assert_eq!(past, Some(second.join("tool")));
        assert_eq!(find_executable(&directories, "data", &[], |_| false), None);
        assert_eq!(find_executable(&directories, "missing", &[], |_| false), None);
    }

    #[test]
    fn the_path_line_matches_the_shell_and_travels_with_the_dotfiles() {
        let home = Path::new("/home/mark");
        let shims = Path::new("/home/mark/.local/share/pkg/shims");
        let line = |shell: Option<&str>| {
            path_line(System::Linux, shims, Some(home), shell.map(OsStr::new))
        };
        assert_eq!(line(Some("/bin/zsh")), "export PATH=\"$HOME/.local/share/pkg/shims:$PATH\"");
        assert_eq!(line(None), "export PATH=\"$HOME/.local/share/pkg/shims:$PATH\"");
        assert_eq!(
            line(Some("/usr/bin/fish")),
            "set -gx PATH \"$HOME/.local/share/pkg/shims\" $PATH"
        );
        let elsewhere = path_line(System::Macos, Path::new("/opt/shims"), Some(home), None);
        assert_eq!(elsewhere, "export PATH=\"/opt/shims:$PATH\"");
    }

    #[test]
    fn on_windows_the_line_sets_the_user_path() {
        let line = path_line(
            System::Windows,
            Path::new(r"C:\Users\mark\AppData\Local\pkg\shims"),
            None,
            None,
        );
        assert!(
            line.starts_with("[Environment]::SetEnvironmentVariable(\"Path\", \"C:\\Users"),
            "{line}"
        );
        assert!(line.ends_with("\"User\")"), "{line}");
    }
}
