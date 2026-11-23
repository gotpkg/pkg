//! Recognizes the plain lines mise prints when its stderr is not a terminal, as mise 2026.11.9
//! writes them.
//!
//! Each line is one of three kinds:
//!
//! - **Transient**: it only reports progress, so it updates the display and is not left on
//!   stderr. These are a tool session's header (`by @jdx – installing 2 tools`, also `resolving`
//!   and `removing`, and `1 tool`); its snapshots, a 16-cell bar of `█` and `░` with the count
//!   done, a `·` and the elapsed time (`██░░░░░░░░░░░░░░ 0/2 · 3.1s`), each followed by rows
//!   indented by two spaces in columns two or more spaces apart: one per active tool (label,
//!   phase, elapsed time, and the transfer when there is one:
//!   `  node@24.22.3  downloading  3.1s  17.5/53.1 MB · 6.5 MB/s`), one per tool held back
//!   (`  fd@10.5.2  waiting for node`), and a count of the rest (`  3 queued`); its closing
//!   summary, a bar, the count, a `·` and a past-tense verb (`████████████████ 2/2 · installed
//!   2 tools in 15.7s`); and a bootstrap package's phase, its `manager:name` label padded with
//!   spaces and one of the words `download <file>`, `checksum`, `extract` (alone or with the
//!   file), `relocate`, `codesign`, `link`, or `post-install` (`brew:wget       checksum`). In a
//!   session that only looks versions up (`by @jdx – resolving 67 tools`, which `mise upgrade`
//!   prints for every tool in the config before it installs anything), a tool's `✓` line is
//!   transient too.
//! - **Completion**: it records that an item finished, so it updates the display and stays on
//!   stderr unchanged. These are a tool's `✓`, `⇢`, or `✗` line, the icon, the label, two
//!   spaces, and the elapsed time (`✓ bun@1.4.5     11.4s  bun-darwin-aarch64.zip`,
//!   `⇢ node@24.22.3  6ms · already installed`, `✗ node@0.0.999  746ms · failed: …`), and a
//!   bootstrap package's label, `✓`, and its version (`brew:wget     ✓ 1.25.0_2`). In a session
//!   that looks versions up, only a failed lookup's `✗` line is.
//! - **Other**: warnings, errors, the output of programs mise runs, and every line that does not
//!   match exactly one shape above. It stays on stderr unchanged.
//!
//! Shapes are matched after color codes and the leading `mise ` are set aside; snapshot rows have
//! no `mise `. Two things are kept between lines. One is whether the previous line was a snapshot
//! or one of its rows, so an indented line elsewhere, such as the advice under a warning, or one
//! after a snapshot that has no row's shape, is Other. The other is whether a session that looks
//! versions up has begun and not yet printed its summary. The files in `recorded/` are mise
//! 2026.11.9's output, recorded piped in a disposable home and Homebrew prefix; when the
//! supported mise changes, record them again.

/// What a line says, for the display.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Report {
    /// A tool session with `total` tools begins.
    Session { total: usize },
    /// A snapshot: `done` of `total` tools finished. Its rows follow.
    Snapshot { done: usize, total: usize },
    /// A snapshot row: what one tool is doing, and its transfer when mise reports one.
    Row { item: String, phase: String, transfer: Option<String> },
    /// A tool session's closing summary.
    Summary { done: usize, total: usize },
    /// A session that looks versions up begins, takes a snapshot, or ends. Nothing it does is
    /// counted; a snapshot's rows follow.
    Resolving,
    /// A tool's version lookup finished.
    Resolved { item: String },
    /// A bootstrap package's phase, such as `checksum` or `download wget-1.25.0_2.tar.gz`.
    Phase { item: String, phase: String },
    /// An item finished, with its version when the line states it apart from the label.
    Done { item: String, version: Option<String> },
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// Progress only; `None` for a snapshot row that names no tool, such as `  3 queued`.
    Transient(Option<Report>),
    Completion(Report),
    Other,
}

#[derive(Default)]
pub(super) struct Recognizer {
    in_snapshot: bool,
    /// Whether the session under way only looks versions up.
    resolving: bool,
}

