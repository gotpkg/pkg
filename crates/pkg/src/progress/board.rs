//! What the display shows, kept from core's steps and plans and the mise lines recognized: the
//! step, the package's place in the request, how many items are finished, and a row for each
//! item mise is working on. Every figure is one mise reported or one pkg counted.

use pkg_core::{PlannedAction, PlannedItem, Step};

use super::recognize::Report;

/// One line under the headline.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Line {
    /// Leading text, padded so the rows line up; empty for none.
    pub label: String,
    /// A bar `done` of `total` after the label.
    pub bar: Option<(usize, usize)>,
    pub text: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct View {
    pub headline: String,
    /// Counts the steps begun, so the display can restart the clock it shows.
    pub step: u64,
    pub lines: Vec<Line>,
}

/// The most item rows shown; the rest are counted on one line.
const MOST_ROWS: usize = 8;

#[derive(Default)]
pub(super) struct Board {
    step: u64,
    headline: String,
    /// The package's position in the request and the number of packages.
    place: Option<(usize, usize)>,
    /// The install plan, then each item mise worked on that the plan left out.
    planned: Vec<PlannedItem>,
    dependencies: usize,
    /// `name version` of each package an uninstall plan lists.
    uninstalling: Vec<String>,
    /// What the count counts, once a plan or a tool session says.
    noun: Option<Noun>,
    done: usize,
    /// The items counted in `done`, so a repeated line is not counted twice.
    counted: Vec<String>,
    total: Option<usize>,
    rows: Vec<Row>,
}

/// What mise last said about an item it is working on.
struct Row {
    item: String,
    phase: String,
    transfer: Option<String>,
}

#[derive(Clone, Copy)]
enum Noun {
    Formulae,
    Casks,
    Items,
    Tools,
}

impl Noun {
    fn of(items: &[PlannedItem]) -> Noun {
        let all = |prefix: &str| items.iter().all(|item| item.name.starts_with(prefix));
        if all("brew:") {
            Noun::Formulae
        } else if all("brew-cask:") {
            Noun::Casks
        } else {
            Noun::Items
        }
    }

    fn word(self, count: usize) -> &'static str {
        let (one, many) = match self {
            Noun::Formulae => ("formula", "formulae"),
            Noun::Casks => ("cask", "casks"),
            Noun::Items => ("item", "items"),
            Noun::Tools => ("tool", "tools"),
        };
        if count == 1 { one } else { many }
    }
}

impl Board {
    /// Starts over for `step`: nothing it shows belongs to the step before.
    pub(super) fn begin(&mut self, step: Step<'_>) {
        *self = Board {
            step: self.step + 1,
            headline: headline(step),
            place: match step {
                Step::PlanInstall(place) | Step::Install(place) => {
                    Some((place.position, place.count))
                }
                _ => None,
            },
            ..Board::default()
        };
    }

    pub(super) fn plan(&mut self, items: &[PlannedItem]) {
        if items.iter().all(|item| item.action == PlannedAction::Uninstall) {
            self.uninstalling = items.iter().map(described).collect();
            return;
        }
        self.dependencies =
            items.iter().filter(|item| item.action == PlannedAction::InstallDependency).count();
        self.noun = Some(Noun::of(items));
        self.total = Some(items.len());
        self.planned = items.to_vec();
    }

    pub(super) fn apply(&mut self, report: Report) {
        match report {
            Report::Session { total } => {
                self.counted.clear();
                self.count_tools(0, total);
            }
            Report::Snapshot { done, total } | Report::Summary { done, total } => {
                self.count_tools(done, total);
            }
            Report::Row { item, phase, transfer } => self.show(item, phase, transfer),
            Report::Phase { item, phase } => {
                self.notice(&item);
                self.show(item, phase, None);
            }
            Report::Done { item, .. } => self.finish(item),
        }
    }

