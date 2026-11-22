---
id: proposal.0002
title: First-Machine Setup and Bootstrap
authors: [Mark]
status: implemented
pull-request: https://github.com/gotpkg/pkg/pull/2
issues: []
supersedes: []
---

# First-Machine Setup and Bootstrap

## Summary

Make pkg the first software-management tool installed on a new Mac or Linux machine. Provide an installer at `gotpkg.dev` that puts pkg on `PATH`, install mise with upstream's own installer when it is missing, hand pkg itself to mise once mise exists, replace `pkg upgrade` with `pkg update`, which also updates mise, and add an explicit `pkg bootstrap` workflow for restoring an existing mise configuration. Keep everyday package synchronization separate from full machine setup.

## Motivation

The current installation instructions require Rust, mise, or Homebrew before pkg can be installed. That reverses the desired dependency: a person starting with a new computer wants pkg to help install everything else.

Restoring an existing setup also requires learning mise's bootstrap flags, locating the correct configuration directory, arranging its prerequisites, and distinguishing installation from authentication and other manual setup. Mark's dotfiles demonstrate the actual use case: a checkout at `~/.dotfiles`, global mise configuration in its `mise/` subdirectory, and bootstrap that restores packages, tools, dotfiles, settings, services, and hooks. The root `mise.toml` is for developing the repository, not restoring the machine.

## Domain grounding

### Established context

