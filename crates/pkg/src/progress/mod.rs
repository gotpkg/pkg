//! The live display `install`, `update`, `sync`, `remove`, and `prune` show on a terminal.
//!
//! [`shows_display`] decides once per run whether it is on. When it is, [`Live`] takes the
//! operation's progress events in place of stderr: it assembles mise's output into lines
//! ([`lines`]), recognizes them ([`recognize`]), keeps what the display shows ([`board`]), and
//! draws it ([`display`]). Completion lines and lines it does not recognize stay on stderr above
//! the display; transient progress lines only update it, and are written after all when the
//! step they arrived in fails.

mod board;
mod display;
mod lines;
mod recognize;
mod terminal;

use pkg_core::Progress;

use board::Board;
use display::Display;
use lines::Lines;
use recognize::{Kind, Recognizer};
pub(crate) use terminal::shows_display;

pub(crate) struct Live {
    lines: Lines,
    recognizer: Recognizer,
    board: Board,
    display: Display,
    /// The transient lines of the current step.
    withheld: Vec<String>,
}

impl Live {
    pub(crate) fn on_stderr() -> Live {
        Live::new(Display::on_stderr())
    }

    fn new(display: Display) -> Live {
        Live {
            lines: Lines::default(),
            recognizer: Recognizer::default(),
            board: Board::default(),
            display,
            withheld: Vec::new(),
        }
    }

    pub(crate) fn observe(&mut self, event: Progress<'_>) {
        match event {
            Progress::Output { stream, text } => {
                let lines = self.lines.push(stream, text);
                let kept = self.sort(lines);
                self.display.print(&kept);
            }
            Progress::Step(step) => {
                let rest = self.lines.take_rest();
                let kept = self.sort(rest);
                self.display.print(&kept);
                self.withheld.clear();
                self.board.begin(step);
            }
            Progress::Plan(items) => self.board.plan(items),
            // `report` writes the warnings an outcome retains once the display is gone.
            Progress::Warning(_) => {}
        }
        self.display.draw(&self.board.view());
    }

    /// Removes the display, then writes what stderr is still owed: after a failure, the lines
    /// withheld during the failing step, and what mise wrote after its last newline.
    pub(crate) fn finish(mut self, failed: bool) {
        let rest = self.lines.take_rest();
        let kept = self.sort(rest);
        let mut owed = if failed { std::mem::take(&mut self.withheld) } else { Vec::new() };
        owed.extend(kept);
        self.display.close(&owed);
    }

    /// Updates the board from `lines` and returns those that stay on stderr; the transient ones
    /// are withheld.
    fn sort(&mut self, lines: Vec<String>) -> Vec<String> {
        let mut kept = Vec::new();
        for line in lines {
            match self.recognizer.recognize(&line) {
                Kind::Transient(report) => {
                    if let Some(report) = report {
                        self.board.apply(report);
                    }
                    self.withheld.push(line);
                }
                Kind::Completion(report) => {
                    self.board.apply(report);
                    kept.push(line);
                }
                Kind::Other => kept.push(line),
            }
        }
        kept
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use indicatif::{InMemoryTerm, ProgressDrawTarget, TermLike};
    use pkg_core::{OutputStream, PackageStep, PlannedAction, PlannedItem, Step};

    use super::*;

    /// Writes to the in-memory terminal the display draws on.
    struct Screen(InMemoryTerm);

    impl Write for Screen {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.write_str(&String::from_utf8_lossy(bytes).replace('\n', "\r\n"))?;
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            TermLike::flush(&self.0)
        }
    }

    fn live() -> (Live, InMemoryTerm) {
        let term = InMemoryTerm::new(40, 200);
        let target = ProgressDrawTarget::term_like(Box::new(term.clone()));
        (Live::new(Display::new(target, Box::new(Screen(term.clone())))), term)
    }

    /// Feeds `recording` to stderr in uneven chunks, as a pipe delivers it.
    fn output(live: &mut Live, recording: &str) {
        let mut rest = recording;
        while !rest.is_empty() {
            let mut end = rest.len().min(37);
            while !rest.is_char_boundary(end) {
                end += 1;
            }
            live.observe(Progress::Output { stream: OutputStream::Stderr, text: &rest[..end] });
            rest = &rest[end..];
        }
    }

