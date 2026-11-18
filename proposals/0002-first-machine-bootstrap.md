---
id: proposal.0002
title: First-Machine Setup and Bootstrap
authors: [Mark]
status: draft
pull-request: https://github.com/markmals/pkg/pull/2
issues: []
supersedes: []
---

# First-Machine Setup and Bootstrap

## Summary

Make pkg the first software-management tool installed on a new machine. Provide a downloadable installer, let pkg obtain and maintain mise when needed, and add an explicit `pkg bootstrap` workflow for restoring an existing mise configuration. Keep everyday package synchronization separate from full machine setup.

## Motivation

The current installation instructions require Rust, mise, or Homebrew before pkg can be installed. That reverses the desired dependency: a person starting with a new computer wants pkg to help install everything else.

Restoring an existing setup also requires learning mise's bootstrap flags, locating the correct configuration directory, arranging its prerequisites, and distinguishing installation from authentication and other manual setup. Mark's dotfiles demonstrate the actual use case: a checkout at `~/.dotfiles`, global mise configuration in its `mise/` subdirectory, and bootstrap that restores packages, tools, dotfiles, settings, services, and hooks. The root `mise.toml` is for developing the repository, not restoring the machine.

## Domain grounding

### Established context

- [mise installation](https://mise.jdx.dev/installing-mise.html) supports official prebuilt executables without Rust or Homebrew. The upstream shell installer checks release checksums and supports a chosen installation path and version. Multiple mise executables on PATH can select an unintended version.
- [Mise Bootstrap](https://mise.jdx.dev/bootstrap.html) applies machine configuration, not just packages. Its declarative parts converge; hooks and the bootstrap task run again on subsequent invocations. A failed run is not a transaction: earlier changes remain.
- [Bootstrap from a repository](https://mise.jdx.dev/bootstrap/from-repository.html) distinguishes a bootstrap project from global configuration. `--from` applies a checkout; `--from` with `--config-dir` can link global configuration into a dotfiles checkout. `--adopt` has different semantics and is not interchangeable with either.
- [The upstream bootstrap contract](https://mise.jdx.dev/cli/bootstrap.html) supplies environment selection, configuration linking, dry runs, and explicit repository updates. An absent checkout prevents a complete dry-run plan. Existing checkouts must match the requested origin; updates are fast-forward-only.
- Upstream's [template execution warning](https://mise.jdx.dev/bootstrap.html#templates-that-run-commands) says file-content templates can run `exec()` during dry runs and status queries. [Safe mode](https://mise.jdx.dev/security.html#safe-mode) is not a sandbox, skips some configuration, and is not an equivalent full-bootstrap preview.

### Relevant constraints and principles

The executable installer, the mise executable, and the person's machine configuration have different owners and lifecycles. Obtaining mise does not authorize running arbitrary configuration. Running full bootstrap does not mean `pkg sync` should gain those effects. A Git checkout path also does not identify the configuration within it.

Preserve the existing package-management contract in [Packages](../crates/pkg/docs/packages.md) outside the explicit onboarding commands. Retain mise as the interpreter and owner of machine configuration. Do not build a second bootstrap engine, package inventory, dotfiles format, or secret store.

### Quality bar

A new user can install pkg without a compiler or another package manager, find out which mise it will use, and restore a reviewed setup through a small number of explicit commands. A failed download cannot damage an existing executable. Existing mise installations and global configuration are not silently replaced. Bootstrap reports partial completion rather than promising rollback or a completely restored machine when manual work remains.

### Remaining uncertainty

The strict dry-run guarantee and the first version's platform scope require resolution before implementation approval; see Open questions. Public release artifacts do not exist yet: GitHub's release list was empty during discovery. The installer URL below is proposed, not an available installation command.

## Existing baseline

[README installation](../README.md#install) offers Cargo installation and future mise/Homebrew installation. [Knope configuration](../knope.toml) already names archives for macOS arm64/x64, Linux arm64/x64 musl, and Windows x64; existing release provenance must be preserved.

[Environment resolution](../crates/pkg/src/environment.rs) chooses `PKG_MISE` or mise on PATH. [The mise adapter](../crates/pkg/src/mise/mod.rs) always runs from home, uses closed stdin, and tells the user to install mise themselves when it is absent. [Sync](../crates/pkg/src/commands/sync.rs) deliberately excludes settings, services, dotfiles, and bootstrap hooks.

Mark's local restore guide was inspected as evidence, not as a portable dependency or a proposed file to change. Its enrollment operation is `mise bootstrap --from <repository> --from-dir ~/.dotfiles --config-dir mise`; later runs use the linked global configuration. It also documents Git authentication, App Store and 1Password sign-in, Xcode requirements, sudo, and interactive hooks. pkg must not silently omit those operations to make restoration appear successful.

## Proposed solution

Use a small installer to obtain pkg, then put onboarding orchestration in pkg rather than in an expanding shell script. Reuse a compatible existing mise; provide a pkg-owned fallback only when needed. Add full bootstrap as a distinct command, leaving sync unchanged.

The human selected this direction, including automatically permitting upstream terminal interaction for text-mode bootstrap in a terminal. The human also selected the necessary expansion of the vision beyond package-only operations for this explicit command.

Proposed commands, not commands available today:

```sh
curl -fsSL https://github.com/markmals/pkg/releases/latest/download/install.sh | sh
~/.local/bin/pkg setup
~/.local/bin/pkg bootstrap --from https://github.com/example/dotfiles.git --directory ~/.dotfiles --config-dir mise --dry-run
~/.local/bin/pkg bootstrap --from https://github.com/example/dotfiles.git --directory ~/.dotfiles --config-dir mise
```

`setup` can be omitted before an actual bootstrap: bootstrap obtains missing mise as an explicit prerequisite of the requested restoration. A dry run does not.

## Detailed design

### Installing pkg without another package manager

Extend [README installation](../README.md#install), [release configuration](../knope.toml), and the existing release archive workflow with a POSIX `install.sh` release asset and a SHA-256 manifest. The stable URL resolves to a release-owned script whose embedded release identity fixes both archive and manifest selection, avoiding a mixed-version download across a moving `latest` redirect. Support an explicit released version for reproducible installation.

The script selects one supported host archive, verifies its checksum before extracting and executing anything, and installs `pkg` to `~/.local/bin/pkg`. It needs only the documented host shell, HTTPS downloader, archive utilities, and checksum utility, not Git, gh, mise, Homebrew, or Rust. It must fail clearly on unsupported hosts or missing prerequisites instead of choosing a nearby architecture or building from source.

Download and validate in temporary storage, reject unexpected archive members, and replace the destination atomically on the destination filesystem only after validation. Do not overwrite a symlink or an unrelated executable. Repeating installation of the same version is harmless; a failed update leaves the old executable intact. The script offers a dry-run mode that reports the version, platform, and destination without installing or changing configuration.

Do not run as root, invoke sudo, edit shell startup files, install mise, or apply a bootstrap from the pipe. Report the installed executable's absolute path and the PATH instruction needed to invoke it. A downloaded-and-inspected script is an equivalent supported installation method. Existing artifact attestations remain available for independent verification; a checksum downloaded from the same GitHub release detects corruption but is not independent proof against compromise of that release or the script. Document that trust boundary rather than calling checksums signature verification.

### Selecting and maintaining mise

Extend [environment resolution](../crates/pkg/src/environment.rs), [context reporting](../crates/pkg/src/commands/context.rs), and the [registry](../crates/pkg/src/registry.rs) with a single mise-selection contract:

1. An explicit `PKG_MISE` wins. An invalid, missing, or incompatible explicit executable is an error, never permission to use another one.
2. Otherwise reuse a compatible external mise on PATH. An incompatible or broken external executable is reported with its path and remediation; do not silently shadow it.
3. Otherwise use pkg's managed fallback, if installed; absent that, report mise as missing.

Proposed commands are `pkg setup`, `pkg mise status`, and `pkg mise upgrade [--version <version>]`. Setup installs missing mise or reports the selected compatible installation unchanged. Status is read-only and reports availability, path, version, and ownership. Upgrade modifies only the selected pkg-owned installation; an externally managed installation produces an actionable refusal, not a call to another package manager or an unqualified `mise self-update`.

Place the fallback below pkg's per-user application data directory, separate from external installations. Give it one current executable selection and retain mise's ordinary configuration, data, and state locations: it is a managed runtime, not a second package installation universe. Pass its absolute executable path internally and expose its directory to bootstrap children that invoke `mise` by name. Do not change the parent process's environment or the user's shell files. Setup reports how to make the managed executable available to an interactive shell if desired.

Use official upstream release archives with verified checksums, staged installation, and atomic selection. Initial setup uses a recommended mise release recorded with the pkg release; reject an explicitly requested version below pkg's supported minimum. An unversioned upgrade resolves an eligible stable upstream version, respecting upstream's documented minimum-release-age default, and reports the selected version before applying it. Never update mise merely because a package query or installation was requested. Preserve the last usable executable if download or validation fails.

The current project minimum is mise 2026.11.9. Implementation must verify the new command capabilities against the supported runtime floor and explicitly revise that floor in the owning documentation and registry if necessary; it must not assume current online documentation describes every older version.

All new mutations have dry runs. A setup or mise-upgrade preview reports the selected version and paths without downloading or executing an installer. Existing commands that require absent mise fail with a `pkg setup` remedy. Only actual `pkg bootstrap`, in addition to explicit setup, performs prerequisite installation automatically. Context and status must remain usable when mise is absent.

### Restoring the requested configuration

Add a registered `pkg bootstrap` command and generated reference changes through [the command registry](../crates/pkg/src/registry.rs). Keep package-only [sync](../crates/pkg/src/commands/sync.rs) unchanged. Specify these source forms and reject conflicting selectors before any mutation:

- No source: apply the existing global mise configuration from home.
- `--from <url> --directory <checkout> --config-dir <relative-directory>`: delegate upstream repository enrollment, reusing a matching checkout or cloning into the stated directory. `--directory` maps to upstream `--from-dir`; `--config-dir` is required for this personal-global-configuration workflow rather than guessing from directory names. `.` names the checkout root.
- `--config <path>`: apply a local existing mise config file explicitly, without turning the caller's current directory into the default configuration source or copying its declarations. The reference and receipt must identify whether upstream combines global configuration with that file; this is not an isolation promise.

The documented dotfiles recipe must use its configuration directory, not its repository-development `mise.toml`. Preserve mise's protection against an occupied global-config directory, a mismatched repository origin, dirty checkout updates, and config-directory traversal outside the checkout. Do not manually merge or rewrite configuration to evade a refusal.

Expose environment selection as `--env <name>` and persistence as explicit `--save-env`. Expose `--update` for an explicitly requested upstream repository/metadata update; never pull a checkout merely because bootstrap runs again. Preserve upstream pin/ref syntax rather than inventing another repository identity. Rerunning `pkg bootstrap` after enrollment uses mise's linked global configuration; pkg stores no duplicate enrollment database.

Full bootstrap applies every part selected by that configuration, including hooks and tasks. It is not transactional or unconditionally idempotent. No implicit pruning, conflict-forcing, history replacement, environment persistence, package upgrade, or repository update. Resolve file conflicts explicitly outside this first surface rather than automatically forwarding destructive force flags. Do not include arbitrary passthrough arguments that bypass the registry's effects and schemas.

### Interaction, trust, failure, and preview

Extend [the current stream contract](../crates/pkg/docs/packages.md#output-for-scripts-and-agents) only for full bootstrap: when text output and an interactive terminal are available, mise and its subprocesses may interact with that terminal for authentication, privilege elevation, and their own questions. pkg adds no prompts of its own. JSON and nonterminal runs must not wait for user input. They report the unavailable prerequisite or upstream failure and how to continue interactively; do not silently skip affected parts or pretend that closing stdin prevents every program from opening a controlling terminal.

Bootstrap progress remains distinct from the final pkg result. No credential values in receipts, arguments recorded in diagnostics, or persistent pkg state. Authentication stays with the existing Git, OS, and credential-manager workflows; a new computer may require human sign-in before a private repository can be read. Do not bundle a credential manager or bypass Apple licenses and authentication.

Naming a bootstrap source is permission to apply that reviewed source, consistent with mise's documented trust semantics. Explain that the source can execute arbitrary code. Do not turn a read, preview, or installer pipe into persistent blanket trust of configurations.

A successful receipt identifies the selected mise, source, effective config location, environments, whether mise setup or enrollment occurred, and known follow-up work. Report only outcomes upstream evidence establishes, not fabricated per-package or per-service success. A failure includes the phase, upstream status, prerequisite work that completed, and whether earlier bootstrap changes may remain. Do not promise to roll back arbitrary hooks, services, or files; preserve mise's diagnostics and recovery advice.

The existing promise in [Previewing with dry run](../crates/pkg/docs/packages.md#previewing-with-dry-run) remains the required safety boundary. A preview must not install mise, clone or update a repository, change trust, link configuration, write answers, run hooks, or execute configuration templates. Missing prerequisites make the preview incomplete, with the uninspected stages identified. The mechanism for a sufficiently useful preview without executing upstream templates is unresolved below; directly forwarding upstream `--dry-run` does not satisfy this design.

### Current sources and new contract homes

- [VISION.md](../VISION.md): expand the first-machine goal and permit explicit full bootstrap delegated to mise. Preserve package-only sync and mise's ownership of installed software. Narrow the no-extra-state principle to permit the managed executable and its release-selection metadata, not a parallel package database. This is a proposed vision revision, not a change made in this draft.
- [README.md](../README.md) and [Packages](../crates/pkg/docs/packages.md): distinguish installer, setup, full bootstrap, sync, runtime ownership, terminal interaction, and prerequisites.
- [Registry and command modules](../crates/pkg/src/registry.rs): own arguments, effects, version requirements, stable errors, receipt schemas, examples, and generated [CLI reference](../crates/pkg/docs/cli.md). Reuse existing error categories where meaningful; a new category must represent a distinct caller recovery action.
- [mise adapter](../crates/pkg/src/mise/mod.rs) and [environment resolution](../crates/pkg/src/environment.rs): consume the same selected executable and explicitly chosen bootstrap context; ordinary package operations retain home-directory isolation.
- [mise tasks](../mise.toml), [Knope](../knope.toml), and [release workflows](../.github/workflows/): build and verify installer assets with the release, including checksums and archive safety.
- New current documentation is needed for onboarding under `guides/` and in the binary's documentation set: start from the guide template, link the registry contract, and cover a blank home, an existing mise installation, and a dotfiles checkout with a config subdirectory. Update the project's command/Rust guidance to describe the approved bootstrap exceptions.

## Compatibility

Existing package commands, argument defaults, JSON fields, write-target rules, and sync behavior remain unchanged. An explicit `PKG_MISE` keeps precedence. No external mise, existing global config, or existing checkout is taken over automatically. The full-bootstrap effects and its terminal interaction are new, opt-in command behavior.

This proposal is independent of the library extraction in [PR #1](https://github.com/markmals/pkg/pull/1). If that lands first, put onboarding operations in the library and CLI presentation in the executable. If onboarding lands first, extraction must include its operations rather than leave a second implementation behind.

## Implications on adoption

Installing pkg needs network access and ordinary platform utilities, but no development toolchain. Applying a setup may need Git, OS privileges, account sign-ins, and config-specific tools. State those requirements before describing restoration as complete.

Removing pkg or its managed mise executable must not remove mise's packages, tools, configuration, or the dotfiles checkout. Users can keep an external mise and decline the fallback. Publishing the first release and any public installer endpoint remains a separately authorized release action; this proposal does not authorize deployment.

## Scope

One end-to-end first-machine path: obtain pkg, resolve or install mise, apply existing configuration, explain partial failure, and resume safely. Include lifecycle status and explicit mise upgrades, installation safety, the command contracts and guides, and tests against isolated homes and fake release servers.

### Out of scope

- A native app, UniFFI bindings, or mandatory library extraction.
- Installing Homebrew itself, calling brew, or duplicating mise's package/bootstrap engines.
- Creating or editing a person's bootstrap configuration, syncing dotfiles independently, or a secret-management system.
- Remote-host bootstrap, setup-history adoption, arbitrary upstream passthrough, and automatic force/prune operations.
- A pkg self-update subsystem: rerunning the reviewed installer or using the original package manager remains the pkg update path.

## Preview

- Repository shape: CLI with a release installer.
- Artifact: installable executable plus matching installer, archives, and checksum manifest for the reviewed revision; private CI artifacts, not a public release.
- Access: authenticated download linked from the review report, with an explicit local artifact source for previewing the installer. Do not publish an unapproved vanity endpoint.
- Exercise: disposable fresh macOS/Linux environments with no mise, and an existing external mise; a non-secret fixture dotfiles repository; interactive authentication/privilege boundaries; JSON noninteractive failures; checksum mismatch; interrupted installation; and dry-run filesystem snapshots. Do not run Mark's full bootstrap on the development machine as a test.
- Cleanup: discard disposable environments and preview downloads; retain review evidence, never credentials. Existing real-machine pkg reads remain useful for compatibility checks but cannot prove a blank-machine restore.

## Policies and decisions checked

No policy or decision records exist beyond directory READMEs. [VISION.md](../VISION.md#non-goals) currently excludes managing dotfiles, services, and settings. The human chose a narrow exception for explicitly delegating full bootstrap, not a replacement engine or a widened sync command. The human also chose interactive upstream behavior for terminal bootstrap; update the current unconditional no-prompt wording accordingly during implementation.

The managed runtime needs a stated exception to the vision's literal no-state wording. It must not become another source of truth for packages or configuration. Workbench's explicit release authorization remains unchanged.

## Future directions

A shorter installer domain can redirect to the same release-owned script. Native consumers could later use the same mise lifecycle and bootstrap operations. Setup-history adoption and remote-machine restoration are separate capabilities, not prerequisites for the dotfiles-checkout workflow here.

## Alternatives considered

- Require mise first: simplest internally, but fails the requested new-computer entry point.
- Always install a private mise: predictable version ownership, but duplicates working installations and hides the version a person already uses. The human selected reuse with a managed fallback instead.
- Make sync run full bootstrap: fewer command names, but adds arbitrary hooks and machine changes to a previously package-only command. The human selected separate bootstrap.
- Keep bootstrap completely noninteractive: preserves the old stream model but cannot reproduce the actual restore workflow without external orchestration. The human selected terminal-aware upstream interaction.
- Put restoration into the shell installer: makes the shortest command much more privileged and harder to preview, recover, test, or reuse from a library.

## Open questions

- [NEEDS CLARIFICATION: What preview mechanism preserves pkg's zero-mutation guarantee for full bootstrap, including template execution and trust writes? Choose a conservative non-evaluating/incomplete plan or establish an upstream capability that can preview the selected configuration safely; safe mode alone is not equivalent.]
- [NEEDS CLARIFICATION: Should first-machine installation and pkg-managed mise ship on Windows in this proposal, with a PowerShell equivalent, or be explicitly scoped to the four existing macOS/Linux archive targets while retaining current Windows CLI support?]
