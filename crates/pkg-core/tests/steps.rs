#![cfg(unix)]

//! The steps and plans operations report through the progress callback.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::thread::ThreadId;

use pkg_core::{
    Client, InstallReceipt, InstallRequest, Mode, PlannedAction, Progress, PruneRequest,
    RemoveRequest, Settings, Step, SyncRequest, UpdateRequest,
};
use tempfile::TempDir;

/// A fake mise that logs every call to `$HOME/calls` and answers by `case "$*"` in `answers`.
fn fixture(answers: &str) -> (TempDir, Settings) {
    let root = TempDir::new().unwrap();
    let executable = root.path().join("mise");
    let script = format!(
        "#!/bin/sh\nshift 2\nprintf '%s\\n' \"$*\" >> \"$HOME/calls\"\ncase \"$*\" in\n{answers}\n\
         'backends ls') printf 'npm';;\n\
         'settings get write_targets.'*) printf 'is not set' >&2; exit 1;;\n\
         esac\n"
    );
    fs::write(&executable, script).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let settings = Settings::new(root.path().to_path_buf(), executable.into_os_string());
    (root, settings)
}

fn calls(root: &TempDir) -> Vec<String> {
    fs::read_to_string(root.path().join("calls")).unwrap().lines().map(str::to_string).collect()
}

/// Every event, in order, as one line each; consecutive output is joined.
struct Recorder {
    thread: ThreadId,
    events: Vec<String>,
}

impl Recorder {
    fn new() -> Recorder {
        Recorder { thread: std::thread::current().id(), events: Vec::new() }
    }

    fn observe(&mut self, event: Progress<'_>) {
        assert_eq!(std::thread::current().id(), self.thread, "progress left the calling thread");
        let entry = match event {
            Progress::Step(step) => format!("step {}", describe(step)),
            Progress::Plan(items) => {
                let items: Vec<String> = items
                    .iter()
                    .map(|item| {
                        let version = item.version.as_deref().unwrap_or("?");
                        let action = match item.action {
                            PlannedAction::Install => "requested",
                            PlannedAction::InstallDependency => "dependency",
                            PlannedAction::Uninstall => "uninstall",
                        };
                        format!("{}@{version} {action}", item.name)
                    })
                    .collect();
                format!("plan {}", items.join(", "))
            }
            Progress::Output { text, .. } => {
                if let Some(last) =
                    self.events.last_mut().filter(|last| last.starts_with("output "))
                {
                    last.push_str(text);
                    return;
                }
                format!("output {text}")
            }
            Progress::Warning(message) => format!("warning {message}"),
        };
        self.events.push(entry);
    }

    fn events(&self) -> Vec<String> {
        self.events.iter().map(|event| event.trim_end().to_string()).collect()
    }
}

fn describe(step: Step<'_>) -> String {
    match step {
        Step::PlanInstall(package) => {
            format!("PlanInstall {} {}/{}", package.package, package.position, package.count)
        }
        Step::Install(package) => {
            format!("Install {} {}/{}", package.package, package.position, package.count)
        }
        Step::PlanPrune { manager } => format!("PlanPrune {manager}"),
        Step::Uninstall { manager } => format!("Uninstall {manager}"),
        other => format!("{other:?}"),
    }
}

/// A Homebrew formula with a dependency, then a tool; neither is declared or installed.
const INSTALL: &str = r#"
 'config ls --json') printf '[]';;
 'bootstrap packages status --json') printf '{}';;
 'ls --global --json') printf '{}';;
 'bootstrap packages use --dry-run -g brew:wget')
    printf '~/.config/mise/config.toml: "brew:wget" = "latest"\npour libidn2/2.3.8 (dependency)\npour wget/1.25.0_2 (requested)\n';;
 'use -g --dry-run ripgrep@latest') printf 'would install ripgrep\n';;
 'bootstrap packages use --yes -g brew:wget') printf 'brew:wget ✓ 1.25.0_2\n' >&2;;
 'use -g ripgrep@latest') printf 'installed ripgrep\n' >&2;;"#;

/// Installs `brew:wget` and `ripgrep`: the receipt, one line per package with its config relative
/// to the home, and every event.
fn install(answers: &str, mode: Mode, report_plans: bool) -> (TempDir, Vec<String>, Vec<String>) {
    let (root, settings) = fixture(answers);
    let mut client = Client::new(settings).unwrap();
    let mut recorder = Recorder::new();
    let request =
        InstallRequest { packages: vec!["brew:wget".into(), "ripgrep".into()], mode, report_plans };
    let outcome = client.install(request, &mut |event| recorder.observe(event)).unwrap();
    let receipt = receipt(&outcome.result, &root);
    (root, receipt, recorder.events())
}