    fn install(package: &str, position: usize, count: usize) -> Progress<'_> {
        Progress::Step(Step::Install(PackageStep { package, position, count }))
    }

    #[test]
    fn kept_lines_stay_above_the_display_and_nothing_of_it_is_left_behind() {
        let (mut live, term) = live();
        let recording = include_str!("recorded/pour-wget.stderr");
        let plan: Vec<PlannedItem> = ["ca-certificates", "json-c", "libunistring", "wget"]
            .into_iter()
            .map(|name| PlannedItem {
                name: format!("brew:{name}"),
                version: None,
                action: PlannedAction::InstallDependency,
            })
            .collect();
        live.observe(install("brew:wget", 1, 2));
        live.observe(Progress::Plan(&plan));
        let (first, _) =
            recording.split_at(recording.find("mise brew:json-c     checksum").unwrap());
        output(&mut live, first);
        let screen = term.contents();
        let rows: Vec<&str> = screen.lines().collect();
        assert_eq!(rows[0], "mise ~/.config/mise/config.toml: added brew:wget", "{screen}");
        assert_eq!(rows[1], "mise brew:ca-certificates ✓ 2026-11-05", "{screen}");
        assert!(rows[2].contains("Installing brew:wget, package 1 of 2"), "{screen}");
        assert!(screen.contains("0/2 packages"), "{screen}");
        assert!(screen.contains("1/8 formulae, 4 of them dependencies"), "{screen}");
        assert!(
            rows.iter().any(|row| row.trim_start().starts_with("brew:wget ")
                && row.ends_with("download wget-1.25.0_2.tar.gz")),
            "{screen}"
        );
        assert!(!screen.contains("mise brew:wget       download"), "{screen}");

        output(&mut live, &recording[first.len()..]);
        live.finish(false);
        let kept: Vec<&str> = recording
            .lines()
            .filter(|line| {
                line.contains('✓') || line.contains("mise ~/") || line.contains("brew: ")
            })
            .collect();
        assert_eq!(term.contents(), kept.join("\n"));
    }

    #[test]
    fn a_failure_writes_the_lines_withheld_during_the_failing_step() {
        let (mut live, term) = live();
        live.observe(install("node", 1, 2));
        output(&mut live, include_str!("recorded/tools-install.stderr"));
        live.observe(install("node@0.0.999", 2, 2));
        output(&mut live, include_str!("recorded/fail-tool-version.stderr"));
        live.finish(true);
        let screen = term.contents();
        let rows: Vec<&str> = screen.lines().collect();
        assert_eq!(rows.len(), 11, "{screen}");
        assert_eq!(rows[0], "mise bun@1.4.5 1.4.5");
        assert!(rows[5].starts_with("mise ✗ node@0.0.999  746ms · failed:"), "{screen}");
        assert!(rows[6..9].iter().all(|row| row.starts_with("mise ERROR ")), "{screen}");
        assert_eq!(rows[9], "mise by @jdx – installing 1 tool");
        assert!(rows[10].ends_with("1/1 · installed 0 tools · 1 failed in 748ms"), "{screen}");
        assert!(!screen.contains("installing 2 tools"), "an earlier step's lines: {screen}");
    }

    #[test]
    fn what_mise_wrote_after_its_last_newline_is_written_at_the_end() {
        let (mut live, term) = live();
        live.observe(Progress::Step(Step::UpdateTools));
        output(&mut live, "mise WARN no newline");
        live.finish(false);
        assert_eq!(term.contents(), "mise WARN no newline");
    }

    #[test]
    fn a_line_ending_in_a_carriage_return_stays_above_the_display() {
        let (mut live, term) = live();
        live.observe(install("brew:hello", 1, 1));
        output(&mut live, "mise brew:hello      checksum\r\nmise brew:hello    ✓ 2.12.3\r\n");
        let screen = term.contents();
        assert_eq!(screen.lines().next(), Some("mise brew:hello    ✓ 2.12.3"), "{screen}");
        assert!(screen.contains("Installing brew:hello"), "{screen}");
        live.finish(false);
        assert_eq!(term.contents(), "mise brew:hello    ✓ 2.12.3");
    }
}