    pub(super) fn view(&self) -> View {
        let mut lines = Vec::new();
        if let Some((position, count)) = self.place.filter(|(_, count)| *count > 1) {
            let done = position - 1;
            lines.push(Line {
                label: String::new(),
                bar: Some((done, count)),
                text: format!("{done}/{count} packages"),
            });
        }
        lines.extend(self.tally());
        let shown = &self.rows[..self.rows.len().min(MOST_ROWS)];
        let labels: Vec<String> = shown.iter().map(|row| self.label(&row.item)).collect();
        let width = labels.iter().map(|label| label.chars().count()).max().unwrap_or(0);
        for (row, label) in shown.iter().zip(labels) {
            let text = match &row.transfer {
                Some(transfer) => format!("{}  {transfer}", row.phase),
                None => row.phase.clone(),
            };
            let bar = row.transfer.as_deref().and_then(fraction);
            lines.push(Line { label: format!("{label:<width$}"), bar, text });
        }
        if self.rows.len() > MOST_ROWS {
            let more = self.rows.len() - MOST_ROWS;
            lines.push(Line { label: String::new(), bar: None, text: format!("and {more} more") });
        }
        View { headline: self.headline.clone(), step: self.step, lines }
    }

    fn count_tools(&mut self, done: usize, total: usize) {
        self.noun = Some(Noun::Tools);
        self.done = done;
        self.total = Some(total);
        self.rows.clear();
    }

    fn show(&mut self, item: String, phase: String, transfer: Option<String>) {
        match self.rows.iter_mut().find(|row| row.item == item) {
            Some(row) => (row.phase, row.transfer) = (phase, transfer),
            None => self.rows.push(Row { item, phase, transfer }),
        }
    }

    /// Adds an item mise works on that the plan left out, so the total grows to include it.
    fn notice(&mut self, item: &str) {
        if self.planned.is_empty()
            || self.planned.iter().any(|planned| same_item(&planned.name, item))
        {
            return;
        }
        self.planned.push(PlannedItem {
            name: item.to_string(),
            version: None,
            action: PlannedAction::Install,
        });
        self.total = self.total.map(|total| total + 1);
    }

    fn finish(&mut self, item: String) {
        self.rows.retain(|row| row.item != item);
        if self.counted.contains(&item) {
            return;
        }
        self.notice(&item);
        self.counted.push(item);
        self.done += 1;
        self.total = self.total.map(|total| total.max(self.done));
    }

    fn tally(&self) -> Option<Line> {
        let text = |text: String| Line { label: String::new(), bar: None, text };
        if !self.uninstalling.is_empty() {
            let count = self.uninstalling.len();
            let noun = if count == 1 { "package" } else { "packages" };
            return Some(text(format!("{count} {noun}: {}", self.uninstalling.join(", "))));
        }
        match (self.total, self.noun) {
            (Some(total), Some(noun)) => {
                let mut tally = format!("{}/{total} {}", self.done, noun.word(total));
                match self.dependencies {
                    0 => {}
                    1 => tally.push_str(", 1 of them a dependency"),
                    many => tally.push_str(&format!(", {many} of them dependencies")),
                }
                Some(Line { label: String::new(), bar: Some((self.done, total)), text: tally })
            }
            _ if self.done > 0 => Some(text(format!("{} finished", self.done))),
            _ => None,
        }
    }

    /// The item's name, with the version its plan states.
    fn label(&self, item: &str) -> String {
        self.planned
            .iter()
            .find(|planned| same_item(&planned.name, item))
            .map_or_else(|| item.to_string(), described)
    }
}

fn headline(step: Step<'_>) -> String {
    let place = |position: usize, count: usize| {
        if count > 1 { format!(", package {position} of {count}") } else { String::new() }
    };
    match step {
        Step::UpdateMise => "Updating mise".into(),
        Step::ReadInstalled => "Reading what is installed".into(),
        Step::PlanInstall(package) => format!(
            "Asking mise what installing {} involves{}",
            package.package,
            place(package.position, package.count)
        ),
        Step::Install(package) => {
            format!("Installing {}{}", package.package, place(package.position, package.count))
        }
        Step::UpdatePackages => "Updating packages".into(),
        Step::UpdateTools => "Updating tools".into(),
        Step::InstallMissingPackages => "Installing missing packages".into(),
        Step::InstallMissingTools => "Installing missing tools".into(),
        Step::CheckPrune => "Checking that a prune would uninstall nothing else".into(),
        Step::Undeclare => "Taking the declarations out of your config".into(),
        Step::PlanPrune { manager } => format!("Finding {manager} packages no config declares"),
        Step::Uninstall { manager } => format!("Uninstalling {manager} packages"),
        Step::ReadResult => "Reading what is installed now".into(),
    }
}