impl Recognizer {
    /// Sorts one line, with or without its line ending.
    pub(super) fn recognize(&mut self, line: &str) -> Kind {
        let plain = without_colors(line.trim_end_matches(['\n', '\r']));
        if self.in_snapshot
            && plain.starts_with("  ")
            && let Some(row) = row(&plain)
        {
            return Kind::Transient(row);
        }
        self.in_snapshot = false;
        let Some(text) = plain.strip_prefix("mise ") else {
            return Kind::Other;
        };
        let words: Vec<&str> = text.split_whitespace().collect();
        if let Some(report) = snapshot(&words) {
            self.in_snapshot = true;
            return Kind::Transient(Some(if self.resolving { Report::Resolving } else { report }));
        }
        if let Some(report) = session(&words) {
            self.resolving = report == Report::Resolving;
            return Kind::Transient(Some(report));
        }
        if let Some(report) = summary(&words) {
            self.resolving = false;
            return Kind::Transient(Some(report));
        }
        if let Some(report) = phase(&words) {
            return Kind::Transient(Some(report));
        }
        if let Some((icon, label)) = tool_done(&words) {
            return self.tool_finished(icon, label.to_string());
        }
        package_done(&words).map_or(Kind::Other, Kind::Completion)
    }

    /// A tool's `✓`, `⇢`, or `✗` line. A version lookup is worth keeping only when it failed.
    fn tool_finished(&self, icon: &str, item: String) -> Kind {
        match (self.resolving, icon) {
            (false, _) => Kind::Completion(Report::Done { item, version: None }),
            (true, "✗") => Kind::Completion(Report::Resolved { item }),
            (true, _) => Kind::Transient(Some(Report::Resolved { item })),
        }
    }
}

/// `by @jdx – installing 2 tools`, or `resolving` and `removing`; `1 tool`.
fn session(words: &[&str]) -> Option<Report> {
    let ["by", "@jdx", "\u{2013}", verb, count, noun] = words else {
        return None;
    };
    let total = count.parse().ok()?;
    if !matches!(*noun, "tool" | "tools") {
        return None;
    }
    match *verb {
        "resolving" => Some(Report::Resolving),
        "installing" | "removing" => Some(Report::Session { total }),
        _ => None,
    }
}

/// `██░░░░░░░░░░░░░░ 0/2 · 3.1s`.
fn snapshot(words: &[&str]) -> Option<Report> {
    let [bar, count, "·", elapsed] = words else {
        return None;
    };
    let (done, total) = tally(count)?;
    (is_bar(bar) && is_elapsed(elapsed)).then_some(Report::Snapshot { done, total })
}

/// `████████████████ 2/2 · installed 2 tools in 15.7s`, or `resolved` and `removed`.
fn summary(words: &[&str]) -> Option<Report> {
    let [bar, count, "·", verb, ..] = words else {
        return None;
    };
    let (done, total) = tally(count)?;
    if !is_bar(bar) {
        return None;
    }
    match *verb {
        "resolved" => Some(Report::Resolving),
        "installed" | "removed" => Some(Report::Summary { done, total }),
        _ => None,
    }
}

/// A snapshot row: `  node@24.22.3  downloading  3.1s  17.5/53.1 MB · 6.5 MB/s` (the transfer
/// only during one), `  fd@10.5.2  waiting for node`, or `  3 queued`, which names no tool.
/// `None` for an indented line of any other shape.
fn row(plain: &str) -> Option<Option<Report>> {
    let cells: Vec<&str> =
        plain.split("  ").map(str::trim).filter(|cell| !cell.is_empty()).collect();
    let report = |item: &str, phase: &str, transfer: Option<&str>| Report::Row {
        item: item.to_string(),
        phase: phase.to_string(),
        transfer: transfer.map(str::to_string),
    };
    match cells.as_slice() {
        [queued] => {
            let count = queued.strip_suffix(" queued")?;
            count.parse::<usize>().ok().map(|_| None)
        }
        [item, waiting] if waiting.starts_with("waiting for ") => {
            Some(Some(report(item, waiting, None)))
        }
        [item, phase, elapsed] if is_elapsed(elapsed) => Some(Some(report(item, phase, None))),
        [item, phase, elapsed, transfer] if is_elapsed(elapsed) => {
            Some(Some(report(item, phase, Some(transfer))))
        }
        _ => None,
    }
}

