---
id: proposal.0003
title: Progress Display for Package Changes
authors: [Mark]
status: implemented
pull-request: https://github.com/gotpkg/pkg/pull/3
issues: []
supersedes: []
---

# Progress Display for Package Changes

## Summary

On a terminal, `pkg install`, `update`, `sync`, `remove` and `prune` show a live indicatif display: the current step, the package and its place in the request, what mise is doing to each item, and a bar wherever a total is known. Piped and JSON runs do not change.

## Motivation

A person who runs `pkg install` in a terminal mostly watches nothing. Measured on 2026-11-17 with mise 2026.11.9:

- Before pkg prints anything, it asks mise what is installed. For bootstrap packages that query (`mise bootstrap packages status --json`) takes 3.4 s on Mark's Mac, and pkg runs it again after the install to build the receipt. `pkg install brew:hello --dry-run` prints its first character after 5.6 s.
- While a tool downloads, one update arrives every 3 s: a 53 MB Node download showed four in 12 s. mise lengthens that interval to as much as 60 s on long installs.
- While Homebrew formulae pour, the lines name phases only. `pkg install brew:wget` printed more than 60 lines for 8 formulae and not one size, rate or count. Nothing appears between a formula's `download` line and its `checksum` line, however large the bottle.
- Nothing says which package of a multi-package request is running or how many remain, or that the seven other formulae scrolling past are dependencies of the one that was asked for.
- `pkg remove brew:wget` was silent for 2.9 s and then printed one line.

The person cannot tell a slow download from a hang, and cannot tell how much is left. The only workaround is to run mise directly in a terminal, where it shows its own live display, and give up what pkg adds: the recorded declaration, the rollback of a failed install, and the receipt.

## Domain grounding

### Established context

Observations were made on 2026-11-17 on macOS arm64 with mise 2026.11.9, in a disposable home and a disposable Homebrew prefix, with read-only timings taken on Mark's Mac. Source references are to mise at tag `v2026.11.9`.

