//! The files a shim is: a short script that calls `pkg shims run`.

use std::path::Path;

use super::catalog::System;

/// The word every shim carries on its second line, so `pkg` replaces and deletes only its own
/// files.
pub(crate) const MARKER: &str = "pkg-shim";

/// How much of a file is read to look for the marker.
const MARKER_WINDOW: usize = 512;

/// The comment prefixes that start the marker line: shell and batch.
const MARKER_LINES: [&str; 2] = ["# pkg-shim", "REM pkg-shim"];

/// The shim file for `tool`: `npm`, or `npm.cmd` on Windows.
pub(crate) fn file_name(tool: &str, system: System) -> String {
    if system == System::Windows { format!("{tool}.cmd") } else { tool.to_string() }
}

/// The tool a shim file is for, without the `.cmd` suffix.
pub(crate) fn tool_name(file_name: &str) -> &str {
    file_name.strip_suffix(".cmd").unwrap_or(file_name)
}

/// What a shim file contains. `directory` is where it is installed.
pub(crate) fn script(tool: &str, system: System, directory: &Path) -> String {
    if system == System::Windows { windows_script(tool, directory) } else { unix_script(tool) }
}

/// Whether the start of a file says it is a shim: one of its first lines is the marker comment.
pub(crate) fn is_shim(start: &[u8]) -> bool {
    let start = &start[..start.len().min(MARKER_WINDOW)];
    String::from_utf8_lossy(start)
        .lines()
        .take(3)
        .any(|line| MARKER_LINES.iter().any(|prefix| line.starts_with(prefix)))
}

/// The fallback when `pkg` is not on `PATH` reads each candidate's second line with the shell's
/// own `read`, so it needs no other program, and skips every file whose second line is the marker:
/// a shim in another directory too, which would otherwise hand the call back and forth forever.
fn unix_script(tool: &str) -> String {
    format!(
        r##"#!/bin/sh
# {MARKER}: written by `pkg shims install`; `pkg shims remove` deletes it.
if command -v pkg >/dev/null 2>&1; then
  exec pkg shims run {tool} -- "$@"
fi
# pkg is no longer on PATH: run the first real {tool}.
IFS=:
for directory in $PATH; do
  candidate="$directory/{tool}"
  [ -n "$directory" ] && [ -f "$candidate" ] && [ -x "$candidate" ] || continue
  {{ IFS= read -r first; IFS= read -r second; }} < "$candidate" 2>/dev/null
  case $second in "# {MARKER}"*) continue ;; esac
  exec "$candidate" "$@"
done
echo "{tool}: not installed on this machine" >&2
exit 127
"##
    )
}

/// The directory is quoted, with `%` doubled, because it comes from the person's environment and
/// a batch file would otherwise run what a `&` in it starts. The marker comes first and the
/// message needs no parenthesized block, which a `)` in the directory would end early.
fn windows_script(tool: &str, directory: &Path) -> String {
    let directory = directory.display().to_string().replace('%', "%%");
    let lines = [
        "@echo off".to_string(),
        format!("REM {MARKER}: written by `pkg shims install`; `pkg shims remove` deletes it."),
        "where pkg >nul 2>nul".to_string(),
        "if not errorlevel 1 goto run".to_string(),
        format!(
            "echo {tool}: pkg was not found on PATH. Run `pkg shims remove`, or take \"{directory}\" off PATH. 1>&2"
        ),
        "exit /b 127".to_string(),
        ":run".to_string(),
        format!("pkg shims run {tool} -- %*"),
        "exit /b %errorlevel%".to_string(),
    ];
    lines.join("\r\n") + "\r\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unix_shim_calls_pkg_and_falls_back_to_the_real_tool() {
        let script = script("npm", System::Linux, Path::new("/x"));
        assert!(script.starts_with("#!/bin/sh\n# pkg-shim"), "{script}");
        assert!(script.contains("exec pkg shims run npm -- \"$@\""), "{script}");
        assert!(script.contains("exit 127"), "{script}");
        assert!(is_shim(script.as_bytes()));
    }

    #[test]
    fn a_windows_shim_is_a_batch_file_that_names_the_way_out() {
        let script = script("winget", System::Windows, Path::new(r"C:\pkg\shims"));
        assert!(script.contains("REM pkg-shim"), "{script}");
        assert!(script.contains("pkg shims run winget -- %*"), "{script}");
        assert!(script.contains(r"C:\pkg\shims"), "{script}");
        assert!(script.lines().all(|line| !line.is_empty()), "{script:?}");
        assert!(script.ends_with("\r\n"));
    }

    #[test]
    fn file_names_follow_the_system() {
        assert_eq!(file_name("npm", System::Macos), "npm");
        assert_eq!(file_name("npm", System::Windows), "npm.cmd");
        assert_eq!(tool_name("npm.cmd"), "npm");
        assert_eq!(tool_name("npm"), "npm");
    }

    #[test]
    fn a_file_is_a_shim_only_when_a_line_near_its_start_says_so() {
        assert!(is_shim(b"#!/bin/sh\n# pkg-shim: written by pkg\nexit 1\n"));
        assert!(is_shim(b"@echo off\r\nREM pkg-shim: written by pkg\r\n"));
        assert!(!is_shim(b"#!/bin/sh\necho pkg-shim\n"));
        assert!(!is_shim(b"#!/bin/sh\n# this tool is not a pkg-shim\n"));
        assert!(!is_shim(b"#!/bin/sh\n\n\n\n# pkg-shim\n"));
    }

    #[test]
    fn a_windows_shim_quotes_the_directory_it_names_and_has_no_block_to_close() {
        let script = script("npm", System::Windows, Path::new(r"C:\Temp\Alice&calc&x (x86)\100%"));
        assert!(script.contains(r#""C:\Temp\Alice&calc&x (x86)\100%%""#), "{script}");
        assert!(script.lines().all(|line| !line.trim_end().ends_with('(')), "{script}");
        let unquoted: String = script
            .split('"')
            .enumerate()
            .filter(|(index, _)| index % 2 == 0)
            .map(|(_, outside)| outside)
            .collect();
        assert!(!unquoted.contains("calc"), "the directory is outside quotes: {script}");
    }

    #[test]
    fn only_the_start_of_a_file_is_searched_for_the_marker() {
        assert!(is_shim(b"#!/bin/sh\n# pkg-shim\n"));
        assert!(!is_shim(b"#!/bin/sh\necho hello\n"));
        let mut late = vec![b'x'; 600];
        late.extend_from_slice(b"pkg-shim");
        assert!(!is_shim(&late));
    }
}