fn receipt(receipt: &InstallReceipt, root: &TempDir) -> Vec<String> {
    let mut lines = vec![format!("{:?} {:?}", receipt.status, receipt.plan)];
    lines.extend(receipt.packages.iter().map(|item| {
        let config = item.config.strip_prefix(root.path()).unwrap_or(&item.config);
        let (kind, action) = (item.kind, item.action);
        format!("{} {kind:?} {action:?} {:?} {}", item.package, item.version, config.display())
    }));
    lines
}

#[test]
fn install_reports_each_step_and_the_plan_it_asked_for_before_the_install() {
    let (root, _, events) = install(INSTALL, Mode::Apply, true);
    assert_eq!(
        events,
        [
            "step ReadInstalled",
            "step PlanInstall brew:wget 1/2",
            "step Install brew:wget 1/2",
            "plan brew:libidn2@2.3.8 dependency, brew:wget@1.25.0_2 requested",
            "output brew:wget ✓ 1.25.0_2",
            "step Install ripgrep 2/2",
            "output installed ripgrep",
            "step ReadResult",
        ]
    );
    let calls = calls(&root);
    let plan =
        calls.iter().position(|call| call == "bootstrap packages use --dry-run -g brew:wget");
    let install = calls.iter().position(|call| call == "bootstrap packages use --yes -g brew:wget");
    assert_eq!(plan.map(|plan| plan + 1), install, "{calls:?}");
    assert!(!calls.iter().any(|call| call.contains("--dry-run") && call.contains("ripgrep")));
}

#[test]
fn install_runs_no_plan_query_unless_the_request_asks() {
    let (root, _, events) = install(INSTALL, Mode::Apply, false);
    assert_eq!(
        events,
        [
            "step ReadInstalled",
            "step Install brew:wget 1/2",
            "output brew:wget ✓ 1.25.0_2",
            "step Install ripgrep 2/2",
            "output installed ripgrep",
            "step ReadResult",
        ]
    );
    assert!(!calls(&root).iter().any(|call| call.contains("--dry-run")), "{:?}", calls(&root));
}

#[test]
fn a_plan_query_that_fails_or_says_nothing_changes_nothing_else() {
    let (_, expected_receipt, mut expected_events) = install(INSTALL, Mode::Apply, false);
    expected_events.insert(1, "step PlanInstall brew:wget 1/2".into());
    for plan in [
        "printf 'mise ERROR cannot reach the formula API\\n' >&2; exit 1",
        "printf 'mise brew: all formulae already poured\\n'",
    ] {
        let answers = format!(
            " 'bootstrap packages use --dry-run -g brew:wget') {plan};;{}",
            INSTALL.replace("'bootstrap packages use --dry-run -g brew:wget')", "'unused')")
        );
        let (_, receipt, events) = install(&answers, Mode::Apply, true);
        assert_eq!(receipt, expected_receipt);
        assert_eq!(events, expected_events);
    }
}

#[test]
fn an_install_dry_run_reports_asking_for_each_plan_and_runs_nothing_more() {
    let (root, _, events) = install(INSTALL, Mode::DryRun, true);
    assert_eq!(
        events,
        ["step ReadInstalled", "step PlanInstall brew:wget 1/2", "step PlanInstall ripgrep 2/2",]
    );
    let dry_runs = calls(&root).iter().filter(|call| call.contains("--dry-run")).count();
    assert_eq!(dry_runs, 2, "{:?}", calls(&root));
}

#[test]
fn update_reports_mise_packages_tools_and_the_reread() {
    let (_root, settings) = fixture(
        r#"
 'config ls --json') printf '[]';;
 'version --json') printf '{"version":"2026.11.9 macos-arm64 (2026-11-16)","latest":"2026.11.9"}';;
 'self-update --yes') printf 'mise is up to date\n' >&2;;
 'bootstrap packages status --json') printf '{}';;
 'ls --global --json') printf '{}';;
 'bootstrap packages upgrade --yes') printf 'brew: already up to date\n' >&2;;
 'upgrade --global') printf 'All tools are up to date\n' >&2;;"#,
    );
    let mut client = Client::new(settings).unwrap();
    let mut recorder = Recorder::new();
    let request = UpdateRequest { packages: Vec::new(), mode: Mode::Apply };
    client.update(request, &mut |event| recorder.observe(event)).unwrap();
    assert_eq!(
        recorder.events(),
        [
            "step UpdateMise",
            "output mise is up to date",
            "step ReadInstalled",
            "step UpdatePackages",
            "output brew: already up to date",
            "step UpdateTools",
            "output All tools are up to date",
            "step ReadResult",
        ]
    );
}