fn described(item: &PlannedItem) -> String {
    match &item.version {
        Some(version) => format!("{} {version}", item.name),
        None => item.name.clone(),
    }
}

/// Whether two names are one item: equal, or the same manager and the same last part, so a
/// tap's `brew:owner/tap/tg` is the plan's `brew:tg`.
fn same_item(first: &str, second: &str) -> bool {
    let short = |name: &str| -> (String, String) {
        let (manager, rest) = name.split_once(':').unwrap_or(("", name));
        (manager.to_string(), rest.rsplit('/').next().unwrap_or(rest).to_string())
    };
    first == second || short(first) == short(second)
}

/// `17.5/53.1 MB · 6.5 MB/s` as 175 of 531: mise prints both figures in the total's unit.
fn fraction(transfer: &str) -> Option<(usize, usize)> {
    let (done, total) = transfer.split_whitespace().next()?.split_once('/')?;
    let (done, total) = (tenths(done)?, tenths(total)?);
    (total > 0).then_some((done.min(total), total))
}

/// `17.5` as 175, `40` as 400.
fn tenths(number: &str) -> Option<usize> {
    let (whole, tenth) = number.split_once('.').unwrap_or((number, "0"));
    if tenth.len() != 1 {
        return None;
    }
    Some(whole.parse::<usize>().ok()? * 10 + tenth.parse::<usize>().ok()?)
}

#[cfg(test)]
mod tests {
    use pkg_core::PackageStep;

    use super::*;
    use crate::progress::recognize::{Kind, Recognizer};

    fn feed(board: &mut Board, recording: &str) {
        let mut recognizer = Recognizer::default();
        for line in recording.lines() {
            match recognizer.recognize(line) {
                Kind::Transient(Some(report)) | Kind::Completion(report) => board.apply(report),
                Kind::Transient(None) | Kind::Other => {}
            }
        }
    }

