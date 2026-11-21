---
id: proposal.0007
title: Local Package State
authors: [Mark]
status: draft
pull-request: https://github.com/gotpkg/pkg/pull/15
issues: []
supersedes: []
---

# Local Package State

## Summary

On an Apple silicon Mac, pkg reads which Homebrew formulae, casks, App Store apps and macOS apps are declared and installed from the config files and the disk, instead of waiting on mise's network-bound status query. `pkg list` answers in about 0.1 s. `info`, `install`, `sync`, `update` and `outdated` stop waiting on that query too.

## Motivation

`pkg list` took long enough on 2026-11-21 that Mark expected a hang. Listing is reading a file, parsing it and printing it, so it should be instant. Measured that day on Mark's Mac (mise 2026.11.9, 123 declared bootstrap packages, 64 tools), with a wrapper around mise that timed each call:

| Command                            | Total       | `mise bootstrap packages status --json` | Everything else, pkg included |
| ---------------------------------- | ----------- | --------------------------------------- | ----------------------------- |
| `pkg list`                         | 0.94–2.95 s | 0.95–2.67 s                             | 0.03–0.06 s                   |
| `pkg list --manager tool`          | 0.06 s      | not run                                 | 0.06 s                        |
| `pkg info brew:wget`               | 1.85 s      | 1.75 s                                  | 0.10 s                        |
| `pkg install brew:hello --dry-run` | 1.76 s      | 1.67 s, its answer unused               | 0.09 s                        |
| `pkg sync --dry-run`               | 1.81 s      | 1.73 s                                  | 0.08 s                        |
| `pkg outdated`                     | 5.68 s      | 1.69 s, in parallel with the plan       | mise's 4.82 s upgrade plan    |

The same query took 3.4 s on 2026-11-17 (Beads `pkg-vpa`). Every bootstrap package command that reads state runs it, and `install`, `sync` and `update` run it twice: once before changing anything and once to build the receipt. A `pkg install` of one formula waits on it for 2–7 s however small the formula. `pkg install brew:hello` asks before it starts even though `brew:hello` is not declared, so the answer cannot mention it.

The query also fails outright when the network does. With `MISE_OFFLINE=1` it exits 1, and `pkg list` fails with `MISE_FAILED` (exit 6) naming a tap cask it could not fetch. A GitHub rate limit makes the query exit 1 as well, and an agent's shell meets that limit quickly (Domain grounding). The only workaround today is `--manager tool`, which drops every bootstrap package from the answer.

## Domain grounding

Observed on 2026-11-21 on macOS arm64 with mise 2026.11.9, mas 7.0.0 and Mark's configuration, by timing each mise call pkg makes, by running mise with `MISE_DEBUG=1` and `MISE_TRACE=1`, and by reading mise's source at tag `v2026.11.9`. Sources below link to that tag.

### Established context