#[test]
fn sync_reports_the_missing_packages_and_tools_it_installs() {
    let (_root, settings) = fixture(
        r#"
 'config ls --json') printf '[]';;
 'bootstrap packages status --json')
    if [ -f "$HOME/applied" ]; then state=installed; else state=missing; fi
    printf '{"brew":{"available":true,"packages":[{"package":"wget","state":"%s"}]}}' "$state";;
 'ls --global --json')
    if [ -f "$HOME/installed" ]; then installed=true; else installed=false; fi
    printf '{"jq":[{"version":"1.8.2","installed":%s,"active":true}]}' "$installed";;
 'bootstrap packages apply --yes') touch "$HOME/applied";;
 'install') touch "$HOME/installed";;"#,
    );
    let mut client = Client::new(settings).unwrap();
    let mut recorder = Recorder::new();
    let outcome =
        client.sync(SyncRequest { mode: Mode::Apply }, &mut |event| recorder.observe(event));
    assert_eq!(outcome.unwrap().result.installed, ["brew:wget", "jq"]);
    assert_eq!(
        recorder.events(),
        [
            "step ReadInstalled",
            "step InstallMissingPackages",
            "step InstallMissingTools",
            "step ReadResult",
        ]
    );
}

#[test]
fn remove_reports_the_check_the_undeclaring_and_each_prune_with_its_plan() {
    let (root, settings) = fixture(
        r#"
 'config ls --json') printf '[{"path":"%s/.config/mise/config.toml"}]' "$HOME";;
 'bootstrap packages prune --dry-run --manager brew')
    if [ -f "$HOME/checked" ]; then
        printf 'remove brew:wget@1.25.0_2\nremove brew:libidn2@2.3.8\n'
    else
        touch "$HOME/checked"
    fi;;
 'bootstrap packages prune --yes --manager brew') printf 'brew: pruned 2 formulae\n' >&2;;"#,
    );
    let config = root.path().join(".config/mise/config.toml");
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(&config, "[bootstrap.packages]\n\"brew:wget\" = \"latest\"\n").unwrap();
    let mut client = Client::new(settings).unwrap();
    let mut recorder = Recorder::new();
    let request = RemoveRequest { packages: vec!["brew:wget".into()], mode: Mode::Apply };
    client.remove(request, &mut |event| recorder.observe(event)).unwrap();
    assert_eq!(
        recorder.events(),
        [
            "step CheckPrune",
            "step Undeclare",
            "step PlanPrune brew",
            "step Uninstall brew",
            "plan brew:wget@1.25.0_2 uninstall, brew:libidn2@2.3.8 uninstall",
            "output brew: pruned 2 formulae",
        ]
    );
}

#[test]
fn prune_reports_each_manager_and_uninstalls_only_after_a_plan() {
    let (_root, settings) = fixture(
        r#"
 'config ls --json') printf '[]';;
 'bootstrap packages prune --dry-run --manager brew') printf 'remove brew:unused@1\n';;
 'bootstrap packages prune --yes --manager brew') printf 'brew: pruned 1 formula\n' >&2;;
 'bootstrap packages prune --dry-run --manager brew-cask') printf 'mise brew-cask: nothing to prune\n';;"#,
    );
    let mut client = Client::new(settings).unwrap();
    let mut recorder = Recorder::new();
    let request = PruneRequest { mode: Mode::Apply, force: false };
    client.prune(request, &mut |event| recorder.observe(event)).unwrap();
    assert_eq!(
        recorder.events(),
        [
            "step PlanPrune brew",
            "step Uninstall brew",
            "plan brew:unused@1 uninstall",
            "output brew: pruned 1 formula",
            "step PlanPrune brew-cask",
        ]
    );
}

#[test]
fn dry_runs_report_only_the_steps_that_change_nothing() {
    let (_root, settings) = fixture(
        r#"
 'config ls --json') printf '[]';;
 'bootstrap packages prune --dry-run --manager brew') printf 'remove brew:unused@1\n';;
 'bootstrap packages prune --dry-run --manager brew-cask') printf '';;"#,
    );
    let mut client = Client::new(settings).unwrap();
    let mut recorder = Recorder::new();
    let request = PruneRequest { mode: Mode::DryRun, force: false };
    client.prune(request, &mut |event| recorder.observe(event)).unwrap();
    assert_eq!(recorder.events(), ["step PlanPrune brew", "step PlanPrune brew-cask"]);
}
