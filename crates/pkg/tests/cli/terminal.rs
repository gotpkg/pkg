//! The live display: pkg's stderr on a pseudo-terminal, as in a person's terminal, and stdout a
//! pipe.

use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::process::Stdio;

use indicatif::{InMemoryTerm, TermLike};

use crate::support::{Fixture, MAC_CONFIG};

const APPLY_HELLO: &str = include_str!("../../src/progress/recorded/apply-hello.stderr");
const FAIL_TOOL: &str = include_str!("../../src/progress/recorded/fail-tool-version.stderr");

/// What one run on a terminal produced: stdout, everything written to the terminal, and what the
/// terminal shows once pkg exits.
struct TerminalRun {
    status: i32,
    stdout: String,
    written: String,
    screen: String,
}

/// Runs pkg with stderr on a 40-row, 120-column pseudo-terminal and `TERM` set.
fn run_on_terminal(
    fixture: &Fixture,
    args: &[&str],
    env: &[(&str, Option<&OsStr>)],
) -> TerminalRun {
    let (mut controller, device) = open_terminal();
    let mut command = fixture.command(args, env);
    command.env("TERM", "xterm-256color").stdout(Stdio::piped()).stderr(Stdio::from(device));
    let mut child = command.spawn().expect("pkg runs");
    // The terminal reports its end only once no process holds the device open.
    drop(command);
    let reader = std::thread::spawn(move || {
        let mut written = Vec::new();
        // The end of the terminal arrives as an error on Linux and as the end of input on macOS.
        let _ = controller.read_to_end(&mut written);
        written
    });
    let mut stdout = String::new();
    child.stdout.take().expect("piped stdout").read_to_string(&mut stdout).expect("stdout");
    let status = child.wait().expect("pkg exits").code().unwrap_or(-1);
    let written = String::from_utf8_lossy(&reader.join().expect("the reader")).into_owned();
    let terminal = InMemoryTerm::new(40, 120);
    terminal.write_str(&written).expect("the in-memory terminal takes text");
    TerminalRun { status, stdout, written, screen: terminal.contents() }
}

fn open_terminal() -> (File, OwnedFd) {
    let (mut controller, mut device) = (0, 0);
    let size = libc::winsize { ws_row: 40, ws_col: 120, ws_xpixel: 0, ws_ypixel: 0 };
    // SAFETY: openpty stores two new descriptors in the integers it is given and only reads the
    // size; the name and the terminal settings may be null.
    let opened = unsafe {
        libc::openpty(
            &mut controller,
            &mut device,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::from_ref(&size).cast_mut(),
        )
    };
    assert_eq!(opened, 0, "openpty: {}", io::Error::last_os_error());
    // SAFETY: openpty succeeded, so both are open descriptors that nothing else owns.
    let (controller, device) =
        unsafe { (File::from_raw_fd(controller), OwnedFd::from_raw_fd(device)) };
    for descriptor in [controller.as_raw_fd(), device.as_raw_fd()] {
        // SAFETY: the descriptor is open; marking it close-on-exec keeps it out of pkg and of
        // the fake mise, so the terminal ends when pkg does.
        unsafe { libc::fcntl(descriptor, libc::F_SETFD, libc::FD_CLOEXEC) };
    }
    (controller, device)
}

/// A slow Homebrew install that prints what mise 2026.11.9 printed pouring `brew:hello`.
fn slow_hello(fixture: &Fixture) {
    fixture
        .mise(&["bootstrap", "packages", "use", "--yes", "-g", "brew:hello"])
        .declares("brew:hello")
        .slowly()
        .stderr(APPLY_HELLO);
}

#[test]
fn on_a_terminal_install_asks_for_the_plan_draws_and_leaves_only_the_lines_worth_keeping() {
    let fixture = Fixture::seeded();
    slow_hello(&fixture);
    let run = run_on_terminal(&fixture, &["install", "brew:hello"], &[]);
    assert_eq!(run.status, 0, "{}", run.written);
    assert_eq!(
        run.stdout,
        "Installed brew:hello, declared in ~/.dotfiles/mise/config.macos.toml\n"
    );
    assert!(run.written.contains("Installing brew:hello"), "{}", run.written);
    assert_eq!(run.screen, "mise brew:hello    ✓ 2.12.3\nmise brew: installed hello");
    let calls = fixture.calls();
    let plan =
        calls.iter().position(|call| call == "bootstrap packages use --dry-run -g brew:hello");
    let install =
        calls.iter().position(|call| call == "bootstrap packages use --yes -g brew:hello");
    assert_eq!(plan.map(|plan| plan + 1), install, "{calls:?}");
}

#[test]
fn a_failure_on_a_terminal_leaves_every_line_mise_printed_before_the_error() {
    let fixture = Fixture::seeded();
    fixture.mise(&["use", "-g", "node@0.0.999"]).slowly().stderr(FAIL_TOOL).status(1);
    let run = run_on_terminal(&fixture, &["install", "node@0.0.999"], &[]);
    assert_eq!(run.status, 6, "{}", run.screen);
    assert!(run.stdout.is_empty(), "{}", run.stdout);
    let rows: Vec<&str> = run.screen.lines().collect();
    let position = |text: &str| {
        rows.iter().position(|row| row.contains(text)).unwrap_or_else(|| {
            panic!("`{text}` is not on the terminal:\n{}", run.screen);
        })
    };
    let failed = position("mise ✗ node@0.0.999  746ms · failed:");
    let error_lines = position("mise ERROR Run with --verbose");
    let header = position("mise by @jdx – installing 1 tool");
    let summary = position("1/1 · installed 0 tools · 1 failed in 748ms");
    let error = position("error: mise could not install node");
    assert!(failed < error_lines && error_lines < header && header < summary && summary < error);
    assert!(!run.screen.contains("Installing node"), "the display stayed: {}", run.screen);
    assert_eq!(fixture.read_dotfile("config.macos.toml"), MAC_CONFIG);
}

#[test]
fn ci_or_json_on_a_terminal_gives_plain_lines_and_no_plan_query() {
    for (args, env) in [
        (&["install", "brew:hello"][..], &[("CI", Some(OsStr::new("true")))][..]),
        (&["install", "brew:hello", "--output", "json"][..], &[][..]),
    ] {
        let fixture = Fixture::seeded();
        slow_hello(&fixture);
        let run = run_on_terminal(&fixture, args, env);
        assert_eq!(run.status, 0, "{}", run.written);
        let expected = APPLY_HELLO.trim_end().replace('\n', "\r\n");
        assert!(run.written.contains(&expected), "{args:?}: {}", run.written);
        assert!(!fixture.calls().iter().any(|call| call.contains("--dry-run")), "{args:?}");
    }
}
