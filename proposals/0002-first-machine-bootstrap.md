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

Make pkg the first software-management tool installed on a new machine. Provide an installer at `gotpkg.dev` that puts pkg on `PATH`, let pkg obtain and maintain mise when needed, hand pkg itself to mise once mise exists, and add an explicit `pkg bootstrap` workflow for restoring an existing mise configuration. Keep everyday package synchronization separate from full machine setup.

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
- [Installing mise](https://mise.jdx.dev/installing-mise.html) is also the closest precedent for this installer. `mise.run` installs to `~/.local/bin`; its shell-specific endpoints append to that shell's startup file and skip the append when their own marker is present. Official binaries update with [`mise self-update`](https://mise.jdx.dev/cli/self-update.html), which verifies the release signature; an installation from a package manager updates through that manager instead.
- A script piped to `sh` runs in a child process and cannot change the environment of the shell that started it. An installer can change startup files for later shells and tell the person how to update the current one. macOS's default `PATH` does not include `~/.local/bin`.
- The [github backend](https://mise.jdx.dev/dev-tools/backends/github.html) installs a tool from a repository's release assets, choosing the asset by OS, architecture, and C library. It checks the digest GitHub reports for the asset and, on public GitHub, the release's artifact attestations, which pkg's release workflow already produces.
- On 2026-11-18, `https://pkgx.sh` returned its install script to curl and wget user agents and its HTML page to a browser user agent from the same URL; `https://mise.run` returned only the script. HTTP's `Accept` header is the standard signal for which representation a client wants: browsers ask for `text/html` when navigating, while curl and wget send `*/*`.

### Relevant constraints and principles

The executable installer, the mise executable, and the person's machine configuration have different owners and lifecycles. Obtaining mise does not authorize running arbitrary configuration. Running full bootstrap does not mean `pkg sync` should gain those effects. A Git checkout path also does not identify the configuration within it. pkg itself is a tool mise can install from its GitHub releases, so once mise exists it can own pkg's updates as it owns every other tool's; pkg needs no update logic of its own.

Preserve the existing package-management contract in [Packages](../crates/pkg/docs/packages.md) outside the explicit onboarding commands. Retain mise as the interpreter and owner of machine configuration. Do not build a second bootstrap engine, package inventory, dotfiles format, or secret store.

### Quality bar

A new user can install pkg without a compiler or another package manager, run it by name from a new shell, find out which mise it will use, and restore a reviewed setup through a small number of explicit commands. A failed download cannot damage an existing executable. Existing mise installations and global configuration are not silently replaced. Once mise manages pkg, no stale installer copy can shadow the version mise updates. Bootstrap reports partial completion rather than promising rollback or a completely restored machine when manual work remains.

### Remaining uncertainty

The strict dry-run guarantee, the first version's platform scope, and the owner of mise's own updates require resolution before implementation approval; see Open questions. Public release artifacts do not exist yet: GitHub's release list was empty during discovery. Mark owns `gotpkg.dev`; on 2026-11-18 its DNS was on Cloudflare with no address records, so the installer URL below is proposed, not available. Cloudflare's rules language exposes request headers as `http.request.headers`, and Single Redirect rules are available on every plan; whether the zone's redirect-rule editor accepts that field must be confirmed in the zone before configuring it.

## Existing baseline

[README installation](../README.md#install) offers Cargo installation and future mise/Homebrew installation, naming `mise use -g github:markmals/pkg` as the mise form. [Knope configuration](../knope.toml) already names archives for macOS arm64/x64, Linux arm64/x64 musl, and Windows x64 by Rust target triple so that mise's `github:` backend matches them; existing release provenance must be preserved.

[Environment resolution](../crates/pkg/src/environment.rs) chooses `PKG_MISE` or mise on PATH and supplies the [core settings](../crates/pkg-core/src/settings.rs). [The core mise adapter](../crates/pkg-core/src/mise/mod.rs) always runs from home, uses closed stdin, and reports a missing mise through typed errors that the CLI turns into installation advice. [Sync](../crates/pkg-core/src/sync.rs) deliberately excludes settings, services, dotfiles, and bootstrap hooks.

Mark's local restore guide was inspected as evidence, not as a portable dependency or a proposed file to change. Its enrollment operation is `mise bootstrap --from <repository> --from-dir ~/.dotfiles --config-dir mise`; later runs use the linked global configuration. It also documents Git authentication, App Store and 1Password sign-in, Xcode requirements, sudo, and interactive hooks. pkg must not silently omit those operations to make restoration appear successful.

## Proposed solution

Use a small installer, served from `gotpkg.dev`, to obtain pkg and put it on `PATH`, then put onboarding orchestration in pkg rather than in an expanding shell script. Reuse a compatible existing mise; provide a pkg-owned fallback only when needed. Once mise is available, hand pkg to mise: declare it like any other tool and retire the installer's copy, so mise updates pkg from then on. Add full bootstrap as a distinct command, leaving sync unchanged.

The human selected this direction, including automatically permitting upstream terminal interaction for text-mode bootstrap in a terminal. The human also selected the necessary expansion of the vision beyond package-only operations for this explicit command, the `gotpkg.dev` installer address, an installer that sets up `PATH`, and mise's ownership of pkg's updates.

Proposed commands, not commands available today. Restoring a dotfiles checkout on a new machine:

```sh
curl -fsSL https://gotpkg.dev | sh
pkg bootstrap --from https://github.com/example/dotfiles.git --directory ~/.dotfiles --config-dir mise --dry-run
pkg bootstrap --from https://github.com/example/dotfiles.git --directory ~/.dotfiles --config-dir mise
```

Starting with no configuration to restore:

```sh
curl -fsSL https://gotpkg.dev | sh
pkg setup
```

The installer puts `pkg` on `PATH` for every new shell; the shell that ran it needs a new terminal or the one command the installer prints. Bootstrap obtains missing mise as an explicit prerequisite of the requested restoration, so it needs no `setup` first; a dry run does not obtain it.

## Detailed design

### Installing pkg without another package manager

Extend [README installation](../README.md#install), [release configuration](../knope.toml), and the existing release archive workflow with a POSIX `install.sh` release asset and a SHA-256 manifest. Archives, script, and manifest remain GitHub release assets; `gotpkg.dev` only routes to them. `https://gotpkg.dev/install.sh` always answers with the latest release's script. `https://gotpkg.dev` answers a request that accepts HTML, such as a browser's, with the project's page, which is the GitHub repository until a website exists, and any other request, such as curl's or wget's, with the same script. Decide by the request's `Accept` header, not a list of user agents. The script is release-owned: its embedded release identity fixes both archive and manifest selection, avoiding a mixed-version download across a moving `latest` redirect. Support an explicit released version for reproducible installation.

Implement the routing as redirects to `https://github.com/markmals/pkg/releases/latest/download/install.sh` and to the repository, configured as Single Redirect rules in the `gotpkg.dev` Cloudflare zone behind a proxied apex record: no code to deploy and no deploy credential. Document the rules and the DNS record in the README's Releasing section, and have the release workflow check after publishing that `https://gotpkg.dev` returns the new release's script.

The script selects one supported host archive, verifies its checksum before extracting and executing anything, and installs `pkg` to `~/.local/bin/pkg`. It needs only the documented host shell, HTTPS downloader, archive utilities, and checksum utility, not Git, gh, mise, Homebrew, or Rust. It must fail clearly on unsupported hosts or missing prerequisites instead of choosing a nearby architecture or building from source.

Download and validate in temporary storage, reject unexpected archive members, and replace the destination atomically on the destination filesystem only after validation. Do not overwrite a symlink or an unrelated executable. Repeating installation of the same version is harmless; a failed update leaves the old executable intact. The script offers a dry-run mode that reports the version, platform, and destination without installing or changing configuration.

Do not run as root, invoke sudo, install mise, or apply a bootstrap from the pipe. Put `~/.local/bin` on `PATH` for later shells: when it is not already on the `PATH` the script runs with, prepend it in the startup file every new shell of the person's login shell reads (`${ZDOTDIR:-$HOME}/.zshenv` for zsh; `~/.bashrc`, and `~/.bash_profile` if it exists or else `~/.profile`, for bash; a file in `~/.config/fish/conf.d/` for fish) inside a marked block, so a rerun adds nothing. For any other shell, or when the person opts out through an installer option, report the line to add instead. The script cannot change the shell that ran it, so it reports the installed executable's absolute path, each file it changed, and the one command that makes `pkg` available in the current shell. Its dry run reports the files it would change. A downloaded-and-inspected script is an equivalent supported installation method. Existing artifact attestations remain available for independent verification; a checksum downloaded from the same GitHub release detects corruption but is not independent proof against compromise of that release or the script. Document that trust boundary rather than calling checksums signature verification.

### Selecting and maintaining mise

Extend [environment resolution](../crates/pkg/src/environment.rs), [core settings](../crates/pkg-core/src/settings.rs), [context reporting](../crates/pkg/src/commands/context.rs), and the [registry](../crates/pkg/src/registry.rs) with a single mise-selection contract:

1. An explicit `PKG_MISE` wins. An invalid, missing, or incompatible explicit executable is an error, never permission to use another one.
2. Otherwise reuse a compatible external mise on PATH. An incompatible or broken external executable is reported with its path and remediation; do not silently shadow it.
3. Otherwise use pkg's managed fallback, if installed; absent that, report mise as missing.

Proposed commands are `pkg setup`, `pkg mise status`, and `pkg mise upgrade [--version <version>]`. Setup installs missing mise or reports the selected compatible installation unchanged. Status is read-only and reports availability, path, version, and ownership. Upgrade modifies only the selected pkg-owned installation; an externally managed installation produces an actionable refusal, not a call to another package manager or an unqualified `mise self-update`.

Place the fallback below pkg's per-user application data directory, separate from external installations. Give it one current executable selection and retain mise's ordinary configuration, data, and state locations: it is a managed runtime, not a second package installation universe. Pass its absolute executable path internally and expose its directory to bootstrap children that invoke `mise` by name. Do not change the parent process's environment or the user's shell files. Setup reports how to make the managed executable available to an interactive shell if desired.

Use official upstream release archives with verified checksums, staged installation, and atomic selection. Initial setup uses a recommended mise release recorded with the pkg release; reject an explicitly requested version below pkg's supported minimum. An unversioned upgrade resolves an eligible stable upstream version, respecting upstream's documented minimum-release-age default, and reports the selected version before applying it. Never update mise merely because a package query or installation was requested. Preserve the last usable executable if download or validation fails.

The current project minimum is mise 2026.11.9. Implementation must verify the new command capabilities against the supported runtime floor and explicitly revise that floor in the owning documentation and registry if necessary; it must not assume current online documentation describes every older version.

All new mutations have dry runs. A setup or mise-upgrade preview reports the selected version and paths without downloading or executing an installer. Existing commands that require absent mise fail with a `pkg setup` remedy. Only actual `pkg bootstrap`, in addition to explicit setup, performs prerequisite installation automatically. Context and status must remain usable when mise is absent.

### Handing pkg to mise

pkg gets no self-update mechanism. Once mise is available, it owns pkg's installation and updates as it owns every other tool's. Extend [setup and bootstrap](#restoring-the-requested-configuration), [context reporting](../crates/pkg/src/commands/context.rs), and [Packages](../crates/pkg/docs/packages.md) with these rules:

- **Declare.** An actual `pkg setup`, and an actual `pkg bootstrap` after enrollment and the configured bootstrap succeed, end by handing pkg to mise. When no loaded configuration declares `github:markmals/pkg`, pkg declares and installs it exactly as `pkg install github:markmals/pkg` does: through mise's tools write target, recorded as `latest`. An existing declaration, such as one restored from a dotfiles checkout, is used as it is, so after the first machine a bootstrap writes nothing and later machines restore pkg with the rest of the configuration.
- **Retire the installer's copy.** Once mise's pkg is installed and runs, replace the installer's executable at `~/.local/bin/pkg` with a link to mise's `pkg` shim. The `PATH` entry the installer created then keeps working with or without mise's shell activation, and every invocation runs the version mise selects. Replace only a regular file there that identifies itself as pkg. Leave every other pkg executable alone, and report any that a new shell would find before mise's. Never replace the running pkg with an older release: when mise selects an older version, for example because of a minimum release age, keep the installer's copy and say why.
- **Update.** `pkg outdated` and `pkg upgrade` report and update pkg like any other tool, and mise verifies the release's digest and attestation. Before the handoff, rerunning the installer updates pkg. While mise provides pkg, the installer leaves the link in place and points to `pkg upgrade github:markmals/pkg`.
- **Report.** `pkg context show` reports which pkg is running and its owner: the installer's copy, mise with the declared and installed versions, or another installation. Dry runs of setup and bootstrap report the declaration, installation, and link they would make, and make none of them.
- **Order.** Enrollment with `--config-dir` needs mise's config directory to be missing, empty, or already the link. On a machine without a global configuration, setup's declaration creates one, so a later `pkg bootstrap --from … --config-dir …` refuses before any mutation and says to move that configuration aside. The onboarding guide therefore sends a person restoring a checkout straight to `pkg bootstrap`.

### Restoring the requested configuration

Add a registered `pkg bootstrap` command and generated reference changes through [the command registry](../crates/pkg/src/registry.rs). Keep package-only [sync](../crates/pkg/src/commands/sync.rs) unchanged. Specify these source forms and reject conflicting selectors before any mutation:

- No source: apply the existing global mise configuration from home.
- `--from <url> --directory <checkout> --config-dir <relative-directory>`: delegate upstream repository enrollment, reusing a matching checkout or cloning into the stated directory. `--directory` maps to upstream `--from-dir`. `--config-dir` names the directory of the checkout that becomes the global mise configuration, such as `mise` in Mark's dotfiles or `.config/mise` in upstream's example; `.` names the checkout root. Without it, upstream applies the checkout's root `mise.toml` as a bootstrap project and links nothing, which for Mark's dotfiles would run only the repository's development configuration and leave later pkg commands recording outside the checkout. It is required rather than guessed from directory names.
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

- [VISION.md](../VISION.md): expand the first-machine goal and permit explicit full bootstrap delegated to mise. Preserve package-only sync and mise's ownership of installed software, which now includes pkg itself once mise exists. Narrow the no-extra-state principle to permit the managed executable and its release-selection metadata, not a parallel package database. This is a proposed vision revision, not a change made in this draft.
- [README.md](../README.md) and [Packages](../crates/pkg/docs/packages.md): distinguish installer, `PATH` setup, setup, handing pkg to mise, full bootstrap, sync, runtime ownership, terminal interaction, and prerequisites. The README's installation leads with `curl -fsSL https://gotpkg.dev | sh`, and its Releasing section records the `gotpkg.dev` rules.
- [Registry and command modules](../crates/pkg/src/registry.rs): own arguments, effects, version requirements, stable errors, receipt schemas, examples, and generated [CLI reference](../crates/pkg/docs/cli.md). Reuse existing error categories where meaningful; a new category must represent a distinct caller recovery action.
- [Core API](../crates/pkg-core/src/lib.rs), [mise adapter](../crates/pkg-core/src/mise/mod.rs), [settings](../crates/pkg-core/src/settings.rs), and [CLI environment resolution](../crates/pkg/src/environment.rs): add typed onboarding operations, consume the same selected executable and explicitly chosen bootstrap context, and keep presentation in the CLI. Ordinary package operations retain home-directory isolation.
- [mise tasks](../mise.toml), [Knope](../knope.toml), and [release workflows](../.github/workflows/): build and verify installer assets with the release, including checksums and archive safety, and check `gotpkg.dev` after publishing.
- New current documentation is needed for onboarding under `guides/` and in the binary's documentation set: start from the guide template, link the registry contract, and cover a blank home, an existing mise installation, a dotfiles checkout with a config subdirectory, and updating pkg before and after mise manages it. Update the project's command/Rust guidance to describe the approved bootstrap exceptions.

## Compatibility

Existing package commands, argument defaults, JSON fields, write-target rules, and sync behavior remain unchanged. An explicit `PKG_MISE` keeps precedence. No external mise, existing global config, or existing checkout is taken over automatically. The full-bootstrap effects and its terminal interaction are new, opt-in command behavior. Handing pkg to mise replaces a pkg executable at `~/.local/bin/pkg`, including one built with the README's `cargo install --root ~/.local`; a developer who wants a development build there reinstalls it after setup. A pkg installed by Homebrew or elsewhere is reported, not changed.

The library extraction in [proposal 0001](0001-reusable-package-library.md) is merged through [PR #1](https://github.com/markmals/pkg/pull/1). Put onboarding operations in `pkg-core` and CLI presentation in `pkg`, using the existing typed context, error, and progress boundaries rather than introducing a second implementation.

## Implications on adoption

Installing pkg needs network access and ordinary platform utilities, but no development toolchain. Applying a setup may need Git, OS privileges, account sign-ins, and config-specific tools. State those requirements before describing restoration as complete.

Removing pkg or its managed mise executable must not remove mise's packages, tools, configuration, or the dotfiles checkout. Once mise manages pkg, `pkg remove github:markmals/pkg` removes it as it removes any tool. Users can keep an external mise and decline the fallback. Publishing the first release and configuring `gotpkg.dev` remain separately authorized release actions; this proposal does not authorize deployment.

## Scope

One end-to-end first-machine path: obtain pkg from `gotpkg.dev`, put it on `PATH`, resolve or install mise, hand pkg to mise, apply existing configuration, explain partial failure, and resume safely. Include lifecycle status and explicit mise upgrades, installation safety, the command contracts and guides, and tests against isolated homes and fake release servers.

### Out of scope

- A native app, UniFFI bindings, or mandatory library extraction.
- Installing Homebrew itself, calling brew, or duplicating mise's package/bootstrap engines.
- Creating or editing a person's bootstrap configuration other than declaring pkg itself, syncing dotfiles independently, or a secret-management system.
- Remote-host bootstrap, setup-history adoption, arbitrary upstream passthrough, and automatic force/prune operations.
- A pkg self-update mechanism: mise updates pkg once it manages it, and rerunning the installer updates pkg before then.
- A website at `gotpkg.dev`; browsers go to the repository until one exists.

## Preview

- Repository shape: CLI with a release installer.
- Artifact: installable executable plus matching installer, archives, and checksum manifest for the reviewed revision; private CI artifacts, not a public release.
- Access: authenticated download linked from the review report, with an explicit local artifact source for previewing the installer. `gotpkg.dev` is configured only as part of the authorized first release; once it is, check it with one request that accepts HTML and one that does not.
- Exercise: disposable fresh macOS/Linux environments with no mise, and an existing external mise; `PATH` setup in zsh and bash, a rerun, and the opt-out; handing pkg to mise and a later `pkg upgrade` of pkg against a fixture release source; a non-secret fixture dotfiles repository; interactive authentication/privilege boundaries; JSON noninteractive failures; checksum mismatch; interrupted installation; and dry-run filesystem snapshots. Do not run Mark's full bootstrap on the development machine as a test.
- Cleanup: discard disposable environments and preview downloads; retain review evidence, never credentials. Existing real-machine pkg reads remain useful for compatibility checks but cannot prove a blank-machine restore.

## Policies and decisions checked

No policy or decision records exist beyond directory READMEs. [VISION.md](../VISION.md#non-goals) currently excludes managing dotfiles, services, and settings. The human chose a narrow exception for explicitly delegating full bootstrap, not a replacement engine or a widened sync command. The human also chose interactive upstream behavior for terminal bootstrap; update the current unconditional no-prompt wording accordingly during implementation.

The managed runtime needs a stated exception to the vision's literal no-state wording. It must not become another source of truth for packages or configuration. Workbench's explicit release authorization remains unchanged.

Handing pkg to mise follows the vision's goal that mise owns installation and adds no pkg state: the declaration lives in the person's mise configuration. Editing shell startup files is new for this project; the human chose it for the installer only.

## Future directions

A website at `gotpkg.dev` can replace the repository as the page browsers receive without changing the script's URLs. Native consumers could later use the same mise lifecycle and bootstrap operations. Setup-history adoption and remote-machine restoration are separate capabilities, not prerequisites for the dotfiles-checkout workflow here.

## Alternatives considered

- Require mise first: simplest internally, but fails the requested new-computer entry point.
- Always install a private mise: predictable version ownership, but duplicates working installations and hides the version a person already uses. The human selected reuse with a managed fallback instead.
- Make sync run full bootstrap: fewer command names, but adds arbitrary hooks and machine changes to a previously package-only command. The human selected separate bootstrap.
- Keep bootstrap completely noninteractive: preserves the old stream model but cannot reproduce the actual restore workflow without external orchestration. The human selected terminal-aware upstream interaction.
- Put restoration into the shell installer: makes the shortest command much more privileged and harder to preview, recover, test, or reuse from a library.
- pkg updates itself, as mise and bun do: pkg would gain download, verification, and replacement logic that mise already applies to every other tool. The human chose to hand pkg to mise.
- Register the installer's copy with `mise link`: mise would record a version it did not install, `~/.local/bin` would become pkg's installation directory with everything else in it, and the first upgrade would leave the stale copy earlier on `PATH`.
- Leave `PATH` to the person and print the full path, as the previous draft did: no startup-file edits, but every documented command needed the `~/.local/bin/` prefix. The human chose an installer that sets up `PATH`.
- Route `gotpkg.dev` with a Cloudflare Worker: version-controlled and testable, but adds code, a toolchain, and a deploy credential for two redirects.
- Choose the response by user agent, as `pkgx.sh` does: works for curl and wget, but every other fetcher needs a list entry, while `Accept` already says whether the client wants a page.

## Open questions

- [NEEDS CLARIFICATION: What preview mechanism preserves pkg's zero-mutation guarantee for full bootstrap, including template execution and trust writes? Choose a conservative non-evaluating/incomplete plan or establish an upstream capability that can preview the selected configuration safely; safe mode alone is not equivalent.]
- [NEEDS CLARIFICATION: Should first-machine installation and pkg-managed mise ship on Windows in this proposal, with a PowerShell equivalent, or be explicitly scoped to the four existing macOS/Linux archive targets while retaining current Windows CLI support?]
- [NEEDS CLARIFICATION: Should pkg still install mise into its own data directory and update it with `pkg mise upgrade`, or should `pkg setup` install mise with upstream's installer at `~/.local/bin/mise` and leave its updates to `mise self-update`, as the review argues the owner of updates should be mise rather than pkg? Recommended: upstream's installer and `mise self-update`, removing `pkg mise upgrade` and the pkg-owned runtime.]
