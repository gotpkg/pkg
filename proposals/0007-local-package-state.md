---
id: proposal.0007
title: Local Package State
authors: [Mark]
status: implemented
pull-request: https://github.com/gotpkg/pkg/pull/15
issues: []
supersedes: []
---

# Local Package State

## Summary

pkg tells mise to skip its network-bound cask check and reads casks from Homebrew's Caskroom itself. Every other package still comes from mise, which checks it locally. `pkg list` drops from 1–3 s to about 0.2 s and needs no network on macOS or Linux, apt included.

## Motivation

`pkg list` took long enough on 2026-11-21 that Mark expected a hang. Listing what is installed is reading files and printing them. It should answer at once, with no progress bar and no network request, for everything installed, whatever installed it: formulae, casks, App Store apps, tools, apt packages.

Measured that day on Mark's Mac (mise 2026.11.9, 123 declared bootstrap packages, 64 tools), with a wrapper around mise that timed each call:

| Command                            | Total       | `mise bootstrap packages status --json` | Everything else, pkg included |
| ---------------------------------- | ----------- | --------------------------------------- | ----------------------------- |
| `pkg list`                         | 0.94–2.95 s | 0.95–2.67 s                             | 0.03–0.06 s                   |
| `pkg list --manager tool`          | 0.06 s      | not run                                 | 0.06 s                        |
| `pkg info brew:wget`               | 1.85 s      | 1.75 s                                  | 0.10 s                        |
| `pkg install brew:hello --dry-run` | 1.76 s      | 1.67 s, its answer unused               | 0.09 s                        |
| `pkg sync --dry-run`               | 1.81 s      | 1.73 s                                  | 0.08 s                        |
| `pkg outdated`                     | 5.68 s      | 1.69 s, in parallel with the plan       | mise's 4.82 s upgrade plan    |

The same query took 3.4 s on 2026-11-17 (Beads `pkg-vpa`). Every bootstrap package command that reads state runs it, and `install`, `sync` and `update` run it twice: once before changing anything and once to build the receipt. `pkg install brew:hello` runs it before starting even though `brew:hello` is not declared, so the answer cannot mention it.

The query also fails when the network does. With `MISE_OFFLINE=1` it exits 1, and `pkg list` fails with `MISE_FAILED` (exit 6) naming a tap cask it could not fetch. A GitHub rate limit makes it exit 1 as well, and an agent's shell meets that limit quickly (Domain grounding). The only workaround today is `--manager tool`, which drops every bootstrap package from the answer.

## Domain grounding

Observed on 2026-11-21 on macOS arm64 with mise 2026.11.9, mas 7.0.0 and Mark's configuration, by timing each mise call pkg makes, by running mise with `MISE_DEBUG=1` and `MISE_TRACE=1`, and by reading mise's source at tag `v2026.11.9` and on `main` that day (2026.11.12 had been released). Source links are to `v2026.11.9`.

### Established context

