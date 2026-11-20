//! Draws the board with indicatif, the only code that names it.

use std::io::Write;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

use super::board::{Line, View};

pub(super) struct Display {
    multi: MultiProgress,
    /// The spinner, the step, and its elapsed time, which move whether or not mise prints.
    headline: ProgressBar,
    /// One bar per line under the headline, and whether its style draws a bar.
    lines: Vec<(ProgressBar, bool)>,
    step: u64,
    /// Where lines go once the display is gone.
    after: Box<dyn Write>,
}

impl Display {
    pub(super) fn on_stderr() -> Display {
        Display::new(ProgressDrawTarget::stderr(), Box::new(std::io::stderr()))
    }

    pub(super) fn new(target: ProgressDrawTarget, after: Box<dyn Write>) -> Display {
        let multi = MultiProgress::with_draw_target(target);
        let headline = multi.add(
            ProgressBar::new_spinner().with_style(style("{spinner:.cyan} {msg} {elapsed:.dim}")),
        );
        headline.enable_steady_tick(Duration::from_millis(100));
        Display { multi, headline, lines: Vec::new(), step: 0, after }
    }

    /// Writes `lines` above the display, each without its line ending.
    pub(super) fn print(&self, lines: &[String]) {
        for line in lines {
            // Like the plain forwarding, a stderr that refuses a line has no one to tell.
            let _ = self.multi.println(line.trim_end_matches('\n'));
        }
    }

    pub(super) fn draw(&mut self, view: &View) {
        if view.step != self.step {
            self.step = view.step;
            self.headline.reset_elapsed();
        }
        self.headline.set_message(view.headline.clone());
        for (index, line) in view.lines.iter().enumerate() {
            match self.lines.get(index) {
                Some((bar, has_bar)) if *has_bar == line.bar.is_some() => update(bar, line),
                _ => self.replace(index, line),
            }
        }
        for (bar, _) in self.lines.drain(view.lines.len()..) {
            bar.finish_and_clear();
            self.multi.remove(&bar);
        }
    }

    /// Puts a new bar for `line` at `index`, built before it is shown so no half-set state is
    /// ever drawn.
    fn replace(&mut self, index: usize, line: &Line) {
        let has_bar = line.bar.is_some();
        let bar = ProgressBar::hidden().with_style(style(if has_bar { BAR } else { TEXT }));
        update(&bar, line);
        let bar = match self.lines.get_mut(index) {
            Some(old) => {
                let bar = self.multi.insert_after(&old.0, bar);
                self.multi.remove(&old.0);
                *old = (bar.clone(), has_bar);
                bar
            }
            None => {
                let bar = self.multi.add(bar);
                self.lines.push((bar.clone(), has_bar));
                bar
            }
        };
        // A bar shows in the display only once it has drawn there; what it drew while hidden
        // does not count.
        bar.tick();
    }

    /// Removes the display, then writes `rest` exactly as it is.
    pub(super) fn close(mut self, rest: &[String]) {
        for (bar, _) in &self.lines {
            bar.finish_and_clear();
        }
        self.headline.finish_and_clear();
        let _ = self.multi.clear();
        for line in rest {
            let _ = self.after.write_all(line.as_bytes());
        }
        let _ = self.after.flush();
    }
}

const TEXT: &str = "  {prefix}{wide_msg}";
const BAR: &str = "  {prefix}{bar:20.cyan/blue} {wide_msg}";

/// # Panics
///
/// When a template above is malformed, which the display's tests would show.
fn style(template: &str) -> ProgressStyle {
    ProgressStyle::with_template(template).expect("a valid template").progress_chars("█░")
}

fn update(bar: &ProgressBar, line: &Line) {
    bar.set_prefix(if line.label.is_empty() { String::new() } else { format!("{}  ", line.label) });
    if let Some((done, total)) = line.bar {
        bar.set_length(u64::try_from(total).unwrap_or(u64::MAX));
        bar.set_position(u64::try_from(done).unwrap_or(u64::MAX));
    }
    bar.set_message(line.text.clone());
}