/// `brew:wget       download wget-1.25.0_2.tar.gz`, `brew:wget       checksum`.
fn phase(words: &[&str]) -> Option<Report> {
    let [label, phase, rest @ ..] = words else {
        return None;
    };
    let known = matches!(
        (*phase, rest.len()),
        ("download", 1)
            | ("extract", 0 | 1)
            | ("checksum" | "relocate" | "codesign" | "link" | "post-install", 0)
    );
    (known && is_package_label(label))
        .then(|| Report::Phase { item: (*label).to_string(), phase: words[1..].join(" ") })
}

/// `✓ bun@1.4.5     11.4s  bun-darwin-aarch64.zip`, `⇢ …`, `✗ …`: the icon and the label.
fn tool_done<'a>(words: &[&'a str]) -> Option<(&'a str, &'a str)> {
    let [icon, label, elapsed, ..] = words else {
        return None;
    };
    (matches!(*icon, "✓" | "⇢" | "✗") && is_elapsed(elapsed)).then_some((*icon, *label))
}

/// `brew:wget     ✓ 1.25.0_2`.
fn package_done(words: &[&str]) -> Option<Report> {
    let [label, "✓", version] = words else {
        return None;
    };
    is_package_label(label)
        .then(|| Report::Done { item: (*label).to_string(), version: Some((*version).to_string()) })
}

/// `brew:wget`: a manager of lowercase letters, digits and dashes, a colon, and a name.
fn is_package_label(word: &str) -> bool {
    word.split_once(':').is_some_and(|(manager, name)| {
        !manager.is_empty()
            && !name.is_empty()
            && manager
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    })
}

fn is_bar(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|cell| cell == '█' || cell == '░')
}

/// `0/2`.
fn tally(word: &str) -> Option<(usize, usize)> {
    let (done, total) = word.split_once('/')?;
    Some((done.parse().ok()?, total.parse().ok()?))
}

/// `6ms` or `11.4s`.
fn is_elapsed(word: &str) -> bool {
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if let Some(milliseconds) = word.strip_suffix("ms") {
        return digits(milliseconds);
    }
    word.strip_suffix('s')
        .and_then(|seconds| seconds.split_once('.'))
        .is_some_and(|(whole, tenths)| digits(whole) && tenths.len() == 1 && digits(tenths))
}

