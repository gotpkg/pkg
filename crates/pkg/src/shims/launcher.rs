//! The files a shim is: a short script that calls `pkg shims run`.

use std::path::Path;

use super::catalog::System;

/// The word every shim carries, so `pkg` replaces and deletes only its own files.
pub(crate) const MARKER: &str = "pkg-shim";

/// How much of a file is read to look for the marker.
const MARKER_WINDOW: usize = 512;

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

/// Whether the start of a file says it is a shim.
pub(crate) fn is_shim(start: &[u8]) -> bool {
    let start = &start[..start.len().min(MARKER_WINDOW)];
    String::from_utf8_lossy(start).contains(MARKER)
}

fn unix_script(tool: &str) -> String {
    format!(
        r#"#!/bin/sh
# {MARKER}: written by `pkg shims install`; `pkg shims remove` deletes it.
if command -v pkg >/dev/null 2>&1; then
  exec pkg shims run {tool} -- "$@"
fi
# pkg is no longer on PATH: run the real {tool}.
IFS=:
for directory in $PATH; do
  if [ -n "$directory" ] && [ -f "$directory/{tool}" ] && [ -x "$directory/{tool}" ] && [ ! "$directory/{tool}" -ef "$0" ]; then
    exec "$directory/{tool}" "$@"
  fi
done
echo "{tool}: not installed on this machine" >&2
exit 127
"#
    )
}

fn windows_script(tool: &str, directory: &Path) -> String {
    let lines = [
        "@echo off".to_string(),
        format!("REM {MARKER}: written by `pkg shims install`; `pkg shims remove` deletes it."),
        "where pkg >nul 2>nul".to_string(),
        "if errorlevel 1 (".to_string(),
        format!(
            "  echo {tool}: pkg was not found on PATH. Run `pkg shims remove`, or take {} off PATH. 1>&2",
            directory.display()
        ),
        "  exit /b 127".to_string(),
        ")".to_string(),
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
    fn only_the_start_of_a_file_is_searched_for_the_marker() {
        assert!(is_shim(b"#!/bin/sh\n# pkg-shim\n"));
        assert!(!is_shim(b"#!/bin/sh\necho hello\n"));
        let mut late = vec![b'x'; 600];
        late.extend_from_slice(b"pkg-shim");
        assert!(!is_shim(&late));
    }
}