- [mise installation](https://mise.jdx.dev/installing-mise.html) supports official prebuilt executables without Rust or Homebrew. The upstream shell installer checks release checksums and supports a chosen installation path and version. Multiple mise executables on PATH can select an unintended version.
- [Mise Bootstrap](https://mise.jdx.dev/bootstrap.html) applies machine configuration, not just packages. Its declarative parts converge; hooks and the bootstrap task run again on subsequent invocations. A failed run is not a transaction: earlier changes remain.
- [Bootstrap from a repository](https://mise.jdx.dev/bootstrap/from-repository.html) distinguishes a bootstrap project from global configuration. `--from` applies a checkout; `--from` with `--config-dir` can link global configuration into a dotfiles checkout. `--adopt` has different semantics and is not interchangeable with either.
- [The upstream bootstrap contract](https://mise.jdx.dev/cli/bootstrap.html) supplies environment selection, configuration linking, dry runs, and explicit repository updates. An absent checkout prevents a complete dry-run plan. Existing checkouts must match the requested origin; updates are fast-forward-only.
- Upstream's dry run prints hooks and the `bootstrap` task instead of running them and leaves `[dotfiles]` templates unrendered, but it renders `[bootstrap.files]` content templates (`template = true`), so an `exec()` in one runs during the dry run, as it does for `mise bootstrap status` and `plan` ([templates that run commands](https://mise.jdx.dev/bootstrap.html#templates-that-run-commands)). [Safe mode](https://mise.jdx.dev/security.html#safe-mode) is not a sandbox, skips some configuration, and is not an equivalent full-bootstrap preview.
- [Installing mise](https://mise.jdx.dev/installing-mise.html) is also the closest precedent for this installer. `mise.run` installs to `~/.local/bin`; its shell-specific endpoints append to that shell's startup file and skip the append when their own marker is present. Official binaries update with [`mise self-update`](https://mise.jdx.dev/cli/self-update.html), which verifies the release signature; an installation from a package manager updates through that manager instead.
- A script piped to `sh` runs in a child process and cannot change the environment of the shell that started it. An installer can change startup files for later shells and tell the person how to update the current one. macOS's default `PATH` does not include `~/.local/bin`.
- The [github backend](https://mise.jdx.dev/dev-tools/backends/github.html) installs a tool from a repository's release assets, choosing the asset by OS, architecture, and C library. It checks the digest GitHub reports for the asset and, on public GitHub, the release's artifact attestations, which pkg's release workflow already produces.
- On 2026-11-18, `https://pkgx.sh` returned its install script to curl and wget user agents and its HTML page to a browser user agent from the same URL; `https://mise.run` returned only the script. HTTP's `Accept` header is the standard signal for which representation a client wants: browsers ask for `text/html` when navigating, while curl and wget send `*/*`.
- A `symlink` dotfile entry never replaces a real file: [apply stops and lists the conflict](https://mise.jdx.dev/dotfiles/managed.html#conflicts) unless forced. Mark's macOS configuration deploys `~/.zshenv`, `~/.zprofile`, and `~/.zshrc` as symlinks, so a startup file an installer creates on a new Mac stops his bootstrap.
- Cloudflare Workers can [run the Worker before static assets](https://developers.cloudflare.com/workers/static-assets/binding/#run_worker_first) for chosen paths only. The project's Astro site, being built in `site/` on its own branch, is deployed as a Worker with prerendered pages, so it can answer `/` and `/install.sh` itself.

### Relevant constraints and principles

The executable installer, the mise executable, and the person's machine configuration have different owners and lifecycles. Obtaining mise does not authorize running arbitrary configuration. Running full bootstrap does not mean `pkg sync` should gain those effects. A Git checkout path also does not identify the configuration within it. pkg itself is a tool mise can install from its GitHub releases, so once mise exists it can own pkg's updates as it owns every other tool's; pkg needs no update logic of its own.

Preserve the existing package-management contract in [Packages](../crates/pkg/docs/packages.md) outside the explicit onboarding commands. Retain mise as the interpreter and owner of machine configuration. Do not build a second bootstrap engine, package inventory, dotfiles format, or secret store.

### Quality bar

A new user can install pkg without a compiler or another package manager, run it by name from a new shell, find out which mise it will use, and restore a reviewed setup through a small number of explicit commands. A failed download cannot damage an existing executable. Existing mise installations and global configuration are not silently replaced. Once mise manages pkg, no stale installer copy can shadow the version mise updates. Bootstrap reports partial completion rather than promising rollback or a completely restored machine when manual work remains.

### Remaining uncertainty

Public release artifacts do not exist yet: GitHub's release list was empty during discovery. Mark owns `gotpkg.dev`; on 2026-11-18 its DNS was on Cloudflare with no address records and the site was not merged, so the installer URL below is proposed, not available. Whether `mise version --json`, which reports a `latest` version, applies the same minimum release age as `mise self-update` must be checked before `pkg outdated` relies on it.

## Existing baseline

[README installation](../README.md#install) offers Cargo installation and future mise/Homebrew installation, naming `mise use -g github:gotpkg/pkg` as the mise form; the repository moved from Mark's account to the `gotpkg` organization, so no declaration of pkg names a person. [Knope configuration](../knope.toml) already names archives for macOS arm64/x64, Linux arm64/x64 musl, and Windows x64 by Rust target triple so that mise's `github:` backend matches them; existing release provenance must be preserved.

[Environment resolution](../crates/pkg/src/environment.rs) chooses `PKG_MISE` or mise on PATH and supplies the [core settings](../crates/pkg-core/src/settings.rs). [The core mise adapter](../crates/pkg-core/src/mise/mod.rs) always runs from home, uses closed stdin, and reports a missing mise through typed errors that the CLI turns into installation advice. [Sync](../crates/pkg-core/src/sync.rs) deliberately excludes settings, services, dotfiles, and bootstrap hooks.

Mark's local restore guide was inspected as evidence, not as a portable dependency or a proposed file to change. Its enrollment operation is `mise bootstrap --from <repository> --from-dir ~/.dotfiles --config-dir mise`; later runs use the linked global configuration. It also documents Git authentication, App Store and 1Password sign-in, Xcode requirements, sudo, and interactive hooks. pkg must not silently omit those operations to make restoration appear successful.

## Proposed solution

Use a small installer, served from `gotpkg.dev`, to obtain pkg and put it on `PATH`, then put onboarding orchestration in pkg rather than in an expanding shell script. Reuse a compatible existing mise; when there is none, install it with upstream's own installer and leave its updates to `mise self-update`, which `pkg update` runs along with everything else. Once mise is available, hand pkg to mise: declare it like any other tool and retire the installer's copy, so mise updates pkg from then on. Add full bootstrap as a distinct command, leaving sync unchanged. Support macOS and Linux only.

The human selected this direction, including automatically permitting upstream terminal interaction for text-mode bootstrap in a terminal. The human also selected the necessary expansion of the vision beyond package-only operations for this explicit command, the `gotpkg.dev` installer address, the `gotpkg` GitHub organization as pkg's home (`github:gotpkg/pkg`), an installer that sets up `PATH` and prints the command for the current shell, mise's ownership of pkg's updates, upstream's installer and self-update for mise, `pkg update` replacing `pkg upgrade` as the one command that updates everything, mise's shims on `PATH` for a person starting fresh, a bootstrap preview that skips templated files, and macOS and Linux as the only platforms.

During implementation the human also chose to merge this proposal before the site: a stub Worker deployed from this repository serves both `gotpkg.dev` routes until the site's Worker takes them over. The human also chose to ship no Homebrew tap, because the installer and mise are pkg's distribution and a Homebrew copy would be a second update channel.

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

The installer puts `pkg` on `PATH` for every new shell and, as Homebrew's installer does, ends by printing next steps for the person's shell, starting with the one command that does the same in the current terminal, such as `source ~/.zshenv`. Bootstrap obtains missing mise as an explicit prerequisite of the requested restoration, so it needs no `setup` first; a dry run does not obtain it.

## Detailed design

### Installing pkg without another package manager

Extend [README installation](../README.md#install), [release configuration](../knope.toml), and the existing release archive workflow with a POSIX `install.sh` release asset and a SHA-256 manifest. Archives, script, and manifest remain GitHub release assets; `gotpkg.dev` only serves the latest release's script. `https://gotpkg.dev/install.sh` always answers with that script. `https://gotpkg.dev` answers a request that accepts HTML, such as a browser's, with the site's home page, and any other request, such as curl's or wget's, with the same script. Decide by the request's `Accept` header, not a list of user agents. The script is release-owned: its embedded release identity fixes both archive and manifest selection, avoiding a mixed-version download across a moving `latest` redirect. Support an explicit released version for reproducible installation.

Serve both routes from the Worker that hosts the project's site (`site/`), so the routing lives in this repository, is tested with the site, and deploys with it. The Worker runs before static assets for `/` and `/install.sh` only. For a request to `/` that accepts HTML it falls through to the prerendered home page. Otherwise it responds `200` with the content of `https://github.com/gotpkg/pkg/releases/latest/download/install.sh` as plain text, with `Vary: Accept` on `/`, so `curl https://gotpkg.dev` works without following redirects and a browser can read `/install.sh`. The edge may cache the script for at most five minutes. The release workflow checks after publishing that `https://gotpkg.dev` serves the new release's script within that window. This proposal merges before the site. Until the site's Worker takes the domain over, a stub Worker in `installer/worker/`, deployed to `gotpkg.dev` from this repository, serves the same two routes, with a placeholder page for a request that accepts HTML, and answers `502` while no release exists. Once the site serves both routes, the stub is deleted.

The script selects one supported host archive, verifies its checksum before extracting and executing anything, and installs `pkg` to `~/.local/bin/pkg`. It needs only the documented host shell, HTTPS downloader, archive utilities, and checksum utility, not Git, gh, mise, Homebrew, or Rust. It must fail clearly on unsupported hosts or missing prerequisites instead of choosing a nearby architecture or building from source.

Download and validate in temporary storage, reject unexpected archive members, and replace the destination atomically on the destination filesystem only after validation. Do not overwrite a symlink or an unrelated executable. Repeating installation of the same version is harmless; a failed update leaves the old executable intact. The script offers a dry-run mode that reports the version, platform, and destination without installing or changing configuration.

Do not run as root, invoke sudo, install mise, or apply a bootstrap from the pipe. Put `~/.local/bin` on `PATH` for later shells: when it is not already on the `PATH` the script runs with, prepend it in the startup file every new shell of the person's login shell reads (`${ZDOTDIR:-$HOME}/.zshenv` for zsh; `~/.bashrc`, and `~/.bash_profile` if it exists or else `~/.profile`, for bash; `~/.config/fish/conf.d/pkg.fish` for fish) inside a marked block, so a rerun adds nothing. For any other shell, or when the person opts out through an installer option, report the line to add instead. The script cannot change the shell that ran it, so, as Homebrew's installer does, it ends with next steps for the shell named by `$SHELL`: first the command that loads the changed file in the current terminal (`source ~/.zshenv`, `source ~/.bashrc`, or `source ~/.config/fish/conf.d/pkg.fish`), then `pkg bootstrap` for restoring a configuration or `pkg setup` for starting fresh. When `~/.local/bin` was already on `PATH`, the first step is omitted. It also reports the installed executable's absolute path and each file it changed. Its dry run reports the files it would change. A downloaded-and-inspected script is an equivalent supported installation method. Existing artifact attestations remain available for independent verification; a checksum downloaded from the same GitHub release detects corruption but is not independent proof against compromise of that release or the script. Document that trust boundary rather than calling checksums signature verification.

### Selecting, installing, and updating mise

Extend [environment resolution](../crates/pkg/src/environment.rs), [core settings](../crates/pkg-core/src/settings.rs), [context reporting](../crates/pkg/src/commands/context.rs), and the [registry](../crates/pkg/src/registry.rs) with a single mise-selection contract:

1. An explicit `PKG_MISE` wins. An invalid, missing, or incompatible explicit executable is an error, never permission to use another one.
2. Otherwise reuse a compatible mise on PATH. An incompatible or broken executable is reported with its path and remediation; do not silently shadow it.
3. Otherwise use `~/.local/bin/mise`, where upstream's installer puts it, even when the current shell's `PATH` lacks that directory; absent that, report mise as missing.

pkg keeps no mise of its own and records nothing about it. `pkg setup` installs missing mise by running upstream's installer (`https://mise.run`) for `~/.local/bin/mise`, or reports the selected compatible installation unchanged. Upstream's script chooses the release, honoring its minimum release age, and verifies the archive's checksum; setup then checks that the installed mise meets pkg's minimum and fails with both versions if it does not. pkg runs the script as fetched over HTTPS, as a person running `curl https://mise.run | sh` does; it does not verify the script's GPG signature, because a new machine has no `gpg`, and the guide says so.

Setup also makes the tools mise installs runnable for a person starting fresh. When no global mise configuration exists and mise's shims directory (`$MISE_DATA_DIR/shims`, normally `~/.local/share/mise/shims`) is not on `PATH`, setup adds it after `~/.local/bin` in the installer's marked block, creating the block in the file the installer uses when there is none. `--no-modify-path` reports the line instead. Setup then reports the same shell-specific command as the installer for loading the change in the current terminal. A person restoring a configuration gets `PATH` from that configuration, and the bootstrap rule for the installer's block below covers the shims line too.

Rename `pkg upgrade` to `pkg update` and retire `upgrade` entirely, with no alias: in the [registry](../crates/pkg/src/registry.rs) and its generated [CLI reference](../crates/pkg/docs/cli.md), in [Keeping up to date](../crates/pkg/docs/packages.md#keeping-up-to-date), and in the [core API](../crates/pkg-core/src/lib.rs), where `Client::upgrade`, `UpgradeRequest`, `UpgradeReceipt`, and `Upgraded` become `Client::update`, `UpdateRequest`, `UpdateReceipt`, and `Updated`, and the receipt's `upgraded` list becomes `updated`. Upstream's own commands keep their names (`mise upgrade`, `mise bootstrap packages upgrade`). `pkg update` with no package names first runs `mise self-update --yes` on the selected mise, so packages and tools then update with the new release; if that fails, the command fails as a mise failure before changing anything else. A mise installed by a package manager, which refuses to update itself, is left to that package manager and listed as skipped with the reason, as casks that update themselves are. mise's version change appears as an entry named `mise` in the receipt's `updated` list. `pkg outdated` and `pkg update --dry-run` list mise when a newer release is available, from what the selected mise reports, without downloading it. Naming packages updates only those packages. Never update mise merely because a package query or installation was requested.

The current project minimum is mise 2026.11.9. Implementation must verify the new command capabilities against the supported runtime floor and explicitly revise that floor in the owning documentation and registry if necessary; it must not assume current online documentation describes every older version.

All new mutations have dry runs. A setup preview reports the installer it would run, its destination, and the `PATH` change, without downloading or running anything. Existing commands that require absent mise fail with a `pkg setup` remedy. Only actual `pkg bootstrap`, in addition to explicit setup, installs mise automatically. `pkg context show` remains usable when mise is absent and reports mise's path, version, and whether `pkg update` updates it.

### Handing pkg to mise

pkg gets no self-update mechanism. Once mise is available, it owns pkg's installation and updates as it owns every other tool's. Extend [setup and bootstrap](#restoring-the-requested-configuration), [context reporting](../crates/pkg/src/commands/context.rs), and [Packages](../crates/pkg/docs/packages.md) with these rules:

- **Declare.** An actual `pkg setup`, and an actual `pkg bootstrap` after enrollment and the configured bootstrap succeed, end by handing pkg to mise. When no loaded configuration declares `github:gotpkg/pkg`, pkg declares and installs it exactly as `pkg install github:gotpkg/pkg` does: through mise's tools write target, recorded as `latest`. An existing declaration, such as one restored from a dotfiles checkout, is used as it is, so after the first machine a bootstrap writes nothing and later machines restore pkg with the rest of the configuration.
- **Retire the installer's copy.** Once mise's pkg is installed and runs, replace the installer's executable at `~/.local/bin/pkg` with a link to mise's `pkg` shim. The `PATH` entry the installer created then keeps working with or without mise's shell activation, and every invocation runs the version mise selects. Replace only a regular file there that identifies itself as pkg. Leave every other pkg executable alone, and report any that a new shell would find before mise's. Never replace the running pkg with an older release: when mise selects an older version, for example because of a minimum release age, keep the installer's copy and say why.
- **Update.** `pkg outdated` and `pkg update` report and update pkg like any other tool, and mise verifies the release's digest and attestation. Before the handoff, rerunning the installer updates pkg. While mise provides pkg, the installer leaves the link in place and points to `pkg update github:gotpkg/pkg`.
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

The installer's `PATH` block must not stop a restored configuration. Before an actual bootstrap applies a configuration, pkg deletes each startup file that holds nothing but the installer's block, so a configuration that deploys that file, as Mark's deploys `~/.zshenv` as a symlink, does not stop on a conflict pkg created. After the bootstrap, whether it succeeded or failed, pkg restores the block in each such file the configuration did not deploy. A file with other content keeps its block, and a conflict there is the person's to resolve. The receipt reports both steps, and the dry run reports them without making them.

### Interaction, trust, failure, and preview

Extend [the current stream contract](../crates/pkg/docs/packages.md#output-for-scripts-and-agents) only for full bootstrap: when text output and an interactive terminal are available, mise and its subprocesses may interact with that terminal for authentication, privilege elevation, and their own questions. pkg adds no prompts of its own. JSON and nonterminal runs must not wait for user input. They report the unavailable prerequisite or upstream failure and how to continue interactively; do not silently skip affected parts or pretend that closing stdin prevents every program from opening a controlling terminal.

Bootstrap progress remains distinct from the final pkg result. No credential values in receipts, arguments recorded in diagnostics, or persistent pkg state. Authentication stays with the existing Git, OS, and credential-manager workflows; a new computer may require human sign-in before a private repository can be read. Do not bundle a credential manager or bypass Apple licenses and authentication.

Naming a bootstrap source is permission to apply that reviewed source, consistent with mise's documented trust semantics. Explain that the source can execute arbitrary code. Do not turn a read, preview, or installer pipe into persistent blanket trust of configurations.

A successful receipt identifies the selected mise, source, effective config location, environments, whether mise setup or enrollment occurred, and known follow-up work. Report only outcomes upstream evidence establishes, not fabricated per-package or per-service success. A failure includes the phase, upstream status, prerequisite work that completed, and whether earlier bootstrap changes may remain. Do not promise to roll back arbitrary hooks, services, or files; preserve mise's diagnostics and recovery advice.

The existing promise in [Previewing with dry run](../crates/pkg/docs/packages.md#previewing-with-dry-run) remains the safety boundary: a bootstrap preview must not install mise, clone or update a repository, change trust, link configuration, write answers, run hooks, or execute configuration templates. When mise and the configuration exist, the preview runs upstream's dry run, which prints hooks and the `bootstrap` task instead of running them and leaves `[dotfiles]` templates unrendered. Upstream's dry run does render `[bootstrap.files]` entries with `template = true`, and an `exec()` in one runs then, so when any loaded configuration declares such an entry the preview skips the `files` part and lists it as not previewed. A preview passes no option that saves answers. When mise or the checkout does not exist yet, the preview reports what pkg would install, clone, and link, and says the configuration itself can be previewed once they exist.

### Current sources and new contract homes

- [VISION.md](../VISION.md): expand the first-machine goal and permit explicit full bootstrap delegated to mise. Preserve package-only sync and mise's ownership of installed software, which now includes pkg itself once mise exists. pkg still keeps no state of its own: mise sits where its upstream installer puts it, and pkg's declaration lives in the person's mise configuration. This is a proposed vision revision, not a change made in this draft.
- [README.md](../README.md), [Packages](../crates/pkg/docs/packages.md), [VISION.md](../VISION.md)'s minimum viable product, and the [Rust library guide](../guides/rust-library.md): distinguish installer, `PATH` setup, setup, handing pkg to mise, full bootstrap, sync, `pkg update`, terminal interaction, and prerequisites; state that the onboarding commands support macOS and Linux; and say `update` wherever they say `upgrade` for pkg's command. The README's installation leads with `curl -fsSL https://gotpkg.dev | sh`.
- [Registry and command modules](../crates/pkg/src/registry.rs): own arguments, effects, version requirements, stable errors, receipt schemas, examples, and generated [CLI reference](../crates/pkg/docs/cli.md) for `setup`, `bootstrap`, `update`, and the changed `outdated`. `setup` and `bootstrap` on Windows fail with an unsupported-platform error. Reuse existing error categories where meaningful; a new category must represent a distinct caller recovery action.
- [Core API](../crates/pkg-core/src/lib.rs), [mise adapter](../crates/pkg-core/src/mise/mod.rs), [settings](../crates/pkg-core/src/settings.rs), and [CLI environment resolution](../crates/pkg/src/environment.rs): add typed onboarding operations, rename the upgrade operation to update, consume the same selected executable and explicitly chosen bootstrap context, and keep presentation in the CLI. Ordinary package operations retain home-directory isolation.
- [mise tasks](../mise.toml), [Knope](../knope.toml), and [release workflows](../.github/workflows/): build and verify installer assets with the release, including checksums and archive safety, and check `gotpkg.dev` after publishing. Remove the unused Homebrew tap job and its formula generator.
- The stub Worker in `installer/worker/`, with its Wrangler configuration, a deploy task, and tests for a request that accepts HTML and one that does not. After this proposal merges, the site's Worker in `site/` takes both routes over, as described above.
- New current documentation is needed for onboarding under `guides/` and in the binary's documentation set: start from the guide template, link the registry contract, and cover a blank home, an existing mise installation, a dotfiles checkout with a config subdirectory, and updating pkg before and after mise manages it. Update the project's command/Rust guidance to describe the approved bootstrap exceptions.

## Compatibility

Existing package commands, argument defaults, JSON fields, write-target rules, and sync behavior remain unchanged, with two exceptions. `pkg upgrade` becomes `pkg update`, whose receipt names its list `updated`; scripts that call `pkg upgrade` must change, which is acceptable because pkg has no published release. `pkg update` and `pkg outdated` with no package names also include mise itself as an entry named `mise`. An explicit `PKG_MISE` keeps precedence. No external mise, existing global config, or existing checkout is taken over automatically. The full-bootstrap effects and its terminal interaction are new, opt-in command behavior. Handing pkg to mise replaces a pkg executable at `~/.local/bin/pkg`, including one built with the README's `cargo install --root ~/.local`; a developer who wants a development build there reinstalls it after setup. A pkg installed by Homebrew or elsewhere is reported, not changed.

The library extraction in [proposal 0001](0001-reusable-package-library.md) is merged through [PR #1](https://github.com/gotpkg/pkg/pull/1). Put onboarding operations in `pkg-core` and CLI presentation in `pkg`, using the existing typed context, error, and progress boundaries rather than introducing a second implementation.

## Implications on adoption

Installing pkg needs network access and ordinary platform utilities, but no development toolchain. Applying a setup may need Git, OS privileges, account sign-ins, and config-specific tools. State those requirements before describing restoration as complete.

Removing pkg must not remove mise, its packages, tools, or configuration, or the dotfiles checkout. Once mise manages pkg, `pkg remove github:gotpkg/pkg` removes it as it removes any tool. A mise installed by a package manager stays in use and keeps updating through that package manager. Publishing the first release and deploying the site to `gotpkg.dev` remain separately authorized release actions; this proposal does not authorize deployment.

## Scope

One end-to-end first-machine path on macOS and Linux: obtain pkg from `gotpkg.dev`, put it on `PATH`, resolve or install mise, hand pkg to mise, apply existing configuration, explain partial failure, and resume safely. Include replacing `pkg upgrade` with `pkg update`, mise's update within it, mise's shims on `PATH` for a fresh start, installation safety, the two `gotpkg.dev` routes, the command contracts and guides, and tests against isolated homes and fake release servers.

### Out of scope

- A native app, UniFFI bindings, or mandatory library extraction.
- Installing Homebrew itself, calling brew, or duplicating mise's package/bootstrap engines.
- Creating or editing a person's bootstrap configuration other than declaring pkg itself, syncing dotfiles independently, or a secret-management system.
- Remote-host bootstrap, setup-history adoption, arbitrary upstream passthrough, and automatic force/prune operations.
- A pkg self-update mechanism: mise updates pkg once it manages it, and rerunning the installer updates pkg before then.
- The site's pages and Worker, which land after this proposal and take the installer routes over from the stub.
- Windows installation, setup, handoff, and bootstrap, including a PowerShell installer. Windows keeps its release archive and today's package commands.

## Preview

- Repository shape: CLI with a release installer.
- Artifact: installable executable plus matching installer, archives, and checksum manifest for the reviewed revision; private CI artifacts, not a public release.
- Access: authenticated download linked from the review report, with an explicit local artifact source for previewing the installer. Preview the `gotpkg.dev` routes through the stub Worker's tests and its deployment, with one request that accepts HTML and one that does not; the routes serve the installer once the first release exists.
- Exercise: disposable fresh macOS/Linux environments with no mise, and an existing external mise; `PATH` setup in zsh and bash, the printed command, a rerun, and the opt-out; handing pkg to mise and a later `pkg update` that updates a standalone mise and pkg and skips a Homebrew mise; a bootstrap of a fixture configuration that deploys `~/.zshenv` as a symlink after the installer created it; a preview of a fixture configuration with a templated `[bootstrap.files]` entry whose `exec()` would leave a marker file; a non-secret fixture dotfiles repository; interactive authentication/privilege boundaries; JSON noninteractive failures; checksum mismatch; interrupted installation; and dry-run filesystem snapshots. Do not run Mark's full bootstrap on the development machine as a test.
- Cleanup: discard disposable environments and preview downloads; retain review evidence, never credentials. Existing real-machine pkg reads remain useful for compatibility checks but cannot prove a blank-machine restore.

## Policies and decisions checked

No policy or decision records exist beyond directory READMEs. [VISION.md](../VISION.md#non-goals) currently excludes managing dotfiles, services, and settings. The human chose a narrow exception for explicitly delegating full bootstrap, not a replacement engine or a widened sync command. The human also chose interactive upstream behavior for terminal bootstrap; update the current unconditional no-prompt wording accordingly during implementation.

Workbench's explicit release authorization remains unchanged.

Installing mise with upstream's installer and handing pkg to mise follow the vision's goal that mise owns installation, and add no pkg state. Running `mise self-update` within `pkg update` stays within "update everything", which the human asked to include mise. Editing shell startup files is new for this project; the human chose it for the installer and for setup's shims line only.

## Future directions

A PowerShell installer and Windows setup can follow the same contracts later. Native consumers could later use the same mise lifecycle and bootstrap operations. Setup-history adoption and remote-machine restoration are separate capabilities, not prerequisites for the dotfiles-checkout workflow here.

## Alternatives considered

- Require mise first: simplest internally, but fails the requested new-computer entry point.
- Always install a private mise: predictable version ownership, but duplicates working installations and hides the version a person already uses. The human selected reuse, with upstream's installer when mise is missing.
- Make sync run full bootstrap: fewer command names, but adds arbitrary hooks and machine changes to a previously package-only command. The human selected separate bootstrap.
- Keep bootstrap completely noninteractive: preserves the old stream model but cannot reproduce the actual restore workflow without external orchestration. The human selected terminal-aware upstream interaction.
- Put restoration into the shell installer: makes the shortest command much more privileged and harder to preview, recover, test, or reuse from a library.
- pkg updates itself, as mise and bun do: pkg would gain download, verification, and replacement logic that mise already applies to every other tool. The human chose to hand pkg to mise.
- Register the installer's copy with `mise link`: mise would record a version it did not install, `~/.local/bin` would become pkg's installation directory with everything else in it, and the first upgrade would leave the stale copy earlier on `PATH`.
- Leave `PATH` to the person and print the full path, as the previous draft did: no startup-file edits, but every documented command needed the `~/.local/bin/` prefix. The human chose an installer that sets up `PATH`.
- Route `gotpkg.dev` with Cloudflare Single Redirect rules (the previous draft): no code, but the configuration lives outside the repository untested, beside a site Worker that already owns the domain.
- Redirect `/` to the GitHub release instead of serving the script: simpler, but needs `curl -L`, and a browser opening `/install.sh` downloads it instead of showing it.
- Choose the response by user agent, as `pkgx.sh` does: works for curl and wget, but every other fetcher needs a list entry, while `Accept` already says whether the client wants a page.
- A pkg-owned mise updated with `pkg mise upgrade` (the previous draft): pkg would carry download, verification, and selection logic for mise that `mise self-update` already has. The human chose upstream's installer and `mise self-update`.
- Pass `--force-dotfiles` when the installer's file conflicts: replaces every conflicting file, not only the one pkg created.
- Forward upstream's dry run unchanged: shows file changes from templated `[bootstrap.files]` entries, but their `exec()` calls would run during a preview, breaking pkg's dry-run promise.
- Keep `upgrade` and add `update` as its alias (the previous draft): two names for one command. The human found `update` clear and `upgrade` unclear, so `update` replaces it.
- Leave mise's tools off `PATH` for a person starting fresh and print how to activate mise: no extra startup-file line, but `pkg install ripgrep` would leave `rg` unrunnable. The human chose the shims line.

## Open questions

None.