- mise picks its progress renderer from its own stderr ([`multi_progress_report.rs`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-ui/src/multi_progress_report.rs)). It redraws a live region only when stderr is a terminal and none of `quiet`, `verbose`, `raw`, `ci` or its agent detection applies; otherwise it appends plain lines. pkg pipes both of mise's streams ([`Mise::run`](../crates/pkg-core/src/mise/mod.rs)), so under pkg mise always appends plain lines. Both modes were observed.
- Tool installs in plain mode ([`text_install_progress.rs`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-ui/src/text_install_progress.rs)) print a header with the number of tools; a snapshot at an interval of one tenth of the elapsed time, clamped to 3–60 s; a completion line per tool; and a closing summary. A snapshot holds done/total and, for each active tool, its phase, its elapsed time and, during a download, bytes done, total and rate. Observed.
- Homebrew formulae in plain mode print one line per phase change per formula (`download <file>`, `checksum`, `extract`, `relocate`, `codesign`, `link`, `post-install`) and a `✓ <version>` line when the formula is done. The downloader reports bytes to the renderer, but the plain reporter does not print them (`VerboseReport` in [`progress_report.rs`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-ui/src/progress_report.rs)), and no line states how many formulae there are. Observed for formulae. Casks use the same reporter; that is read from source, not observed.
- The count exists in mise's plan. `mise bootstrap packages use --dry-run -g <package>` prints one `pour <name>/<version> (requested|dependency)`, `build <name>/<version> from source (…)` or `install cask <token>/<version>` line per item, and took 0.03–0.4 s on Mark's Mac. pkg already runs this query for `pkg install --dry-run` and already reads these lines for `pkg update --dry-run` ([`plans.rs`](../crates/pkg-core/src/mise/plans.rs)). The matching plans for update and sync took 2.2–5.8 s.
- `mise bootstrap packages prune` prints only a closing line (`brew: pruned 8 formulae`). Its `--dry-run` lists every removal, and pkg already runs it before each prune.
- None of these lines is a documented interface. mise's documentation describes no machine-readable progress, and the lines come from its terminal UI crate, so any release can reword them. This is an inference from the absence of a documented format.
- [indicatif](https://docs.rs/indicatif/0.18.6/indicatif/) 0.18.6 draws bars and spinners to stderr about 20 times per second, manages several at once, and prints lines above them. It [draws nothing](https://docs.rs/indicatif/0.18.6/indicatif/struct.ProgressDrawTarget.html#method.term) when its target is not a terminal or `TERM` is `dumb` (or unset, except on Windows). It does not hide the cursor or change terminal modes. It is MIT-licensed, needs Rust 1.85 (this workspace requires 1.100), and brings `console`, `unit-prefix`, `portable-atomic` and `unicode-width`. Its `in_memory` feature provides a terminal for tests.
- Cargo shows no progress bar when `CI` is set or `TERM` is `dumb`, because CI logs get mangled ([`progress_supported`](https://github.com/rust-lang/cargo/blob/master/crates/cargo-util-terminal/src/shell.rs)). mise reads `CI` for the same reason.

What pkg's stderr carries today, as observed (timestamps and some lines removed):

```text
mise by @jdx – installing 2 tools
mise █░░░░░░░░░░░░░░░ 0/2 · 3.0s
  node@24.22.3  downloading  3.0s  11.0/53.1 MB · 4.7 MB/s
  bun@1.4.4     downloading  3.0s  0.6/40.3 MB · 370 kB/s
mise ✓ node@24.22.3  10.8s  node-v24.22.3-darwin-arm64.tar.gz
mise ████████████████ 2/2 · installed 2 tools in 13.0s

mise brew:openssl@4  download openssl@4-4.1.0.tar.gz
mise brew:wget       download wget-1.25.0_2.tar.gz
mise brew:wget       checksum
mise brew:wget       extract
mise brew:wget       relocate
mise brew:wget       codesign
mise brew:wget       link
mise brew:wget     ✓ 1.25.0_2
mise brew: installed wget
```

### Relevant constraints and principles

- Progress is an observation, not a result. pkg's receipts come from reading mise's state after it exits ([Mutation safety](../.agents/skills/designing-human-first-clis/SKILL.md#mutation-safety)), and a display must not become a second source for them.
- A number between two reports is a guess. mise reports bytes every 3 s at best, so a display that keeps moving a byte count between reports is inventing data. Motion that shows the command is alive has to come from the clock.
- A bar is a fraction, so it needs a total, and a total that grows moves the bar backwards.
- A region that redraws and lines that append cannot both write to one terminal unless one of them goes through the other.
- Redrawn output is useless in a log, in CI and to a screen reader. Plain appended output has to stay reachable.
- pkg already reads mise's human-oriented output in one place, and its rule there is to recognize only the lines it needs and skip the rest, so a line a later mise adds is ignored rather than misread ([`plans.rs`](../crates/pkg-core/src/mise/plans.rs)). For a display the safe direction is the opposite one: a line pkg does not recognize must be shown, never dropped.
- pkg's operations live in the `pkg-core` library, which never writes to the terminal and hands its caller mise's output as raw text ([the library contract](../crates/pkg-core/src/lib.rs)). Whatever draws belongs in a frontend, and whatever a frontend cannot work out from raw text has to cross that boundary as typed data.

### Quality bar

- With the display on, a person can always tell which step pkg is on, which package it is working on and how many remain, what mise last said about each item, how long it has taken, and how much is left when mise has said so.
- No period longer than about a second passes with nothing changing on screen.
- Every figure shown is one mise reported or one pkg counted. Nothing is extrapolated.
- Nothing mise printed is lost. A line is shown in the display, left on stderr, or both, and a failed command leaves on stderr everything it leaves today.
- When mise rewords its output, the display loses detail and nothing else: the step, the package and the elapsed time still show, mise's lines scroll past as they do today, and receipts, errors and exit statuses are untouched.
- With the display off, pkg's bytes on both streams and the mise commands it runs are exactly today's.

### Remaining uncertainty

- Output for casks, App Store apps and non-Homebrew managers was read in mise's source, not observed. Lines pkg does not recognize are shown as they are, so the cost of a wrong reading is missing detail.
- `mise bootstrap packages upgrade` was not observed performing a real upgrade. Its source shares the pour pipeline observed for installs.
- `mise self-update`, which `pkg update` runs first when no package is named, was not observed. Whatever it prints is shown as it is.
- How often mise rewords these lines is unknown.
- indicatif was not exercised on a Windows terminal. CI builds and tests on Windows but has no terminal.

## Existing baseline

The closest thing that already produces the intended result is mise's own live display, which a person sees when they run mise directly in a terminal (observed on a pseudo-terminal): one row per item with its phase, bytes, estimated time and a bar, and a done/total count above them. It shows what is worth preserving: a row for every item mise is working on, dependencies included; a count against a total; mise's own phase words; and a mark that stays when an item finishes. Its byte counts for Homebrew cannot be reproduced from piped output. pkg shows that display in one place today: `pkg bootstrap` with text output in a terminal gives mise the terminal ([`bootstrap.rs`](../crates/pkg-core/src/mise/bootstrap.rs)), and there pkg captures nothing mise prints.

pkg's current behavior has two halves. In the library, [`Mise::run`](../crates/pkg-core/src/mise/mod.rs) reads mise's stdout and stderr, keeps the text, which the failure message reads, and hands each chunk to the operation's progress callback on the calling thread. In the CLI, [`execute`](../crates/pkg/src/commands/common.rs) writes each chunk to stderr as it arrives. Preserve the routing (everything from mise goes to stderr, and stdout carries only the result), the capture, the order within each stream, the callback's contract, and the behavior of every run whose stderr is not a terminal. The callback carries mise's output and warnings and nothing about the operation's own work, so nothing reports the silent intervals around mise's runs. They were never designed.

## Proposed solution

Keep running mise exactly as today, piped, and put a display between its output and the terminal. pkg knows things mise does not: which step of its own work is running, which package of the request this is, and, for an install, the plan mise just printed. mise knows what each item is doing. The library reports the first kind as typed progress events, beside the raw output it already delivers. The CLI recognizes mise's lines, combines the two, and draws with indicatif, so that redrawing, terminal width, rate limiting and Windows consoles are indicatif's concern.

The human selected this approach over handing the terminal to mise and over a pseudo-terminal relay, and selected the commands and the plan query described below; see Alternatives considered.

```text
pkg install brew:wget node, in a terminal
→ while pkg reads what is installed: the step and its elapsed time, where today there are 3 s of nothing
→ brew:wget, package 1 of 2: 8 formulae planned, 7 of them dependencies; each formula's current phase; a bar of formulae finished
→ node, package 2 of 2: the bytes, total and rate mise last reported, and a bar
→ afterwards: the display is gone, one line per finished formula and tool remains on stderr, and the receipt is on stdout as today

the same command with stderr redirected, with --output json, or with CI set
→ exactly today's bytes on both streams and exactly today's mise commands
```

## Detailed design

Four words are used precisely below. A **package** is what the person named. An **item** is one thing mise installs or removes: a formula, a cask or a tool, whether it was requested or is a dependency. A **step** is one stretch of pkg's own work, such as reading what is installed. A **plan** is mise's answer to a `--dry-run`.

### When the display is on

The display is on only when all of these hold. Otherwise pkg behaves exactly as it does today.

- The command is `install`, `update`, `sync`, `remove` or `prune` ([registry](../crates/pkg/src/registry.rs)), with or without `--dry-run`.
- The output format is text.
- pkg's stderr is a terminal that indicatif draws on: `TERM` is not `dumb` and, outside Windows, is set.
- `CI` is unset or empty.

There is no new argument, environment variable or setting. A person who wants plain lines in a terminal redirects stderr (`pkg install brew:wget 2>&1 | cat`) or sets `CI`, and then gets what they get today.

The decision is made once per run, in the `pkg` crate, from the output format and the process environment. Nothing else asks whether stderr is a terminal, and `pkg-core` never learns the answer.

### What the person can find out

While the display is on:

- **The step.** Every interval in which pkg waits on mise or the network belongs to a step, and the display names the current step with its elapsed time.
- **The package and its place.** When a request names more than one package, the display names the package being worked on, its position and the number of packages, with a bar of packages finished. A package that needs no work counts as finished at once.
- **The items.** Every item mise reports working on is shown with its name, its version once mise has stated it, and the last phase mise reported for it, until it finishes.
- **How much.** Where a total is known, the display shows the number finished, the total and a bar: items in an install plan, tools in a mise session (mise states done/total), and the bytes of a download when mise reports them, with the rate mise reported. Where no total is known it shows the number finished so far and no bar. For `remove` and `prune` it shows how many packages mise is about to uninstall and which, from the plan pkg already has; mise reports nothing per item while it uninstalls.
- **That it is alive.** An indicator moves several times a second, and elapsed times advance every second, whether or not mise prints anything.

Figures are the last ones mise reported. pkg does not advance bytes, rates or counts between reports and does not estimate time remaining.

The steps, in order:

| Command   | Steps                                                                                                                                                                  |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `install` | Read the config and what is installed. For each package that needs work: ask mise for its plan (bootstrap packages only), then install it. Read what is now installed. |
| `update`  | Update mise itself, when no package is named. Read installed versions. Update bootstrap packages. Update tools. Read installed versions again.                         |
| `sync`    | Find what is missing. Install missing bootstrap packages. Install missing tools. Find what is still missing.                                                           |
| `remove`  | Read the config and check that a prune would uninstall nothing else. Take the declarations out. For each manager: get the prune plan, then uninstall.                  |
| `prune`   | For each manager: find undeclared packages, then uninstall them.                                                                                                       |

A dry run shows the same steps without the ones that change something. For `pkg install --dry-run`, asking for the plan is the dry run itself.

### What crosses the library boundary

`pkg-core` draws nothing and recognizes nothing. It reports what only the operation knows, through the progress callback every operation already takes ([the library contract](../crates/pkg-core/src/lib.rs)). Two events join `Output` and `Warning` in [`Progress`](../crates/pkg-core/src/progress.rs):

- **A step begins.** The event names which stretch of work from the table above is starting. A stretch about one package carries the package, its position and the number of packages in the request. A stretch that uninstalls carries the manager. A step ends when the next one begins or the call returns, so all output delivered in between belongs to it.
- **A plan.** The items mise is about to install or uninstall in the current step: each item's name, its version when the plan states one, and, for an install, whether it was requested or is a dependency.

Both follow the callback's existing contract: they are delivered serially on the calling thread, before the method returns, and a consumer that ignores them loses nothing. `install`, `update`, `sync`, `remove` and `prune` report their steps on every call, dry runs included; no other operation reports any. `remove` and `prune` report the plan they already compute. `install` reports a plan only when its request asks for one, as described under The plan query.

Recognizing mise's lines stays out of the library. Its contract hands over mise's output as text, and turning that text into typed item progress there would present wording mise can change in any release as a stable interface.

### What happens to mise's lines

With the display on, mise's output still reaches the CLI through the progress callback, as it does today, and the CLI assembles it into lines. It sorts each line into one of three kinds:

| Kind               | Lines, as mise 2026.11.9 writes them                                                                                                                                                                                                                                     | Where it goes                                         |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------- |
| Transient progress | Session headers (`by @jdx – installing 2 tools`). Snapshots: the bar line and the indented rows that follow it. Closing summaries. A line for a bootstrap package (`brew:wget checksum`) whose text starts with one of the phase words listed under Established context. | Updates the display. Not left on stderr.              |
| Completion         | `✓ ripgrep@15.2.0  3.0s  ripgrep-15.2.0-aarch64-apple-darwin.tar.gz`, `⇢ jq@1  1ms · already installed`, `✗ no-such-tool@latest  0ms · failed: …`, `brew:wget ✓ 1.25.0_2`.                                                                                               | Updates the display and is left on stderr, unchanged. |
| Everything else    | Warnings, errors, the output of programs mise runs, and any line pkg does not recognize.                                                                                                                                                                                 | Left on stderr, unchanged.                            |

- Recognition works line by line, after the leading `mise` and any color codes are set aside. The only state it keeps is that indented rows belong to the snapshot line before them. It is conservative: a line that does not match exactly one known shape is "everything else".
- Lines left on stderr appear above the display, in the order they arrived on their stream. No order is promised between mise's two streams, as today.
- Recognition changes what the display shows and whether a line is left on stderr. It never changes what pkg runs next, the receipt, the failure or the exit status.
- When an operation fails, the CLI writes the transient lines it withheld during the failing step to stderr, in order, before the error. A failure therefore leaves on stderr every line it leaves today. The error's own text stays as it is today: `execute` already leaves mise's output out of it once that output has been written.
- The text the library keeps from mise for failure messages does not change.

Completion lines stay because they are the only record of which dependencies and which artifacts were installed; the receipt lists neither.

The shapes in this table have no current home. They belong in the rustdoc of the CLI code that recognizes them, with test fixtures recorded from mise 2026.11.9 the way this proposal's observations were made: real installs in a disposable home and Homebrew prefix, with mise's output piped.

### The plan query

The CLI asks for plans on its install request only when the display is on. [`InstallRequest`](../crates/pkg-core/src/requests.rs) gains a field for that, and every other caller leaves it off. When it is set, before the library installs a bootstrap package it asks mise for the plan, with the query its dry run already uses for that package ([`install.rs`](../crates/pkg-core/src/install.rs)): `bootstrap packages use --dry-run -g <package>` for one no config declares, and `bootstrap packages apply --dry-run <package>` for one that is declared and missing. From the plan it takes every item mise will install (the `pour`, `build` and `install cask` lines, dependencies included) and reports them as a plan event. [`upgrade_plan`](../crates/pkg-core/src/mise/plans.rs) reads the same lines today but keeps only requested formulae; an install plan keeps all of them.

- The plan serves the caller's display only. The library makes no decision from it and the receipt does not include it.
- A plan query that fails, or a plan with no line the library recognizes, is not an error and is not reported. The package is installed, and the display shows the count without a total.
- If mise then works on an item the plan did not list, the display's total grows to include it. The number finished never exceeds the total.
- No plan query runs for tools (mise states the count itself), for `update` or `sync` (their plans took 2.2–5.8 s on Mark's Mac), for `remove` or `prune` (they already have the plan), in a dry run (it is that query), or when the request does not ask.

### Starting and ending

- pkg may wait a moment before the display first appears, so that a command that finishes at once shows nothing.
- Before pkg writes its result, a warning or an error, the display is removed. What remains is the lines left on stderr and then pkg's own output, with nothing of the display between or after them.
- stdout carries only the result, as today.
- pkg adds no signal handling. indicatif neither hides the cursor nor changes terminal modes, so a command interrupted with Ctrl-C leaves a usable terminal. The last state drawn may remain on screen.

### Current sources and what changes in each

In `pkg-core`:

- [The library contract](../crates/pkg-core/src/lib.rs): its Progress section describes `Output` and `Warning`. Add the two events, when each operation reports them, and the plan request. Write this before the code; it is the specification of the boundary.
- [`progress.rs`](../crates/pkg-core/src/progress.rs) and [`requests.rs`](../crates/pkg-core/src/requests.rs): the two events, and the field on `InstallRequest`.
- [`install`](../crates/pkg-core/src/install.rs), [`update`](../crates/pkg-core/src/update.rs), [`sync`](../crates/pkg-core/src/sync.rs), [`remove`](../crates/pkg-core/src/remove.rs) and [`prune`](../crates/pkg-core/src/prune.rs): each reports its steps, `remove` and `prune` their plans, and `install` its plans when asked. Their mise commands, their order, their receipts and their errors do not change, apart from the plan query.
- [`plans.rs`](../crates/pkg-core/src/mise/plans.rs): add reading the items of an install plan.
- [`Mise::run`](../crates/pkg-core/src/mise/mod.rs) and the queries do not change.
- [Use pkg From Rust](../guides/rust-library.md): its worked example matches every `Progress` event and builds an `InstallRequest`. Update both, and say that the new events can be ignored.

In `pkg`:

- [`execute`](../crates/pkg/src/commands/common.rs): today its callback writes every `Output` chunk to stderr. For the five commands with the display on, the callback is the display instead. In every other case it stays as it is and ignores the new events, so the bytes it writes do not change.
- The registry descriptions of the five commands ([`install`](../crates/pkg/src/commands/install.rs), [`update`](../crates/pkg/src/commands/update.rs), [`sync`](../crates/pkg/src/commands/sync.rs), [`remove`](../crates/pkg/src/commands/remove.rs), [`prune`](../crates/pkg/src/commands/prune.rs)): `install`, `update` and `sync` end with "mise's progress goes to stderr." Replace that sentence, and add the same one to `remove` and `prune`, with one stating the contract: on a terminal pkg shows progress as a live display, and otherwise mise's progress lines go to stderr as they arrive. The introduction in [`reference.rs`](../crates/pkg/src/reference.rs) says the same thing for every command and changes with them. Regenerate the [CLI reference](../crates/pkg/docs/cli.md).
- [Packages](../crates/pkg/docs/packages.md): "Installing" and "Output for scripts and agents" each say that mise's progress goes to stderr. Revise both, and add a section that tells a reader what the display shows, when it is on, how to get plain lines, and its limits: Homebrew has no byte counts, and every figure is mise's last report.
- The website's [Packages](../site/app/content/docs/docs/packages.md) page tells the same readers how installing, removing and updating go. It gains the same account of the display. Its [Automation](../site/app/content/docs/docs/automation.md) page says mise's progress goes to stderr in both formats, which stays true.
- [`crates/pkg/Cargo.toml`](../crates/pkg/Cargo.toml): add `indicatif` 0.18, with a comment saying what it is for. `pkg-core` gains no dependency.
- New: a progress module in `crates/pkg/src/` with three separate parts. Assembling chunks into lines and recognizing them is pure and does no I/O. The display is the only code that names indicatif. The decision whether the display is on is the only code that looks at the terminal and the environment.

Elsewhere:

- [AGENTS.md](../AGENTS.md) ("mise's progress and warnings go to stderr"), [Designing Human-First CLIs](../.agents/skills/designing-human-first-clis/SKILL.md) ("Progress from long operations goes to stderr as it happens, so a person sees mise working") and [Writing Rust](../.agents/skills/writing-rust/SKILL.md) ("The CLI forwards upstream output to stderr"): revise each sentence to the same contract. Everything still goes to stderr.
- The layouts in [AGENTS.md](../AGENTS.md), [Writing Rust](../.agents/skills/writing-rust/SKILL.md) and [README.md](../README.md#layout) gain the new module. Writing Rust's rule that mise's text is parsed only in `mise/plans.rs` gains its one exception: progress lines, which the CLI recognizes for display and for nothing else.

## Compatibility

When the display is off (stderr is not a terminal, `--output json`, `CI` set, or a dumb terminal), nothing changes for a person or a script: not a byte on either stream, and not a mise command. The tests in [`tests/cli/`](../crates/pkg/tests/cli/) run pkg with piped streams and must pass unmodified; that is the proof.

When the display is on, stderr differs from today in two ways: transient progress lines no longer accumulate, and pkg runs one more read-only mise query for each bootstrap package it installs (0.03–0.4 s measured). stdout, receipts, JSON schemas, error codes, error messages and exit statuses do not change anywhere.

The binary gains a dependency and grows by its size, which was not measured.

For Rust consumers of `pkg-core`, `Progress` gains two events and `InstallRequest` one field. Code that matches every `Progress` event, or builds an `InstallRequest`, has to add an arm or the field before it compiles. The crate is unpublished, and its guide changes with it. A consumer that ignores the new events and leaves plans off gets today's behavior, including today's mise commands.

`pkg bootstrap` with text output in a terminal gives mise the terminal, so that mise and the programs it runs can ask for passwords and answers ([Setting up a machine](../crates/pkg/docs/setup.md#in-a-terminal-and-in-scripts)). A child that owns the terminal and a live display cannot share it. `bootstrap` and `setup` are not among this proposal's commands, so the two do not meet. Whether either gets a display is a later decision.

## Implications on adoption

Nothing to set up and nothing to migrate. A person who prefers today's output gets it per run by redirecting stderr or setting `CI`.

For maintainers, the recognized shapes are tied to mise's wording. Raising the mise version pkg supports means recording its output again and updating the fixtures. A mismatch costs detail in the display and breaks nothing.

## Scope

- In `pkg-core`, the step and plan events and the plan request. In the CLI, the recognition of mise's lines, the indicatif display, and their use by the five commands.
- The documentation changes listed under Detailed design, and a change file for the release notes.
- Tests in `pkg-core`: the order and thread of the new events, that no plan query runs unless the request asks, and that a failing plan query changes nothing else.
- Tests in the CLI: recognition against output recorded from mise 2026.11.9 for tool installs, Homebrew pours, an update, a sync, a remove and a failure; the display against indicatif's in-memory terminal; the existing binary tests, unmodified, as the proof that nothing changes with the display off; and runs of the binary under a pseudo-terminal on Unix proving that the display turns on, the plan query is issued, stdout holds only the result, and a failure leaves the withheld lines on stderr.
- No new guide in `guides/` for the display. [Packages](../crates/pkg/docs/packages.md) is the documentation a reader finds, in the repository and through `pkg docs search`, and a guide would repeat its new section. The Rust consumer guide changes as listed under Detailed design.

### Out of scope

- Byte counts, rates or time remaining for Homebrew formulae and casks. mise does not print them when piped, so they need mise to change or a different approach.
- Letting mise draw its own display, by handing it the terminal or through a pseudo-terminal. See Alternatives considered.
- `list`, `outdated`, `info` and `search`. The first three wait silently on the same slow query, but none of them changes software.
- `setup` and `bootstrap`. They install software too, but they get a machine ready rather than change packages, and `bootstrap` gives mise the terminal.
- `tap add` and `tap remove`. They change the config, not installed software, and finish at once.
- Plain step lines when the display is off, and progress events in `--output json`. Both would change output that scripts and logs read today.
- Plan queries for `update` and `sync`: 2.2–5.8 s of waiting before any work starts.
- Shortening the status queries themselves (Beads `pkg-vpa`), and letting mise prompt for a sudo password (Beads `pkg-4kn`). Both were found while grounding this proposal.
- An option or setting that forces the display on or off.
- Listing dependencies in receipts.

## Preview

- Repository shape: a CLI and the library it consumes. A person can exercise the library's new events only through the CLI; its tests cover them directly.
- Artifact: the installable executable built from the reviewed revision, delivered locally with its path in the review report. This repository is public, so the build is not uploaded.
- Reason: the change is what a person sees in a terminal over time. Only running it in one shows that.
- Exercise, in a real terminal, against a disposable home and a disposable Homebrew prefix so that real downloads and pours happen without touching the machine: a formula with dependencies; two tools; one request naming a package and a tool; something already installed; an update of a named tool held at an old version; a sync with something missing; a remove; a prune; an unknown package, for the failure path; and Ctrl-C in the middle of a download. Then the same commands with stderr redirected, with `--output json` and with `CI` set, compared with a build of `main`. On the real machine, the five commands with `--dry-run` only.
- `pkg update` with no package named first runs `mise self-update` on the mise executable itself, which no disposable home contains. The exercise names the tool, and the real machine sees only `pkg update --dry-run`, which does not update mise.
- The disposable prefix uses `MISE_SYSTEM_BREW_PREFIX`, an undocumented override mise keeps for its own tests ([`env.rs`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-util/src/env.rs)); this proposal's observations used it. If a later mise drops it, exercising Homebrew needs a real install that the human approves first.
- Cleanup: delete the disposable home and prefix. Nothing is installed on the machine.

## Policies and decisions checked

No policy or decision records exist; `policies/` and `decisions/` hold only their READMEs. This proposal establishes neither. Checked against [VISION.md](../VISION.md):

- **Human-first, agent-complete** — the display is for the person at a terminal, and the JSON contract is untouched.
- **mise decides, pkg translates** — mise still installs and still reports. pkg presents what mise reported and adds only what pkg itself knows. The plan query is a read mise already offers and pkg already uses.
- **Keep mise the single owner, so pkg adds no state** — the display keeps nothing after the run.
- **No prompts** — none are added.
- **The same operations through `pkg-core`** — steps and plans are typed progress that any frontend receives. Drawing and the reading of mise's wording stay in the CLI, so the library still writes nothing to the terminal.

## Future directions

- Machine-readable progress from mise would replace the recognition of human-oriented lines and could carry byte counts for Homebrew. It needs a change in mise.
- Other consumers of `pkg-core`, such as an app, may want item-level progress as typed events. Recognition could then move behind the library boundary; here it stays in the CLI, where its one consumer is.
- The read commands wait on the same slow query in silence and could show the same step display.
- Plain step lines for logs, when the display is off.
- Plan queries for `update` and `sync`, if mise's plans for them become cheap.

## Alternatives considered

- **Hand the terminal to mise, and use indicatif only for pkg's own steps.** mise's display has byte counts, estimated time and a bar for formulae and casks as well as tools (observed), and pkg would recognize nothing. But on a terminal pkg could no longer read mise's error line for its own error sentence, as it cannot for `pkg bootstrap` today; mise would switch to interactive sudo, which can ask for a password where it fails fast today; and the five requests would each need the terminal option that only `bootstrap` has. The human chose one display owned by pkg across its own steps and mise's.
- **Recognize mise's lines inside `pkg-core` and report item progress as typed events.** Every frontend would get phases and byte counts without reading text. But the library would be presenting wording mise can change in any release as an interface, and the CLI is the only consumer today. It stays possible later; see Future directions.
- **Run mise inside a pseudo-terminal and relay it.** This keeps the capture and gets mise's display, at the cost of platform-specific code or a dependency on Unix and on Windows, forwarding the window size and signals, and stripping escape sequences to find error lines. It also leaves no room for pkg's own display while mise's is up.
- **No extra queries, or a plan before every command.** Without the plan, Homebrew installs show a count as formulae appear and no bar. With a plan everywhere, `update` and `sync` wait 2.2–5.8 s before starting. The human chose the plan for `install` only.
- **Only `install`, or only `install`, `update` and `sync`.** The human chose all five commands that changed installed software when the choice was made. `setup` and `bootstrap` arrived later and are out of scope.
- **Leave every mise line on stderr and draw the display below them.** Nothing would be withheld, which is the simplest rule. But one formula with dependencies prints more than 60 phase lines that say what the display is already showing, and they scroll the lines worth keeping out of view.
- **An option or environment variable for the display.** A person can already turn it off for a run, and the registry gains an argument only when a command needs one. It can be added when someone needs to force the display.
- **A spinner without a dependency.** The human asked for indicatif, and redrawing several lines, terminal width, rate limiting and Windows consoles are exactly what it handles.

## Open questions

None. During discovery on 2026-11-17 the human chose the approach, the commands and the plan query, and on 2026-11-19 approved this proposal for implementation.