- `mise bootstrap packages status --json` is the only mise command that reports bootstrap packages ([`collect_packages`](https://github.com/jdx/mise/blob/v2026.11.9/src/cli/bootstrap.rs#L5040-L5150)). It has no package or manager filter. Its JSON `state` is one of `installed`, `missing`, `version_mismatch`, `needs_repair`, `skipped`, `absent` and `unexpectedly_installed`.
- For every manager but one, the check asks a local database:
  - Homebrew formulae: the `<prefix>/opt/<name>` link and the keg it points to ([`linked_state`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/pour.rs#L113-L120)).
  - App Store apps: `mas list` ([`installed`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/mas.rs#L276)).
  - `macos-app:` entries: an inline declaration, which these entries must carry ([`resolve_cask_for`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/cask/mod.rs#L1452-L1465)).
  - apt: `dpkg-query` ([`installed`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/apt.rs#L536-L546)), plus `apt-cache policy`, which reads the package lists already on the machine, when the config declares apt sources ([`policy_origins`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/apt.rs#L113)).
  - The rest: dnf and zypper use `rpm -q`, pacman `pacman -Q`, flatpak `flatpak list` and snap `snap list`.
  - Observed for formulae, App Store apps and the macOS app; read from source for the others.
- The exception is `brew-cask:`. For each cask, mise first resolves the cask's current definition, then looks on disk ([`installed_with_options`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/cask/mod.rs#L1231-L1256), [`package_state`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/cask/state.rs#L624-L660)). It uses that definition for four things:
  - **Renamed casks:** it maps a renamed token to the name in the Caskroom.
  - **The app itself:** it checks that the app the current definition installs is the one on disk, so `apply` reinstalls a cask whose app changed.
  - **Platform:** it reports a cask unavailable on this macOS or architecture.
  - **Auto-updates:** it marks casks whose apps update themselves.

  All four serve reconciliation, deciding what `apply` must do. Answering what is installed needs none of them. mise uses one code path for both questions.
- Resolving a definition is where the time goes. An official cask comes from Homebrew's 19.5 MB `cask.json`, which mise trusts for 450 s and then revalidates: a 304 took 0.13 s, and a changed document added 0.4 s. mise allows that refresh up to 120 s ([`bulk.rs`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/bulk.rs#L104-L149)). A tap cask takes a chain of requests on every run: `raw.githubusercontent.com/<tap>/HEAD/api/cask/<token>.json`, then on a 404 `api.github.com/repos/<owner>/<tap>` and `.../commits/<branch>`, then a sandboxed Ruby evaluation ([`resolve_tap_source`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/tap.rs#L277-L300)). Two of Mark's three tap casks take the whole chain, costing 0.4–0.7 s and 4 GitHub API requests per run.
- mise's newest code still does this. Since 2026.11.11 (#15909), a cask that cannot be resolved becomes that entry's own `unresolved` state instead of failing the whole query.
- On disk, mise counts a cask as installed at version `V` when `<prefix>/Caskroom/<token>/` holds exactly one directory `V` other than `.metadata` and `.mise-*` ([`installed_versions`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system-packages/src/brew/cask/state.rs#L3-L35)). Applied to Mark's 41 casks, that rule alone gave mise's state and version for all 41.
- mise's `system_packages.managers` setting (`MISE_SYSTEM_PACKAGES_MANAGERS`, comma-separated) names the managers mise may use. Its documentation says status lists an excluded manager's packages as `skipped` ([settings](https://github.com/jdx/mise/blob/v2026.11.9/settings.toml#L4094-L4116)). The JSON keeps each excluded package's name, requested version and desired state, with `available: false` and the reason `excluded by the system_packages.managers setting` ([`collect_packages`](https://github.com/jdx/mise/blob/v2026.11.9/src/cli/bootstrap.rs#L5049-L5082)). Run with every manager but `brew-cask`, the status query on Mark's Mac:
  - took 0.14–0.15 s instead of 0.95–2.67 s;
  - made no network request;
  - gave identical answers for every other package;
  - listed the 41 casks with the same names, requested versions and desired states;
  - with `MISE_OFFLINE=1` added, still succeeded, in 0.17 s.

  The other calls `pkg list` makes took 0.01–0.03 s each, so `pkg list` would take about 0.2 s in all. That is a sum of measured calls, not a run of this design.
- An unprefixed `[bootstrap.packages]` key goes to the first available manager in `system_packages.prefer`, or by default to the first available of `apt`, `dnf`, `pacman`, `zypper`, `apk`, `brew`, `winget`, `scoop` and `choco` ([`resolve_unprefixed`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system/src/lib.rs#L350-L412)). It never goes to `brew-cask` unless `prefer` names it. Excluding a manager removes it from that choice ([`package_manager_is_enabled`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-system/src/lib.rs#L586-L592)). mise's list of built-in managers grows: `macports` appeared after 2026.11.9.
- `mise settings get system_packages.managers` and `... prefer` print the value, or exit 1 with "is not set". Each took about 10 ms.
- mise's Homebrew prefix is `MISE_SYSTEM_BREW_PREFIX` when set, else `/opt/homebrew` on macOS and `/home/linuxbrew/.linuxbrew` elsewhere ([`system_brew_prefix`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-util/src/env.rs#L1493-L1499)). `brew-cask:` is available on macOS and Linux.
- `MISE_PREFER_OFFLINE=1` does not avoid the network: it made the same 7 requests in the same time.
- Without the cask check, unauthenticated GitHub requests stop as well. Mark's config sets `github.credential_command = 'gh auth token ...'`, but mise runs it with its shims removed from `PATH` ([`credential_command_process`](https://github.com/jdx/mise/blob/v2026.11.9/crates/mise-util/src/tokens.rs#L138-L167)). So in a shell without `mise activate`, which is what agents get, every request goes out unauthenticated, at 60 an hour. A rate-limited request, simulated through `MISE_URL_REPLACEMENTS`, makes the query exit 1.

### Relevant constraints and principles

- mise stays the authority on what is declared: merging across files, `os` and `env` selectors, unprefixed keys, `names`, `state`, plugin managers. pkg reproduces none of it.
- The Caskroom layout (`<prefix>/Caskroom/<token>/<version>/`) is Homebrew's on-disk contract, which mise pours into. pkg reads directory names there and never mise's receipt files.
- pkg's library takes every location from `Settings` and its child-environment snapshot ([library contract](../crates/pkg-core/src/lib.rs)).
- Tests must never read the machine's real prefix. mise's own tests select one with `MISE_SYSTEM_BREW_PREFIX`, and so can pkg's.

### Quality bar

- On macOS and Linux, reading bootstrap package state makes no network request, for any config whose `system_packages.prefer` does not name `brew-cask`. Each manager costs only its local database query.
- On Mark's Mac, `pkg list` finishes in about 0.2 s, and `MISE_OFFLINE=1 pkg list` succeeds.
- `pkg list --output json` matches today's except in the cask divergences and the one JSON value listed under Compatibility.
- Nothing else in pkg's output, receipts, JSON schemas, errors or exit statuses changes.

### Remaining uncertainty

- Linux was not measured. Tangerine declares only `apt:` and `flatpak-user:` packages, so its query never reached the network and should already be fast. That is inferred from source, not timed.
- The checks for winget, scoop, choco, nix, apk and aur were not read. They are unchanged by this proposal either way.
- How often the cask divergences occur. None did among Mark's 41 casks.
- mise may change its default list for unprefixed keys, or how status reports an excluded manager. The second is documented behavior.

## Existing baseline

Every bootstrap package state pkg uses comes from [`Mise::package_states`](../crates/pkg-core/src/mise/queries.rs). It runs the status query and returns one `PackageState` per entry (`key`, `manager`, `requested`, `desired`, `state`, `installed`, `name`, `available`). Six places call it:

- [`list`](../crates/pkg-core/src/list.rs);
- [`info`](../crates/pkg-core/src/info.rs);
- [`install`](../crates/pkg-core/src/install.rs), in `States::load`, before and after;
- [`sync`](../crates/pkg-core/src/sync.rs), in `missing`, before and after;
- [`update`](../crates/pkg-core/src/update.rs), in `versions`, before and after;
- [`outdated::find`](../crates/pkg-core/src/outdated.rs), which `pkg update --dry-run` shares.

Preserve that type, its order (managers by name, then config order) and every field's meaning. They are what the commands turn into output and receipts. [`Mise::write_target`](../crates/pkg-core/src/mise/queries.rs) already reads a mise setting with `mise settings get` and treats "is not set" as unset. [`Declarations`](../crates/pkg-core/src/declarations.rs) already lists every `[bootstrap.packages]` key in each config file.

## Proposed solution

Keep asking mise, minus the one question it answers over the network. pkg runs the status query with `brew-cask` excluded, so mise still resolves every declaration and checks every other manager locally. pkg then fills in each cask from the Caskroom.

The local answer decides as well as reports. When it says a declared cask is installed, `pkg install` leaves it alone and `pkg sync` does not count it as missing.

```text
pkg list                       (Mark's Mac)
→ about 0.2 s instead of 1–3 s, the same packages, versions and states, no network request

MISE_OFFLINE=1 pkg list        (no network)
→ the same answer; today it fails with MISE_FAILED

pkg list                       (Tangerine, apt and flatpak only)
→ the same local query as today: no casks are declared, so nothing changes
```

## Detailed design

### Asking mise without casks

`Mise::package_states` reads `system_packages.managers` and `system_packages.prefer` with `mise settings get`, alongside the queries `Machine::load` runs. It excludes `brew-cask` when all of these hold:

- The host is macOS or Linux.
- Some config file declares a `brew-cask:` key.
- `managers` is unset or includes `brew-cask`.
- `prefer` is unset or does not include `brew-cask`.

To exclude it, pkg runs the status query with `MISE_SYSTEM_PACKAGES_MANAGERS` set to a comma-separated list without `brew-cask`. When `managers` is set, the list is `managers`. Otherwise it is the union of:

- every manager prefix of a `[bootstrap.packages]` key in the config files;
- mise's default list for unprefixed keys: `apt`, `dnf`, `pacman`, `zypper`, `apk`, `brew`, `winget`, `scoop` and `choco`;
- the entries of `prefer`.

A name that matches no manager is harmless. When the conditions do not hold, the query runs exactly as today.

If the answer reports any manager other than `brew-cask` as excluded by the `system_packages.managers` setting while `managers` is unset, pkg's list missed a manager. pkg then runs the query again without the variable and uses that answer.

### Casks from the Caskroom

The prefix is `MISE_SYSTEM_BREW_PREFIX` from the child environment when it is set and nonempty. Otherwise it is `/opt/homebrew` on macOS and `/home/linuxbrew/.linuxbrew` on Linux. A cask's token is its name after the last `/`, so `brew-cask:steipete/tap/codexbar` is `codexbar`. For each `brew-cask` entry in mise's answer:

- **Versions:** the cask's versions are the directories in `<prefix>/Caskroom/<token>/` other than `.metadata` and those starting with `.mise-`.
- **State:** exactly one version means installed at that version, and none or several means missing. A `requested` other than `latest` that differs from the installed version is `version_mismatch`. A package declared `absent` is `unexpectedly_installed` when installed and `absent` when missing, as mise reports them.
- **Fields:** `available` is `true`, and `requested`, `desired` and the order come from mise's answer.
- **Errors:** a missing directory is a missing cask. Any other error reading it fails the operation with `PRECONDITION_FAILED` naming the path, as an unreadable config file does today.

### Commands

The six callers keep calling `Mise::package_states`. Nothing they print or return changes, except as listed under Compatibility.

- `install` no longer reads state before it starts when no requested bootstrap package is declared yet, because nothing in that read could describe the package. It still reads state after installing, for the receipt.
- An installed version that mise reports as `""` becomes none, whichever source it comes from.

### Documentation and contract

- [`crates/pkg-core/src/lib.rs`](../crates/pkg-core/src/lib.rs), under Context and effects: state that an operation reading bootstrap package state may set `MISE_SYSTEM_PACKAGES_MANAGERS` for mise and read `<prefix>/Caskroom`. The prefix comes from `MISE_SYSTEM_BREW_PREFIX` in the child environment.
- [Packages](../crates/pkg/docs/packages.md): add a short section saying where installed state comes from and listing the cask divergences under Compatibility. `pkg docs search` finds it there.
- [Use pkg From Rust](../guides/rust-library.md): add that a host isolating a client from the real machine sets `MISE_SYSTEM_BREW_PREFIX` in the child environment.
- No registry entry, argument, output field or schema changes, so `crates/pkg/docs/cli.md` does not change.
- [VISION.md](../VISION.md), Design principles: the principle "mise decides, pkg translates" gains one clause, agreed with Mark on 2026-11-21. It now reads: "installation, version resolution and config writes go through mise's own commands; pkg edits a config file only to remove an entry mise cannot remove itself, and reads whether a Homebrew cask is installed from Homebrew's Caskroom, which mise answers only over the network." The vision changes when the work is accepted, as `completing-a-feature` does it, with a revision-history row linking this proposal.

### Tests

- **Library, Caskroom:** tests against a temporary prefix with one, none and two version directories, `.metadata`, a `.mise-` directory, a tap token, a version pin and an `absent` declaration.
- **Library, the exclusion list:** prefixes only; with `prefer`; with `managers` including and excluding `brew-cask`; no cask declared, so no variable; a missed manager, so the query runs again.
- **CLI:** the fixtures in [`tests/cli/support.rs`](../crates/pkg/tests/cli/support.rs) set `MISE_SYSTEM_BREW_PREFIX` inside the temporary home, so no test reads the machine's prefix. The fake mise records `MISE_SYSTEM_PACKAGES_MANAGERS` and answers casks as excluded when it leaves them out. The seeded casks exist in the temporary Caskroom. On Windows CI nothing is excluded, so the fake answer is used as today.

## Compatibility

For casks, pkg's answer can differ from mise's in these cases:

- **App deleted by hand** while its version directory remains: pkg says installed, mise says missing. `pkg install` reports it `unchanged` and `pkg sync` skips it, where today both have mise reinstall it. `mise bootstrap` or `mise bootstrap packages apply` still reinstalls it.
- **Changed app:** the cask's current definition installs a different app than the one on disk. pkg says installed, mise says missing.
- **Renamed cask** declared under its old token: pkg says missing, mise says installed. `pkg sync` then asks mise to apply, which changes nothing.
- **Unavailable cask** on this macOS version or architecture: pkg says missing, mise says `skipped`.
- **Several versions owned by Homebrew:** pkg says missing, mise fails the whole query.
- **Interrupted transaction:** after an interrupted mise cask transaction, pkg says installed and mise says missing.

Mark chose this over letting mise decide for `install` and `sync`, which would keep those two exact and slow.

One JSON value changes everywhere: a missing package's version, which mise reports as `""`, becomes `null` wherever pkg passes it on. That covers `installed` in `pkg list`, `from` in `pkg update` and `version` in a `pkg install` receipt. The schemas already allow `null`, and pkg's test fixtures already use it.

Everything else is unchanged, including configs that declare no cask. The `pkg-core` API does not change.

Proposal 0003 (progress display) shows a step while pkg reads installed state. That step remains and now ends almost at once.

## Implications on adoption

No setup and no migration. Reverting restores today's behavior with no data to undo.

When raising the mise version pkg supports, maintainers also check three things: mise's default list for unprefixed keys, how status reports an excluded manager, and its Caskroom rule.

## Scope

- In `pkg-core`: the exclusion list, the Caskroom reader, their use inside `Mise::package_states`, and the `""` mapping.
- `install` skipping its first state read when no requested bootstrap package is declared.
- The documentation changes listed under Detailed design, and a change file for the release notes.
- The tests listed under Detailed design.

### Out of scope

- **Commands that wait on mise's own network work.** `pkg remove` and `pkg prune` run `mise bootstrap packages prune --dry-run`, which resolves formula and cask definitions too (2.4 s for formulae, 1.5 s for casks). `pkg outdated` and `pkg update --dry-run` wait on mise's upgrade plan (3.3–4.8 s), and finding newer versions does need the network. `pkg search` waits on the App Store (0.15–1.3 s). Beads `pkg-1yf` records the mise side.
- **Changing mise.** See Future directions and Beads `pkg-1yf`.
- **Agents' GitHub authentication.** It belongs in the dotfiles or in mise (Beads `pkg-cas`).
- **mas running twice per query.** mas 7.0.0's JSON fails mise's parser, so mise runs `mas list` twice. It is part of the 0.14 s and is mise's to fix.

## Preview

- Repository shape: a CLI and the library it consumes.
- Artifact: the release build of this branch's `pkg`, run in place from `target/release/pkg` on Mark's Mac. The change is how long a person waits and whether pkg works offline.
- Exercise, timed, in text and `--output json`, beside the installed release:
  - `pkg list`, with `pkg list --output json` compared field by field against the installed release;
  - `MISE_OFFLINE=1 pkg list`;
  - `pkg info brew-cask:firefox`;
  - `pkg install brew:hello --dry-run`;
  - `pkg sync --dry-run`;
  - `pkg update --dry-run`.

  The exclusion rules get one run each against a disposable copy of the config selected with `MISE_CONFIG_DIR`: with an unprefixed key, with `system_packages.prefer` set, and with `system_packages.managers` set. No real install runs.
- Cleanup: delete the disposable config directory. Nothing is installed.

## Policies and decisions checked

No policy or decision records exist; `policies/` and `decisions/` hold only their READMEs. This proposal establishes neither. Checked against [VISION.md](../VISION.md):

- **mise as the single owner of installation and of the config, with no second source of truth:** honored. mise installs, writes, and resolves every declaration. pkg keeps nothing between runs and reads Homebrew's own directory.
- **"mise decides, pkg translates":** revised. mise still checks every manager but casks. For casks, pkg decides from the Caskroom whether one is installed, and `install` and `sync` act on that answer. The divergences under Compatibility are the cost. Mark agreed on 2026-11-21 that the principle should say so; the wording is under Detailed design.
- **Non-goal "Replacing mise's installers or keeping a database of its own":** honored.

## Future directions

- mise could answer the inventory question itself: check the Caskroom before resolving a cask's definition, and resolve it only for `apply` and `upgrade`. A way to exclude one manager by name, or a `--manager` filter on `status` like `prune` and `upgrade` have, would also let pkg drop the exclusion list. With the first change pkg could drop its Caskroom reader. Beads `pkg-1yf` holds the report for Mark to file.

## Alternatives considered

- **pkg reads declarations and installed state itself** for plain `brew:`, `brew-cask:`, `mas:` and `macos-app:` configs on Apple silicon, falling back to the full query otherwise. This was the previous draft. It reproduces mise's declaration rules, and every config outside the subset, Linux included, still waits on the network. This design covers every manager and config with far less of mise's logic in pkg.
- **List declarations only**, with no installed state for packages: fast, but `pkg list` loses the installed versions people read it for.
- **Change mise only, and wait:** keeps one source of truth, but `pkg list` stays at 1–3 s until mise ships it.
- **Let mise decide for `install` and `sync`**, reading locally only for output and receipts: exact for casks, but those two commands keep paying for the cask check. Mark chose speed.
- **`MISE_OFFLINE=1` with mise 2026.11.11 or later:** official casks would come from mise's cached index and tap casks would come back `unresolved` for pkg to fill in. The answer would then depend on the cache's age, and it needs a newer mise than Mark's. It was not measured.
- **Link mise's published crates** (`mise-config`, `mise-system`, `mise-system-packages` on crates.io) **or maintain a fork:**
  - The local checks are private to `mise-system-packages`.
  - The public cask check fetches the definition first.
  - The crates read process-wide settings, which `pkg-core`'s contract forbids.
  - A fork means about 620,000 lines of Rust to rebase across 13 releases in the first 21 days of November 2026.
  - A pkg without the mise executable is a vision change, not this proposal.
- **Cache the status answer:** adds state of pkg's own, and it goes stale as soon as anything installs outside pkg.
- **`MISE_PREFER_OFFLINE`:** it made the same requests in the same time.

## Open questions

None.
