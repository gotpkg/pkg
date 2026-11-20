//! Whether a run shows the live display: the only code that asks the terminal and the
//! environment.

use std::ffi::OsStr;
use std::io::IsTerminal;

use crate::invocation::OutputFormat;

/// With text output, a stderr indicatif draws on, and `CI` unset or empty.
pub(crate) fn shows_display(output: OutputFormat) -> bool {
    output == OutputFormat::Text
        && std::io::stderr().is_terminal()
        && draws_on(std::env::var("TERM").ok().as_deref(), std::env::var_os("CI").as_deref())
}

/// indicatif draws nothing when `TERM` is `dumb`, or unset outside Windows. Redrawn output
/// mangles CI logs, so `CI` turns the display off, as it does Cargo's and mise's.
fn draws_on(term: Option<&str>, ci: Option<&OsStr>) -> bool {
    let term = match term {
        Some(term) => term != "dumb",
        None => cfg!(windows),
    };
    term && ci.is_none_or(OsStr::is_empty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dumb_or_missing_terminal_or_ci_turns_the_display_off() {
        let ci = |value: &'static str| Some(OsStr::new(value));
        assert!(draws_on(Some("xterm-256color"), None));
        assert!(draws_on(Some("xterm-256color"), ci("")));
        assert!(!draws_on(Some("xterm-256color"), ci("true")));
        assert!(!draws_on(Some("dumb"), None));
        assert_eq!(draws_on(None, None), cfg!(windows));
    }
}