/// `line` without terminal escape sequences, such as the colors mise adds when forced to.
fn without_colors(line: &str) -> String {
    let mut plain = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(character) = chars.next() {
        if character != '\x1b' {
            plain.push(character);
        } else if chars.next() == Some('[') {
            // A control sequence ends at its first byte from `@` to `~`.
            for character in chars.by_ref() {
                if ('@'..='~').contains(&character) {
                    break;
                }
            }
        }
    }
    plain
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lines of a recording that stay on stderr, and how many only update the display.
    fn sorted(recording: &str) -> (Vec<&str>, usize) {
        let mut recognizer = Recognizer::default();
        let mut kept = Vec::new();
        let mut transient = 0;
        for line in recording.lines() {
            match recognizer.recognize(line) {
                Kind::Transient(_) => transient += 1,
                Kind::Completion(_) | Kind::Other => kept.push(line),
            }
        }
        (kept, transient)
    }

    #[test]
    fn a_homebrew_pour_keeps_each_finished_formula_and_mises_own_lines() {
        let (kept, transient) = sorted(include_str!("recorded/pour-wget.stderr"));
        assert_eq!(
            kept,
            [
                "mise ~/.config/mise/config.toml: added brew:wget",
                "mise brew:ca-certificates ✓ 2026-11-05",
                "mise brew:json-c   ✓ 0.19",
                "mise brew:libunistring ✓ 1.5",
                "mise brew:gettext  ✓ 1.0",
                "mise brew:libidn2  ✓ 2.3.8",
                "mise brew:libpsl   ✓ 0.23.3",
                "mise brew:openssl@4 ✓ 4.1.0",
                "mise brew:wget     ✓ 1.25.0_2",
                "mise brew: installed wget",
            ]
        );
        assert_eq!(transient, 57);
    }

    #[test]
    fn a_tool_session_keeps_completions_and_what_the_tools_print() {
        let (kept, transient) = sorted(include_str!("recorded/tools-install.stderr"));
        assert_eq!(
            kept,
            [
                "mise bun@1.4.5 1.4.5",
                "mise ✓ bun@1.4.5     11.4s  bun-darwin-aarch64.zip",
                "mise node@24.22.3 v24.22.3",
                "mise node@24.22.3 11.19.1",
                "mise ✓ node@24.22.3  15.7s  node-v24.22.3-darwin-arm64.tar.gz",
            ]
        );
        assert_eq!(transient, 15);
    }

    #[test]
    fn an_indented_line_outside_a_snapshot_stays_on_stderr() {
        let (kept, transient) = sorted(include_str!("recorded/tool-already-installed.stderr"));
        assert_eq!(
            kept,
            [
                "mise ⇢ node@24.22.3  6ms · already installed",
                "mise ✓ fd@10.5.2     2.2s  fd-v10.5.2-aarch64-apple-darwin.tar.gz",
                "mise WARN  fd installed but not activated — it is not in any config file.",
                "To install and activate, run:",
                "  mise use fd",
            ]
        );
        assert_eq!(transient, 2);
    }

    #[test]
    fn an_update_keeps_what_it_installed_and_not_what_it_resolved() {
        let (kept, transient) = sorted(include_str!("recorded/upgrade-tools.stderr"));
        assert_eq!(
            kept,
            [
                "mise ✓ jq@1.8.2  1.3s  jq-macos-arm64",
                "mise checking tracked configs resolving tracked configs",
                "mise checking tracked configs ✓ done",
            ]
        );
        assert_eq!(transient, 11);
        let (kept, transient) = sorted(include_str!("recorded/upgrade-packages.stderr"));
        assert_eq!(kept.len(), 2);
        assert_eq!(transient, 0);
    }

    #[test]
    fn a_failed_version_lookup_stays_on_stderr() {
        let recording = [
            "mise by @jdx – resolving 2 tools",
            "mise ✓ jq    1ms",
            "mise ✗ nope  3ms · failed: no versions found for nope",
            "mise ████████████████ 2/2 · resolved 1 tool · 1 failed in 3ms",
            "mise ✓ jq@1.8.2  1.3s  jq-macos-arm64",
        ]
        .join("\n");
        let (kept, transient) = sorted(&recording);
        assert_eq!(
            kept,
            [
                "mise ✗ nope  3ms · failed: no versions found for nope",
                "mise ✓ jq@1.8.2  1.3s  jq-macos-arm64",
            ]
        );
        assert_eq!(transient, 3);
    }

    #[test]
    fn a_sync_keeps_the_installs_and_mises_counts() {
        let (kept, transient) = sorted(include_str!("recorded/sync-packages.stderr"));
        assert_eq!(
            kept,
            [
                "mise brew: 2 package(s) already installed",
                "mise brew:tree     ✓ 2.3.2",
                "mise brew: installed tree",
            ]
        );
        assert_eq!(transient, 6);
        let (kept, transient) = sorted(include_str!("recorded/sync-tools.stderr"));
        assert_eq!(
            kept,
            [
                "mise ⇢ bun@latest      0ms · already installed",
                "mise ⇢ node@24         1ms · already installed",
                "mise ✓ ripgrep@15.2.0  897ms  ripgrep-15.2.0-aarch64-apple-darwin.tar.gz",
            ]
        );
        assert_eq!(transient, 2);
    }

    #[test]
    fn a_remove_leaves_every_line_on_stderr() {
        for recording in
            [include_str!("recorded/prune.stderr"), include_str!("recorded/unuse-tool.stderr")]
        {
            let (kept, transient) = sorted(recording);
            assert_eq!(kept, recording.lines().collect::<Vec<_>>());
            assert_eq!(transient, 0);
        }
    }

    #[test]
    fn a_failure_keeps_the_failed_tool_and_every_error_line() {
        for (recording, failed) in [
            (
                include_str!("recorded/fail-tool-version.stderr"),
                "mise ✗ node@0.0.999  746ms · failed: HTTP status",
            ),
            (
                include_str!("recorded/fail-unknown-tool.stderr"),
                "mise ✗ no-such-tool-pkg-test@latest  0ms · failed: no-such-tool-pkg-test",
            ),
        ] {
            let (kept, transient) = sorted(recording);
            assert_eq!(kept.len(), 4, "{kept:?}");
            assert!(kept[0].starts_with(failed), "{kept:?}");
            assert!(kept[1..].iter().all(|line| line.starts_with("mise ERROR ")), "{kept:?}");
            assert_eq!(transient, 2);
        }
        let recording = include_str!("recorded/fail-unknown-formula.stderr");
        assert_eq!(sorted(recording).0, recording.lines().collect::<Vec<_>>());
    }

    #[test]
    fn reports_carry_what_mise_said() {
        let mut recognizer = Recognizer::default();
        let mut report = |line: &str| match recognizer.recognize(line) {
            Kind::Transient(Some(report)) | Kind::Completion(report) => Some(report),
            Kind::Transient(None) | Kind::Other => None,
        };
        assert_eq!(
            report("mise by @jdx – installing 2 tools\n"),
            Some(Report::Session { total: 2 })
        );
        assert_eq!(
            report("mise ██░░░░░░░░░░░░░░ 0/2 · 3.1s"),
            Some(Report::Snapshot { done: 0, total: 2 })
        );
        assert_eq!(
            report("  node@24.22.3  downloading  3.1s  17.5/53.1 MB · 6.5 MB/s"),
            Some(Report::Row {
                item: "node@24.22.3".into(),
                phase: "downloading".into(),
                transfer: Some("17.5/53.1 MB · 6.5 MB/s".into()),
            })
        );
        assert_eq!(
            report("  node@24.22.3  node -v  15.2s"),
            Some(Report::Row {
                item: "node@24.22.3".into(),
                phase: "node -v".into(),
                transfer: None
            })
        );
        assert_eq!(report("  3 queued"), None);
        assert_eq!(
            report("mise ████████████████ 2/2 · installed 2 tools in 15.7s"),
            Some(Report::Summary { done: 2, total: 2 })
        );
        assert_eq!(
            report("mise brew:wget       download wget-1.25.0_2.tar.gz"),
            Some(Report::Phase {
                item: "brew:wget".into(),
                phase: "download wget-1.25.0_2.tar.gz".into(),
            })
        );
        assert_eq!(
            report("mise brew:openssl@4 ✓ 4.1.0"),
            Some(Report::Done { item: "brew:openssl@4".into(), version: Some("4.1.0".into()) })
        );
        assert_eq!(
            report("mise ⇢ node@24.22.3  6ms · already installed"),
            Some(Report::Done { item: "node@24.22.3".into(), version: None })
        );
    }

    #[test]
    fn color_codes_and_line_endings_are_set_aside() {
        let mut recognizer = Recognizer::default();
        assert_eq!(
            recognizer.recognize("\x1b[2mmise\x1b[0m \x1b[36mbrew:wget\x1b[0m       checksum\r\n"),
            Kind::Transient(Some(Report::Phase {
                item: "brew:wget".into(),
                phase: "checksum".into()
            }))
        );
    }

    #[test]
    fn a_line_close_to_a_known_shape_stays_on_stderr() {
        let mut recognizer = Recognizer::default();
        for line in [
            "mise brew:wget       checksum and more",
            "mise brew:wget       unpack",
            "mise brew:wget     ✓ 1.25.0_2 extra",
            "mise by @jdx – installing several tools",
            "mise ██░░ 0/2 · soon",
            "mise ✓ jq",
            "brew:wget       checksum",
            "  node@24.22.3  downloading  3.1s",
        ] {
            assert_eq!(recognizer.recognize(line), Kind::Other, "{line}");
        }
    }

    #[test]
    fn an_indented_line_after_a_snapshot_is_a_row_only_in_a_rows_shape() {
        let mut recognizer = Recognizer::default();
        assert!(matches!(
            recognizer.recognize("mise ██░░░░░░░░░░░░░░ 0/3 · 3.1s"),
            Kind::Transient(Some(Report::Snapshot { .. }))
        ));
        assert_eq!(
            recognizer.recognize("  fd@10.5.2    waiting for node"),
            Kind::Transient(Some(Report::Row {
                item: "fd@10.5.2".into(),
                phase: "waiting for node".into(),
                transfer: None,
            }))
        );
        assert_eq!(recognizer.recognize("  1 queued"), Kind::Transient(None));
        assert_eq!(recognizer.recognize("  WARNING: low disk space"), Kind::Other);
        assert_eq!(recognizer.recognize("  node@24.22.3  downloading  3.1s"), Kind::Other);
    }
}