    fn install(package: &str, position: usize, count: usize) -> Step<'_> {
        Step::Install(PackageStep { package, position, count })
    }

    fn planned(name: &str, version: &str, action: PlannedAction) -> PlannedItem {
        PlannedItem { name: name.into(), version: Some(version.into()), action }
    }

    /// The plan mise 2026.11.9 gave for `brew:wget` before `recorded/pour-wget.stderr`.
    fn wget_plan() -> Vec<PlannedItem> {
        let dependency = PlannedAction::InstallDependency;
        vec![
            planned("brew:ca-certificates", "2026-11-05", dependency),
            planned("brew:json-c", "0.19", dependency),
            planned("brew:libunistring", "1.5", dependency),
            planned("brew:gettext", "1.0", dependency),
            planned("brew:libidn2", "2.3.8", dependency),
            planned("brew:libpsl", "0.23.3", dependency),
            planned("brew:openssl@4", "4.1.0", dependency),
            planned("brew:wget", "1.25.0_2", PlannedAction::Install),
        ]
    }

    fn line(label: &str, bar: Option<(usize, usize)>, text: &str) -> Line {
        Line { label: label.into(), bar, text: text.into() }
    }

    #[test]
    fn a_planned_pour_counts_formulae_against_the_plan_and_shows_each_phase() {
        let mut board = Board::default();
        board.begin(install("brew:wget", 1, 2));
        board.plan(&wget_plan());
        let recording = include_str!("recorded/pour-wget.stderr");
        let downloads: String = recording.lines().take(9).map(|line| format!("{line}\n")).collect();
        feed(&mut board, &downloads);
        let view = board.view();
        assert_eq!(view.headline, "Installing brew:wget, package 1 of 2");
        assert_eq!(view.lines[0], line("", Some((0, 2)), "0/2 packages"));
        assert_eq!(view.lines[1], line("", Some((0, 8)), "0/8 formulae, 7 of them dependencies"));
        assert_eq!(
            view.lines[2],
            line(
                "brew:ca-certificates 2026-11-05",
                None,
                "download ca-certificates-2026-11-05.tar.gz"
            )
        );
        let wget = format!("{:<31}", "brew:wget 1.25.0_2");
        assert_eq!(view.lines[9], line(&wget, None, "download wget-1.25.0_2.tar.gz"));
        assert_eq!(view.lines.len(), 10);

        feed(&mut board, recording);
        let view = board.view();
        assert_eq!(view.lines[1], line("", Some((8, 8)), "8/8 formulae, 7 of them dependencies"));
        assert_eq!(view.lines.len(), 2, "every formula finished: {:?}", view.lines);
    }

    #[test]
    fn a_tool_session_shows_the_count_and_the_transfer_mise_last_reported() {
        let mut board = Board::default();
        board.begin(install("node", 1, 1));
        let recording: String = include_str!("recorded/tools-install.stderr")
            .lines()
            .take(4)
            .map(|line| format!("{line}\n"))
            .collect();
        feed(&mut board, &recording);
        let view = board.view();
        assert_eq!(view.headline, "Installing node");
        assert_eq!(
            view.lines,
            [
                line("", Some((0, 2)), "0/2 tools"),
                line("node@24.22.3", Some((175, 531)), "downloading  17.5/53.1 MB · 6.5 MB/s"),
                line("bun@1.4.5   ", Some((57, 398)), "downloading  5.7/39.8 MB · 2.7 MB/s"),
            ]
        );
    }

    #[test]
    fn an_item_the_plan_left_out_grows_the_total() {
        let mut board = Board::default();
        board.begin(install("brew:hello", 1, 1));
        board.plan(&[planned("brew:hello", "2.12.3", PlannedAction::Install)]);
        board.apply(Report::Phase { item: "brew:gettext".into(), phase: "link".into() });
        board.apply(Report::Done { item: "brew:gettext".into(), version: Some("1.0".into()) });
        board.apply(Report::Done { item: "brew:hello".into(), version: Some("2.12.3".into()) });
        board.apply(Report::Done { item: "brew:hello".into(), version: Some("2.12.3".into()) });
        assert_eq!(board.view().lines, [line("", Some((2, 2)), "2/2 formulae")]);
    }

    #[test]
    fn without_a_total_the_board_counts_what_finished() {
        let mut board = Board::default();
        board.begin(Step::InstallMissingPackages);
        feed(&mut board, include_str!("recorded/sync-packages.stderr"));
        let view = board.view();
        assert_eq!(view.headline, "Installing missing packages");
        assert_eq!(view.lines, [line("", None, "1 finished")]);
    }

    #[test]
    fn an_uninstall_lists_what_mise_is_about_to_uninstall() {
        let mut board = Board::default();
        board.begin(Step::Uninstall { manager: "brew" });
        board.plan(&[
            planned("brew:wget", "1.25.0_2", PlannedAction::Uninstall),
            planned("brew:libidn2", "2.3.8", PlannedAction::Uninstall),
        ]);
        let view = board.view();
        assert_eq!(view.headline, "Uninstalling brew packages");
        assert_eq!(
            view.lines,
            [line("", None, "2 packages: brew:wget 1.25.0_2, brew:libidn2 2.3.8")]
        );
    }

    #[test]
    fn a_new_step_starts_from_nothing_and_restarts_the_clock() {
        let mut board = Board::default();
        board.begin(install("brew:wget", 1, 2));
        board.plan(&wget_plan());
        board.apply(Report::Phase { item: "brew:wget".into(), phase: "link".into() });
        let first = board.view().step;
        board.begin(Step::ReadResult);
        let view = board.view();
        assert_eq!(view.headline, "Reading what is installed now");
        assert!(view.lines.is_empty(), "{:?}", view.lines);
        assert_ne!(view.step, first);
    }
}