- `mise bootstrap packages status --json` is the only command that reports bootstrap packages, and it checks each one against its declaration in full ([`collect_packages`](https://github.com/jdx/mise/blob/v2026.11.9/src/cli/bootstrap.rs#L5040-L5150)). It takes no package or manager filter (`mise bootstrap packages status --help`). Its JSON `state` is one of `installed`, `missing`, `version_mismatch`, `needs_repair`, `skipped`, `absent` and `unexpectedly_installed`.
- For a Homebrew formula the check is local. mise reads `<prefix>/opt/<name>` and the keg it points to, and reports the keg's version ([`installed`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/mod.rs#L436-L452), [`linked_state`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/pour.rs#L113-L120), [`linked_package_state`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/mod.rs#L525-L543)). A version pin that differs from the keg, allowing for a `_<revision>` suffix, is `version_mismatch`. A missing `var/homebrew/linked` record beside a valid `opt` link is `needs_repair`.
- For a cask, mise first resolves the cask's metadata, then looks on disk ([`installed_with_options`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/cask/mod.rs#L1231-L1256)). The disk answers alone when Homebrew's `.metadata` sits in `<prefix>/Caskroom/<token>/`: the one version directory there is the installed version ([`package_state`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/cask/state.rs#L624-L660)). Otherwise mise reads its own receipt in that version directory, checks that the receipt's targets still exist, and uses the metadata to check that the declared app still belongs to the cask ([`installed_cask_version_in`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/cask/state.rs#L581-L605)). `macos-app:` packages follow the same rules under `<mise state directory>/macos-apps/` instead of the Caskroom ([`cask_state_root_in`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/cask/state.rs#L2025-L2030)).
- The metadata is where the time goes. An official cask comes from Homebrew's 19.5 MB `cask.json`, which mise trusts for 450 s and then revalidates: a 304 took 0.13 s, and a changed document added 0.4 s to download and index. mise allows that refresh up to 120 s before it gives up ([`bulk.rs`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/bulk.rs#L104-L149)). A tap cask is a chain of requests on every run: `raw.githubusercontent.com/<tap>/HEAD/api/cask/<token>.json`, then on a 404 `api.github.com/repos/<owner>/<tap>` and `.../commits/<branch>`, then a sandboxed Ruby evaluation ([`resolve_tap_source`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/tap.rs#L277-L300)). Mark declares three tap casks. Two of them take the whole chain, so each status run spends 0.4–0.7 s and 4 GitHub API requests on them.
- For an App Store app, mise runs `mas list --json`. If that does not parse, it runs `mas list` and parses the text ([`mas_list`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/mas.rs#L204-L245)). With mas 7.0.0 the JSON does not parse ("an app object without an ID and version"), so every status run starts mas twice, 0.21 s in all. mas is available when the host is macOS and `mas` is on the search path ([`is_available`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/mas.rs#L256-L258)).
- mise's Homebrew prefix is `MISE_SYSTEM_BREW_PREFIX` when set, else `/opt/homebrew` on macOS and `/home/linuxbrew/.linuxbrew` elsewhere ([`system_brew_prefix`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-util/src/env.rs#L1493-L1499)). Brew formulae are available only on arm64 macOS and on x86_64 and arm64 Linux.
- Declarations merge across the global config files: mise reads them from most global to most local, a later file's value replaces an earlier one's, and a key keeps the place where it first appeared ([`merge_package_configs`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system/src/lib.rs#L1136-L1149)). `mise config ls --json` lists the same files most local first (observed). An entry is a version string or a table ([`PackageTomlConfig`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-config/src/packages.rs#L9-L70)) whose keys include `os` and `env` selectors, `state = "absent"`, per-manager `names` for unprefixed keys, `appdir` for casks, and `url`, `sha256` and `artifact` for an inline `macos-app:`. Unprefixed keys, plugin managers and the `system_packages.managers` setting change which manager takes an entry.
- No mise command returns the merged declarations without also running the full check. `MISE_PREFER_OFFLINE=1` does not avoid the network: it made the same 7 requests in the same time. `MISE_OFFLINE=1` makes the query fail on the first tap cask.
- A rate-limited GitHub API also makes the query exit 1. That was simulated by routing `api.github.com` to a local server answering 403 with `x-ratelimit-remaining: 0`, through `MISE_URL_REPLACEMENTS`. Unauthenticated, the limit is 60 requests an hour. Mark's config sets `github.credential_command = 'gh auth token ...'`, but mise runs it with its shims removed from `PATH` ([`credential_command_process`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-util/src/tokens.rs#L138-L167)). In a shell that has shims and no `mise activate`, as agents' shells do, `gh` is not found and every request is unauthenticated: 4 per status query and about 14 per `mise bootstrap packages prune --dry-run`.
- A read-only script that applied the disk checks above to the 123 packages mise reported took 94 ms, `mas list` included, and agreed with mise's state and version for all 123. It took the list of packages from mise's answer, so it tested installed state only.

### Relevant constraints and principles

- The Homebrew prefix layout (`opt/` links into `Cellar/<name>/<version>`, `Caskroom/<token>/<version>/`) is Homebrew's long-standing on-disk contract, which mise pours into. mise's own receipts and its `macos-apps/` directory are mise's private format. pkg may look for a version directory there but must not read the receipt files.
- pkg's library takes every location from `Settings` and its child-environment snapshot and never reads the process environment ([library contract](../crates/pkg-core/src/lib.rs)). [`Settings::mise_data_dir`](../crates/pkg-core/src/settings.rs) already derives a mise directory from that snapshot the way mise does.
- Anything pkg reads for itself must agree with what mise would report, or pkg must not answer from it. A config pkg does not fully understand goes to mise.
- Tests must never read the machine's real prefix. mise's tests select a prefix with `MISE_SYSTEM_BREW_PREFIX`, and so can pkg's.

### Quality bar

- On a config made only of what the local path understands, reading bootstrap package state takes no more than about 0.2 s and makes no network request. `pkg list` and `pkg sync --dry-run` then finish within that. `info` and `install --dry-run` add only the catalog request or mise plan they make today.
- For every package the local path covers, `pkg list --output json` matches today's output except in the divergences and the one JSON value listed under Compatibility.
- A config outside the local path gives today's answer, by today's query, unchanged.
- Nothing else in pkg's output, receipts, JSON schemas, errors or exit statuses changes.

### Remaining uncertainty

- How often the listed divergences occur. None did on Mark's Mac: all 123 packages agreed.
- Future mise releases may change the prefix or state layout, or what counts as installed, without notice. pkg's answer would then drift from mise's until pkg follows.
- Intel Macs and Linux were not examined. They keep today's query.

## Existing baseline

Every bootstrap package state pkg uses comes from [`Mise::package_states`](../crates/pkg-core/src/mise/queries.rs), which runs the status query and returns one `PackageState` per entry (`key`, `manager`, `requested`, `desired`, `state`, `installed`, `name`, `available`). Six places call it: [`list`](../crates/pkg-core/src/list.rs), [`info`](../crates/pkg-core/src/info.rs), [`install`](../crates/pkg-core/src/install.rs) (`States::load`, before and after), [`sync`](../crates/pkg-core/src/sync.rs) (`missing`, before and after), [`update`](../crates/pkg-core/src/update.rs) (`versions`, before and after) and [`outdated::find`](../crates/pkg-core/src/outdated.rs), which `pkg update --dry-run` shares. Preserve that type, its ordering (managers by name, then config order) and every field's meaning. They are what the commands already turn into output and receipts.

[`Declarations`](../crates/pkg-core/src/declarations.rs) already parses each config file mise loads with `toml_edit` and records which file declares which key. It has no merged values. Tools show what to aim for: `mise ls --global --json` reports declared tools and their installed state in about 25 ms, and every command that needs only tools is already fast.

The status query as the source of truth is incidental. pkg needs installed state, and that query happens to be the only mise command that reports it.

## Proposed solution

pkg reads bootstrap package state itself when it fully understands the config, and asks mise when it does not. On an Apple silicon Mac whose config declares only `brew:`, `brew-cask:`, `mas:` and `macos-app:` packages in their plain forms, pkg merges the declarations from the config files and looks on disk for each one: the formula's `opt` link, the cask's Caskroom directory, the macOS app's directory and `mas list`. Anything else, such as an `apt:` entry, an `os` selector or an unprefixed key, sends that call to the status query exactly as today.

The local answer decides as well as reports. When it says a declared package is installed, `pkg install` leaves it alone and `pkg sync` does not count it as missing, without asking mise.

```text
pkg list                              (Mark's Mac)
→ 0.1 s instead of 1–3 s, the same packages, versions and states, no network request

pkg install brew:hello                (not declared)
→ no state query before mise installs it; the receipt reads the version from disk

pkg list                              (a config with an apt: entry, or on Linux)
→ today's status query and today's answer
```

## Detailed design

### When pkg reads state itself

pkg takes the local path for one operation when all of these hold, and otherwise calls `mise bootstrap packages status --json` as today for that operation:

- The host is macOS on arm64.
- Every `[bootstrap.packages]` key in the merged declarations starts with `brew:`, `brew-cask:`, `mas:` or `macos-app:`.
- Every value is a version string or a table whose keys are only `version` and `adopt`, plus `url`, `sha256` and `artifact` for `macos-app:`. Any other key (`os`, `env`, `state`, `names`, `appdir`, or one pkg does not know) sends the operation to mise.
- mise's `system_packages.managers` setting is unset, as `mise settings get system_packages.managers` reports.
- If the config declares any `mas:` package, `mas` is found on the child environment's `PATH`.
- Reading the disk raises no error other than a path not existing, and no cask or macOS app directory holds more than one version (mise treats that as an error that needs repair).

The test covers the whole merged config, not one package: a single entry outside it sends the whole operation to mise, because the status query cannot be narrowed. Falling back is silent. It is today's behavior, and it prints nothing new.

### Declarations

Declarations merge as mise merges them. pkg reads `[bootstrap.packages]` from the files `mise config ls --json` lists, in reverse order (most global first). A later file's value replaces an earlier one's, and each key keeps the position where it first appeared. For each entry:

- `key` is the declared key, such as `brew:wget` or `brew:owner/tap/name`; `manager` is its prefix.
- `requested` is the string, or the table's `version`, or `latest` when the table has none.
- `desired` is `present`. The local path accepts no `state` key.
- `available` is `true`.

[`Declarations`](../crates/pkg-core/src/declarations.rs) keeps reporting which file declares a key; it gains the merged entries with their values.

### Installed state

The Homebrew prefix is `MISE_SYSTEM_BREW_PREFIX` from the child environment when it is set and nonempty, else `/opt/homebrew`. mise's state directory is `MISE_STATE_DIR`, else `$XDG_STATE_HOME/mise`, else `<home>/.local/state/mise`, all from the child environment, as [`Settings::mise_data_dir`](../crates/pkg-core/src/settings.rs) resolves the data directory. The name a package is looked up by is the part of its name after the last `/`, so `brew:owner/tap/tg` is `tg`.

- `brew:` is installed when `<prefix>/opt/<name>` resolves to a directory `<prefix>/Cellar/<name>/<version>`. The installed version is `<version>`. Otherwise it is missing.
- `brew-cask:` is installed when `<prefix>/Caskroom/<name>/` holds one directory whose name does not start with `.`. The installed version is that directory's name. With none, it is missing.
- `macos-app:` follows the same rule in `<state directory>/macos-apps/<name>/`.
- `mas:` runs `mas list --json` with the child environment, and if that fails or does not parse, `mas list`, parsing either as mise does. An app with the package's ID is installed at the version mas reports, and `name` is the app's name from the same listing. Otherwise it is missing. pkg runs mas at most once per operation, and only when a `mas:` package is declared.

`state` is `installed` or `missing`. It is `version_mismatch` when `requested` is not `latest` and differs from the installed version. For `brew:` a version that only adds `_<revision>` to the request still matches. `installed` holds the version whenever one was found and is none (JSON `null`) otherwise. The status query reports a missing package's version as an empty string, which reaches `pkg list --output json` today as `"installed": ""`. [`Mise::package_states`](../crates/pkg-core/src/mise/queries.rs) maps it to none, so both sources agree.

### Commands

All six callers named under Existing baseline get their `PackageState` list from one function that picks the source. Nothing they print or return changes.

- `install` no longer reads state before it starts when no requested bootstrap package is declared yet. Nothing in that read could describe the package. It still reads state after installing, for the receipt.
- `outdated::find` (`pkg outdated`, `pkg update --dry-run`) no longer runs the status query beside mise's upgrade plan when the local path applies.
- The other reads are unchanged in when they happen. They just get faster.

### Documentation and contract

- [`crates/pkg-core/src/lib.rs`](../crates/pkg-core/src/lib.rs), Context and effects: state that on arm64 macOS an operation may read bootstrap package state from the config files, the Homebrew prefix, mise's state directory and `mas list` rather than from mise. Name the child-environment variables that select them: `MISE_SYSTEM_BREW_PREFIX`, `MISE_STATE_DIR`, `XDG_STATE_HOME` and `PATH`. Add that an operation may run `mas` as well as mise.
- [Packages](../crates/pkg/docs/packages.md): add a short section saying where pkg reads installed state, when it asks mise instead, and the divergences under Compatibility. `pkg docs search` finds it there.
- [Use pkg From Rust](../guides/rust-library.md): the line saying later calls reload installed state still holds. Add that a host isolating a client from the real machine sets `MISE_SYSTEM_BREW_PREFIX` and `MISE_STATE_DIR` in the child environment.
- No registry entry, argument, output field or schema changes, so `crates/pkg/docs/cli.md` does not change.

### Tests

- Library tests of the local reader against a temporary prefix and state directory: a formula linked, unlinked and pinned; a cask with one, none and two version directories; a tap formula and cask; a macOS app; a merge across three files with an override; and each condition that sends the operation to mise.
- A fake `mas` that prints the JSON form, and one whose JSON fails, so the text form is used.
- The CLI fixtures in [`tests/cli/support.rs`](../crates/pkg/tests/cli/support.rs) set `MISE_SYSTEM_BREW_PREFIX` and `MISE_STATE_DIR` inside the temporary home, so no test reads the machine's prefix. Seeded packages exist on disk there, not only in the fake status answer, and the fake mise records whether the status query ran.
- Tests that need the status query, such as a config with an `apt:` entry, keep using the fake answer, and on Windows CI every test does.

## Compatibility

Where the local path applies, its answer can differ from mise's in these cases, each of which needs metadata or mise's private receipts:

- An app deleted by hand while its cask's version directory remains: pkg says installed, mise says missing. `pkg install` reports it `unchanged` and `pkg sync` skips it, where today both have mise reinstall it. `mise bootstrap` or `mise bootstrap packages apply` still reinstalls it.
- A cask whose app moved to a different path in a newer cask definition: pkg says installed, mise says missing.
- A formula whose Homebrew link records need repair: pkg goes by the `opt` link alone and says installed or missing, where mise says `needs_repair`.
- A cask whose definition is not available on this Mac: pkg says missing, mise says `skipped`.
- An interrupted mise cask transaction: pkg reports the version directory, mise says missing.

Mark chose this over letting mise decide for `install` and `sync`, which would keep those two commands exact and slow.

One JSON value changes on every path: a missing package's version, which mise reports as `""`, becomes `null` wherever pkg passes it on (`installed` in `pkg list`, `from` in `pkg update`, `version` in a `pkg install` receipt). The schemas already allow `null`, and pkg's own test fixtures already use it.

Everything else is unchanged: output, receipts, JSON schemas, errors and exit statuses, and every config outside the local path. The `pkg-core` API does not change. The library now runs `mas` itself on the local path, and its documentation says so.

Proposal 0003 (progress display) shows a step while pkg reads installed state. That step still happens, and on the local path it ends almost at once.

## Implications on adoption

No setup and no migration. Reverting this change restores today's behavior with no data to undo.

For maintainers: raising the mise version pkg supports now includes checking that mise's rules for installed state, the layouts listed above and `mas list`'s formats still hold.

## Scope

- The local reader and the merged declarations in `pkg-core`, the one function that picks the source, and its use by the six callers.
- `install` skipping its first state read when no requested bootstrap package is declared.
- The documentation changes listed under Detailed design, and a change file for the release notes.
- The tests listed under Detailed design.

### Out of scope

- Reading state locally on Intel Macs and on Linux, and for `apt:` and the other managers. Mark's Linux machine declares `apt:` packages, so it keeps today's query. A reader for each would follow the same pattern.
- The `os`, `env`, `state`, `names` and `appdir` keys, unprefixed keys and plugin managers. Each would mean reproducing more of mise's resolution.
- The commands whose slowness is mise's own work: `pkg remove` and `pkg prune` run `mise bootstrap packages prune --dry-run` (2.4 s for formulae, 1.5 s for casks); `pkg outdated` and `pkg update --dry-run` wait on mise's upgrade plan (3.3–4.8 s); `pkg search` waits on the App Store (0.15–1.3 s).
- Changing mise. The mise changes that would make its own query fast are under Future directions and in Beads `pkg-1yf`.
- The GitHub authentication of agents' shells, which belongs in the dotfiles or in mise (Beads `pkg-cas`).

## Preview

- Repository shape: a CLI and the library it consumes.
- Artifact: the release build of this branch's `pkg`, run in place from `target/release/pkg` on Mark's Mac. The change is what a person sees and how long they wait.
- Exercise: `pkg list`, `pkg info brew:wget`, `pkg info mas:<an installed app>`, `pkg install brew:hello --dry-run`, `pkg sync --dry-run`, `pkg update --dry-run` and `pkg outdated`, timed, in text and in `--output json`, beside the installed release. `pkg list --output json` is compared field by field with the installed release. The fallback is exercised by a run whose `MISE_CONFIG_DIR` points at a disposable copy of the config with one `apt:` entry added. No real install runs.
- Cleanup: delete the disposable config directory. Nothing is installed.

## Policies and decisions checked

No policy or decision records exist; `policies/` and `decisions/` hold only their READMEs. This proposal establishes neither. Checked against [VISION.md](../VISION.md):

- Keep mise the single owner of installation and of the config, with no state of pkg's own and no second source of truth: honored. mise still installs and writes every declaration, and pkg keeps nothing between runs. It reads the same disk mise reads.
- mise decides, pkg translates: this proposal departs from it. pkg now decides that a declared package is already installed, and `install` and `sync` skip mise on that answer. The divergences under Compatibility are the cost, and Mark chose it for speed. If that departure stands, the principle's wording needs to allow pkg to read installed state itself. See Open questions.
- Non-goal "Replacing mise's installers or keeping a database of its own": honored. pkg installs nothing itself and stores nothing.

## Future directions

- mise could make its own query fast. Its cask check fetches metadata before it looks at the Caskroom, though the Caskroom alone answers for casks Homebrew's metadata covers. A tap's default branch is fetched from `api.github.com` on every run with no cache. `status` could also accept package and manager filters. Without the requests and Ruby evaluations, the trace of a status run on Mark's Mac leaves about 0.3 s, an estimate rather than a measurement. pkg could then drop its reader.
- mise publishes its internals to crates.io with each release: `mise-config`, `mise-system`, `mise-system-packages`, `mise-util` and `mise-settings`, at 2026.11.12 on 2026-11-21. pkg could parse declarations with `mise-config`'s own types, so a new key fails to parse and falls back to mise rather than being misread. That would add a dependency pinned to a release train of a version every day or two.
- Readers for Linux (Linuxbrew and `apt:` through `dpkg-query`) would make Mark's Linux machine fast too.

## Alternatives considered

- List declarations only, with no installed state for packages: fast, but `pkg list` would lose the installed versions people read it for, and `info`, `install` and `sync` would still need the query.
- Change mise only, and wait: keeps one source of truth, but `pkg list` stays at 1–3 s until mise ships it, and mise is not pkg's to change.
- Let mise decide for `install` and `sync`, and read locally only for output and receipts: exact, but those two commands keep paying for the query. Mark chose speed.
- Link mise's published crates and call its own state checks: the checks that need no network are private to `mise-system-packages`. Its public cask check fetches metadata first, so it is exactly as slow. The crates also read process-wide settings and environment, which `pkg-core`'s contract forbids, and their on-disk formats must match the mise binary the person runs.
- Maintain a fork of mise under the gotpkg organization that exposes those internals: about 620,000 lines of Rust to keep rebased on a project that shipped 13 releases in the first 21 days of November 2026. A pkg that no longer needs the mise executable also contradicts the vision's "mise decides, pkg translates" and its non-goal of replacing mise's installers. That decision belongs in a vision change, not here.
- Cache the status answer on disk: adds the state of pkg's own that the vision rules out, and goes stale as soon as anything installs outside pkg.
- `MISE_PREFER_OFFLINE` or `MISE_OFFLINE`: the first made the same requests in the same time, and the second made the query fail.
- Run the cheap queries alongside the status query: saves about 25 ms of 1–3 s.

## Open questions

- [NEEDS CLARIFICATION: Should VISION.md's "mise decides, pkg translates" principle be revised to allow pkg to read installed state itself, or should this proposal change so that mise still decides for `install` and `sync`?]
