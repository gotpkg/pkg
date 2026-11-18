---
id: proposal.0004
title: Xcode Management
authors: [Mark]
status: draft
pull-request: https://github.com/markmals/pkg/pull/4
issues: []
supersedes: []
---

# Xcode Management

## Summary

Add a `pkg xcode` command group that lists, installs, links, selects and removes Xcode through the mise-xcode plugin. The mise config records at most one release and one beta with an explicit default. Signing in happens on Apple's own page, replacing the hand-copied session cookie.

## Motivation

Xcode is the one piece of software on Mark's Macs that pkg cannot manage well, and the plugin that can install it is hard to drive by hand.

Installing a version with [mise-xcode](https://github.com/markmals/mise-xcode) today takes `mise ls-remote --prerelease xcode` to find the name, a `Cookie` request header copied out of an authenticated browser download into `MISE_XCODE_COOKIE`, then `mise install`, `mise use -g`, and later `mise uninstall` for the version it replaced. On Mark's Mac on 2026-11-18 that path had not been used: both Xcodes mise knows are links to applications in `/Applications` that another manager downloaded, `MISE_XCODE_COOKIE` is unset, and Xcodes.app is still installed.

pkg's generic commands give wrong or dangerous answers for Xcode. Each of these was run with the binary built from `main`:

- `pkg install xcode@16.4 --dry-run` answers that `xcode 27.2 is already installed`. A tool is identified by its name, so a second version is never installed.
- `pkg remove xcode --dry-run` plans to take `xcode` out of the config and uninstall it. The real run uses `mise unuse xcode`, which uninstalls every Xcode no config needs, not one version.
- The global config declares only `xcode = "27.2"`. The linked `27.1-beta.1` is undeclared, and `mise prune --dry-run xcode` lists it as prunable. The config does not describe the machine, which is the first goal in [the vision](../VISION.md).

An agent is worse off. It cannot copy a cookie out of a browser, and it has no way to ask whether a usable Apple session exists. Without one, `mise install xcode@27.1` ends in a Lua stack trace from the plugin, which pkg can only report as `MISE_FAILED`.

## Domain grounding

### Established context

- Apple distributes Xcode outside the App Store as signed `.xip` archives on `download.developer.apple.com`, behind an authenticated Apple Developer session. Several versions coexist as separate application bundles, each with its own toolchain and SDKs.
- A process selects an Xcode through `DEVELOPER_DIR`. The machine-wide selection is the `xcode-select` path, which needs root to change ([`xcode-select(1)`](https://keith.github.io/xcode-man-pages/xcode-select.1.html)). A new Xcode also needs its license accepted and first-launch components installed, both as an administrator. `xcodebuild -checkFirstLaunchStatus` and `xcodebuild -license check` report readiness without changing anything; both exit 0 for the two Xcodes on Mark's Mac. Their failing exit was not observed.
- [xcodes](https://github.com/XcodesOrg/xcodes) is the established manager. Its commands include `list`, `installed`, `install` (a version, `--latest`, `--latest-prerelease`), `select`, `uninstall`, `runtimes` and `signout`. It installs into `/Applications`, moves an uninstalled Xcode to the Trash, and asks for the macOS password to run the root steps. It signs in by asking for the Apple Account password and two-factor code and speaking Apple's undocumented web sign-in protocol itself (SRP and a hashcash challenge, in [XcodesLoginKit](https://github.com/XcodesOrg/XcodesLoginKit)), then keeps the password in the Keychain.
- [Xcode Releases](https://xcodereleases.com) is a community-maintained catalogue, one JSON document of about 390 KB. Each entry has the version number, the build number, the release kind (release, beta N, release candidate N), the date, the oldest macOS that runs it, the download URL, its architectures and checksums. xcodes and mise-xcode both read it.
- mise-xcode ([README](https://github.com/markmals/mise-xcode#readme), [vision](https://github.com/markmals/mise-xcode/blob/main/VISION.md)) names versions `27.2`, `27.3-beta.2`, `27.1-rc.1`. It reads the session from `MISE_XCODE_COOKIE`, checks it with a `HEAD` request, lets mise download and checksum the archive, then verifies Apple's archive and application signatures. The application lands at `$(mise where xcode)/Xcode.app`. It never runs `xcode-select` or `sudo` and never touches `/Applications`. `mise link xcode@<version> <application>` registers an application that already exists, and uninstalling a linked version leaves the application in place. mise's Lua `http` module offers `get`, `head` and `download_file` only ([Plugin Lua reference](https://mise.jdx.dev/plugin-lua-modules.html#http)), so the plugin cannot run a sign-in that needs `POST`.
- Observed on mise 2026.11.9 in an isolated config and data directory, with two linked Xcodes:
  - `mise use -g 'xcode[default=true]@27.2' 'xcode[default=false]@27.1-beta.1'` writes `xcode = [{ version = "27.2", default = "true" }, { version = "27.1-beta.1", default = "false" }]` in place and keeps the comments around it.
  - `mise use -g xcode@<one version>` replaces the whole entry. `mise unuse -g --no-prune xcode@<version>` removes one version from the list.
  - With two versions declared, the first one listed supplies `DEVELOPER_DIR`. The plugin does not read the `default` option today. Hooks do receive each entry's options ([tool options](https://mise.jdx.dev/tool-plugin-development.html#tool-options)).
  - `mise ls-remote --prerelease --json xcode` returns each version with its date and a `prerelease` flag, and no build number or macOS requirement.
  - `mise prune` deletes installed versions that no tracked config selects.
- mise's install directory holds application bundles from other tools under `~/.local`, and Spotlight returns none of them (`mdfind` for application bundles under `~/.local` is empty). An Xcode the plugin downloads is therefore not found by Spotlight.

### Relevant constraints and principles

- The vision's principles apply unchanged: mise decides and pkg translates, pkg edits only the write target, a command does exactly what was typed, and pkg keeps no second record of what is installed.
- A download session acts for the person's Apple Developer account. It must never reach a config file, a lockfile, a command line, a log, a receipt or an error.
- pkg should not hold the Apple Account password at all. A program that never receives it cannot leak it.
- These are different things and the design keeps them apart: a version name and the application it names; an Xcode mise downloaded (managed) and one the person already had (linked); a version's channel, which is a fact about it, and a slot, which is pkg's rule; the default mise selects and the Xcode `xcode-select` points at; a session and the account it belongs to.

### Quality bar

- From a Mac with no Xcode to a declared, selected Xcode in two commands, without a browser's developer tools.
- The config alone says which Xcodes the machine has and which one is the default.
- No command deletes an application that mise did not download, and every deletion is named in the dry run and in the receipt.
- An agent can read the session state, start a sign-in, and tell "a person must sign in" from every other failure by its error code.

### Remaining uncertainty

- The sign-in window is unproven. Two things have not been shown: that Apple's sign-in completes inside a WebKit view opened by a command-line program, and that the cookies the view holds are accepted by `download.developer.apple.com` when mise replays them. xcodes notes that Apple's download-authorization endpoint has answered 403 for Xcode archives since November 2022, so which cookies authorize a download is not documented anywhere.
- Passkeys and security keys probably do not work in the window. When asking Mark to choose a sign-in, I said Apple's page would handle them. WebKit limits WebAuthn in an embedded view to applications with specific entitlements, which a command-line program does not have [INFERENCE, not tested]. Password with a two-factor code is the expected path. Federated sign-in is expected to work and is also untested.
- How long a session lasts is unknown.
- pkg's release archives are not signed with a Developer ID. Whether macOS lets an upgraded pkg read the Keychain item its predecessor wrote, without asking the person again, is untested.
- The catalogue is maintained by volunteers. It can lag Apple by hours and can be unreachable.

## Existing baseline

pkg's [install](../crates/pkg-core/src/install.rs) and [remove](../crates/pkg-core/src/remove.rs) operations treat Xcode like any other tool: one declaration per tool name, written by `mise use -g <tool>@<version>` and removed by `mise unuse`. Preserve what they get right: declarations change only in the tools write target and only through mise, a declaration in another file is refused, every change has a dry run, and receipts report what happened. The identity by name alone is the compromise. It fits tools where one version is wanted and is wrong for Xcode, where a release and a beta side by side is the normal state.

The manual flow in the plugin's [Managing Xcode guide](https://github.com/markmals/mise-xcode/blob/main/guides/managing-xcode.md) is the behavior to wrap. Mark's restore guide for his dotfiles documents the same commands and was read as evidence of the workflow, not as a file to change.

xcodes shows which jobs a manager has to cover and which verbs people already know. Its placement in `/Applications`, its handling of the password and its root steps are not carried over here.

## Proposed solution

Give Xcode its own command group, because its needs have no place in the generic commands: betas, two versions at once, an Apple session, and applications that already exist. Record the result in the same mise config as everything else, so `mise bootstrap` still rebuilds the machine.

```text
pkg xcode login                  → a window shows Apple's sign-in; the session is saved to the Keychain
pkg xcode install latest         → 27.3 downloads, becomes the release, and replaces 27.2
pkg xcode install beta           → 27.4-beta.1 downloads into the beta slot; the default stays 27.3
pkg xcode use beta               → 27.4-beta.1 is the default in every shell
pkg xcode link /Applications/Xcode-27.2.0.app
                                 → that application is recorded as 27.2 without a download
pkg xcode list                   → your Xcodes, then the versions you could install
pkg xcode status --output json   → plugin, declaration, session and system state for an agent
```

The config afterwards says everything about the machine's Xcodes:

```toml
[tools]
xcode = [{ version = "27.4-beta.1", default = "true" }, { version = "27.3", default = "false" }]
```

Two rules keep it small. There is one release slot and one beta slot, so installing a version replaces the one in its slot, and installing the release of a beta you have removes that beta. The default is written as metadata on the entry, so nothing depends on which version is listed first.

## Detailed design

### Vocabulary

These terms are used the same way in commands, output, errors and documentation.

| Term            | Meaning                                                                                                                                                                                      |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Version         | The name mise lists, such as `27.2`, `27.3-beta.2` or `27.3-rc.1`. pkg never invents another format.                                                                                         |
| Channel         | `release` for a plain number, `beta` for a name with a suffix. Release candidates and the historical `dp`, `gm` and `gm-seed` names are in the beta channel.                                 |
| Installed Xcode | A version mise has registered. It is managed when mise downloaded it and owns the application, and linked when it was registered with `mise link` and the application belongs to the person. |
| Declared Xcode  | A version listed under `xcode` in `[tools]` of the tools write target.                                                                                                                       |
| Slot            | The release slot and the beta slot each hold at most one declared Xcode.                                                                                                                     |
| Default         | The declared Xcode that mise selects wherever no project pins its own. It is not the `xcode-select` path.                                                                                    |
| Session         | The credential that lets mise download from Apple. It is not the Apple Account or its password.                                                                                              |

### Commands

[The registry](../crates/pkg/src/registry.rs) gains eight entries under `xcode`. `pkg --help` lists the group as one everyday command.

| Command                                          | Effect                                            | What it does                                                 |
| ------------------------------------------------ | ------------------------------------------------- | ------------------------------------------------------------ |
| `pkg xcode list` (alias `ls`)                    | reads                                             | Your Xcodes and the versions available to install            |
| `pkg xcode install <version>`                    | modifies installed software and the global config | Downloads a version and records it in its slot               |
| `pkg xcode link <application>`                   | modifies the global config                        | Records an Xcode application that already exists             |
| `pkg xcode use <version>` (alias `select`)       | modifies the global config                        | Makes a declared Xcode the default                           |
| `pkg xcode remove <version>` (`rm`, `uninstall`) | modifies installed software and the global config | Takes a version out of the config and uninstalls it          |
| `pkg xcode status`                               | reads                                             | Reports the plugin, the declaration, the session, the system |
| `pkg xcode login`                                | modifies the stored session                       | Signs in to Apple and saves the session                      |
| `pkg xcode logout`                               | modifies the stored session                       | Deletes the saved session                                    |

Every modifying command accepts `--dry-run` with [the existing meaning](../crates/pkg/docs/packages.md#previewing-with-dry-run). All eight run mise as `mise -C <home>`, as every other command does.

`<version>` accepts an exact version. `install` also accepts two words:

- `latest` is the newest release this macOS can run. When the catalogue cannot be reached, it is the newest release mise lists, and pkg warns that the macOS requirement was not checked.
- `beta` is the newest beta whose number is greater than the newest release's. When there is none, the command fails with `NOT_FOUND` and names the newest release.

`use` and `remove` also accept `release` and `beta`, meaning the Xcode in that slot, and fail with `NOT_FOUND` when the slot is empty.

On Linux and Windows the commands exist and fail with `PRECONDITION_FAILED`, saying Xcode runs only on macOS. When no config declares an `xcode` plugin, every command except `login` and `logout` fails with `PRECONDITION_FAILED` and shows the `[plugins]` line to add. pkg does not install or pin the plugin. When the plugin is declared but not yet installed, pkg lets mise install it from that declaration.

### The declaration and the two slots

pkg writes the declaration only through `mise use -g`, naming every declared version in one command, because naming one version replaces the entry. The default is listed first and each entry carries its marker:

```toml
xcode = [{ version = "27.3", default = "true" }, { version = "27.4-beta.1", default = "false" }]
```

mise stores the option as a string. pkg reads the declaration from the tools write target itself.

- A hand-written declaration without markers is accepted. Its first version is the default, which is what mise does with it today, and pkg adds the markers the next time it changes the declaration.
- pkg refuses a declaration its rule cannot represent: two versions in one channel, a request that is not an exact version (`latest`, `27`), two entries marked as the default, or `xcode` declared in a global config file other than the write target. Every modifying command then fails with `PRECONDITION_FAILED`, names the file and the entry, and changes nothing. `pkg xcode status` reports the same problem. pkg never rewrites such a declaration to make it fit.

### Installing

`pkg xcode install <version>` checks, in this order, and fails before changing anything:

1. The version exists in mise's list. Otherwise `NOT_FOUND`, with the closest names.
2. This macOS can run it, according to the catalogue. Otherwise `PRECONDITION_FAILED`, naming the macOS it needs. When the catalogue cannot be reached, pkg warns that the requirement was not checked and continues.
3. The declaration is one pkg can change (above).
4. When the version has to be downloaded, a session exists and Apple accepts it. Otherwise `AUTH_REQUIRED` (below).

Then it decides what the slots will hold:

| Installing | Release slot       | Beta slot                                                                                       |
| ---------- | ------------------ | ----------------------------------------------------------------------------------------------- |
| A release  | Holds this release | Emptied when its beta belongs to this release or an older one (open question 2); otherwise kept |
| A beta     | Unchanged          | Holds this beta                                                                                 |

A version that is already declared and installed changes nothing, so installing twice is safe. A version that is declared but missing, as on a new machine, is installed and no slot changes. A version mise already has but no config declares is declared without a download. Installing a beta older than the current release is allowed, because it was asked for; the slot rule applies when a release is installed.

The default follows these rules. With no Xcode declared before, the new one is the default. When the Xcode it replaces or supersedes was the default, the new one is the default. Otherwise the default does not change.

The effects happen in an order that never leaves the machine without the Xcode it had:

1. mise installs the new version, with the session in that one child's environment. If this fails, nothing else has changed.
2. pkg rewrites the declaration. If this fails, the error says the new version is installed and undeclared.
3. pkg uninstalls each version that left a slot. A managed version is deleted with `mise uninstall`. A linked version loses its link and its application stays where it is (open question 3). A version another tracked config still requires stays installed (open question 1). Both are reported under `leftInstalled` with the reason.
4. pkg reads the result back from mise: the installed versions, and that mise's global environment names the default.

The receipt reports the version, whether it was downloaded or already present, the config file, each version removed or left installed and why, and the default. A dry run makes the same checks, including the session check, and reports the same plan.

### Linking an existing application

`pkg xcode link <application>` records an Xcode application that is already on the machine. It needs no session.

- The path must be an application with the bundle identifier `com.apple.dt.Xcode` that contains `xcodebuild`. Otherwise `INVALID_ARGUMENT`. pkg does not verify the application's signature; the person already runs it.
- pkg reads the application's build number and looks it up in the catalogue to find its version, then confirms mise lists that version. A build the catalogue does not know fails with `NOT_FOUND`, and an unreachable catalogue with `UNAVAILABLE`. Both suggest `--version <version>`, which states the name. pkg then checks that the application's own version number matches it.
- pkg registers it with `mise link` and declares it under the same slot rules as `install`.
- When mise already has that version from a different location, pkg fails with `PRECONDITION_FAILED` and names it. When it is already linked to the same application, pkg only declares it.

### Choosing the default

`pkg xcode use <version>` marks one declared Xcode as the default and the other as not. A version that is not declared fails with `NOT_FOUND` and suggests `pkg xcode install`; `use` never installs anything. Choosing the Xcode that is already the default changes nothing.

After writing, pkg reads mise's global environment and confirms it names that Xcode. When something outranks the global config, such as a `MISE_XCODE_VERSION` variable, the command fails with `PRECONDITION_FAILED` and says what mise selected instead.

### Removing

`pkg xcode remove <version>` takes the version out of the declaration and uninstalls it, with the same rules as step 3 of installing. It also removes a version that is installed but undeclared. When the removed Xcode was the default and another is declared, the other becomes the default and the receipt says so. A version that is neither installed nor declared fails with `NOT_FOUND`.

### Listing and status

`pkg xcode list` returns a collection in [the existing shape](../crates/pkg/docs/cli.md): `items`, `hasMore` and `nextCursor`, with `--limit` (10 by default) and `--cursor`. Declared Xcodes come first, the default before the other, then other installed versions, then the available ones, newest first. Each item has the version, its channel, its release date, the macOS it needs, whether this macOS can run it, whether it is installed, managed or linked, where its application is, and whether it is declared and the default. `--installed` keeps only installed versions and reads only local state. `--channel release|beta` keeps one channel. Without the catalogue, the macOS fields are null and pkg warns.

`pkg xcode status` is a read-only report that exits 0 whenever it can be produced. It contains:

- the plugin: whether it is declared and installed, its URL and ref, and the file that declares it;
- the declaration: the Xcode in each slot and the default;
- other installed versions, each with the config that requires it or a note that nothing does;
- the session: `signed-in`, `expired`, `signed-out`, or `unverified` when Apple or the catalogue cannot be reached; whether it comes from the Keychain or the environment; and when it expires, if the session says;
- the system, read and never changed: the `xcode-select` path and whether it is the default, and for each installed Xcode whether its license and first-launch setup are pending;
- `problems`: one entry per finding with a stable identifier, a sentence, and the command that fixes it. The identifiers cover a missing plugin, a declaration pkg cannot represent, a declared version that is not installed, an installed version no config declares, a default mise does not select, a missing or expired session, an `xcode-select` path that differs from the default, and pending license or first-launch setup.

### The Apple session

The session is the set of cookies an authenticated Apple Developer web session sends to `download.developer.apple.com`, as one `Cookie` header value. That is exactly what the plugin already accepts in `MISE_XCODE_COOKIE`.

#### Storage and supply

pkg keeps the session as one item in the login Keychain, under the service name `pkg-xcode-session`, and nowhere else. It never appears in a config file, a lockfile, a command line, a log, a receipt, an error, a dry run or `pkg context show`.

A `MISE_XCODE_COOKIE` already in pkg's environment wins over the Keychain, so a secret manager or CI keeps working and is reported as the `environment` source. Otherwise pkg adds the stored session as `MISE_XCODE_COOKIE` to the environment of a mise child that can download Xcode, and to no other: the install step of `pkg xcode install`, and `pkg install` or `pkg sync` when a declared Xcode is missing.

pkg checks a session by sending one `HEAD` request for an Xcode archive URL from the catalogue. Apple answering with the archive means signed in; a sign-in page or an error status means expired or rejected. This is the test the plugin applies before a download. When the catalogue cannot be reached there is no URL to ask for: `login` fails with `UNAVAILABLE`, `status` reports `unverified`, and `install` continues and leaves the check to the plugin.

#### Signing in as a person

`pkg xcode login` opens a window that shows Apple's Developer sign-in. The person signs in there, with whatever Apple's page asks for. pkg treats sign-in as complete once Apple accepts the window's session for a download, saves the session, closes the window, and says so.

- The Apple Account password and two-factor code are typed into Apple's page. pkg never receives them.
- The person can always see which site the window is showing, and every control in it can be reached from the keyboard.
- The window keeps nothing after it closes. The Keychain item is the only copy of the session.
- Closing the window cancels. The command also gives up after `--timeout <minutes>`, a whole number from 1 to 60 and 10 by default.
- When the stored session is still accepted, the command changes nothing and opens no window. A dry run reports the session's state and opens nothing.
- The window is part of the pkg executable on macOS and uses the system's WebKit. pkg bundles no browser engine and installs no helper application.

#### Signing in from an agent

The same command with `--output json` is the agent flow: the agent starts it and a person finishes it. It reads nothing from standard input and prints nothing secret. The window opens on the Mac's screen, the command waits up to the time limit, and it returns one JSON result with the session's state, source and expiry. An agent that only needs to know reads `session` from `pkg xcode status --output json`.

[The error contract](../crates/pkg/src/failure.rs) gains `AUTH_REQUIRED`, exit status 4, not retryable. A command that needs a session fails with it before installing or writing anything. `details.reason` is one of:

| Reason       | Meaning                                                                                        |
| ------------ | ---------------------------------------------------------------------------------------------- |
| `signed-out` | No session is stored or supplied.                                                              |
| `expired`    | Apple no longer accepts the session.                                                           |
| `rejected`   | Apple accepted the sign-in and refuses downloads, for example because an agreement is pending. |
| `cancelled`  | The person closed the window.                                                                  |
| `timed-out`  | The time limit passed.                                                                         |
| `no-display` | There is no screen to show the window on, as over SSH.                                         |

Its suggestion is `pkg xcode login`, or setting `MISE_XCODE_COOKIE` when there is no display.

#### Signing out

`pkg xcode logout` deletes the Keychain item and is safe to repeat. It does not end the session on Apple's side. A session supplied by the environment is not pkg's to delete, and the command says it is still in effect.

#### The gate

Implementation starts by proving the window: a signed-in window's session downloads an Xcode archive through mise. A negative result, or sign-in that cannot complete in the window, returns this proposal to Mark. It is not a licence to switch to another sign-in.

### The generic commands

- `pkg install xcode`, `pkg install xcode@<version>` and `pkg remove xcode@<version>` run the operations above, with `latest` when no version is given. `pkg remove xcode` without a version removes every declared Xcode, as it removes every version of any other tool. The `xcode` entry of [the install receipt](../crates/pkg-core/src/install.rs) gains `replaced`, the versions the slot rule took out; no other package has it.
- `pkg list` shows each declared Xcode as its own item.
- `pkg sync` supplies the stored session when a declared Xcode is missing. Without a session it behaves as today and reports the plugin's failure from mise.
- `pkg info`, `pkg where`, `pkg search`, `pkg outdated`, `pkg upgrade` and `pkg prune` do not change.

### Current sources this changes

- [`pkg-core` API contract](../crates/pkg-core/src/lib.rs): `Client` gains the list, install, link, use, remove and status operations and three session operations (state, store, clear), each with typed requests, results and errors, written before the code. `ErrorKind` gains the authentication case. `Settings` gains the catalogue URL and the session store, which is the login Keychain at the CLI's edge on macOS and absent elsewhere. Core never opens a window: the frontend captures a session and hands it to the store operation, which checks it with Apple before saving. A future native app can present its own web view against the same operations.
- [Registry](../crates/pkg/src/registry.rs), [CLI reference](../crates/pkg/docs/cli.md) and [error contract](../crates/pkg/src/failure.rs): the eight commands, the `replaced` field and `AUTH_REQUIRED`. The reference is regenerated.
- [Catalogs](../crates/pkg/docs/catalogs.md): Xcode Releases becomes the fourth catalog, read for build numbers, macOS requirements and archive URLs. mise's list stays the authority for which versions exist and what they are called. pkg fetches the catalogue when a command needs it and keeps no copy. `PKG_XCODE_RELEASES_API` overrides its URL, and `pkg context show` prints the value in effect.
- [Packages](../crates/pkg/docs/packages.md): `xcode` is the one tool whose version is part of what is declared, with a pointer to the new document.
- New `crates/pkg/docs/xcode.md`, compiled into the binary for `docs search`: the vocabulary, the slot rule, the declaration format, the session, and the Keychain service name. None of these has a home today.
- New `guides/managing-xcode.md`, written from this contract: signing in, the first install, moving to a new release, running a beta beside it, adopting an application, and what to do when a session expires. [README](../README.md) gains the commands and [Use pkg From Rust](../guides/rust-library.md) the new operations.
- [The CLI design skill](../.agents/skills/designing-human-first-clis/SKILL.md) and the platform rules in [AGENTS.md](../AGENTS.md) say no command prompts. That stays true of the terminal. Both gain one sentence: the sign-in window is the only interactive surface, it opens only for `pkg xcode login`, and no other command waits for a person.
- [VISION.md](../VISION.md) changes at acceptance: Xcode joins the minimum viable product's list, and the non-goal about keeping no database says that a saved credential is not a record of what is installed.
- Tests run the real binary against the fake mise, a local catalogue and a local stand-in for Apple's download host, with a session supplied by the environment. They do not touch the real Keychain or open a window. The window is exercised by a person in the preview.

### Required of the plugin

One change in mise-xcode, under its own proposal in that repository: a version entry marked `default = "false"` contributes no environment, so mise itself honors the marker. Until a plugin release has it, mise selects the first version listed, which is why pkg lists the default first. pkg's read-back after every change catches a disagreement either way. Nothing else in this proposal needs a plugin change.

## Compatibility

- An existing `xcode = "27.2"` keeps working. The first change pkg makes rewrites it with markers.
- An Xcode that is installed but undeclared, like the linked `27.1-beta.1` on Mark's Mac, appears under other installed versions. `pkg xcode install 27.1-beta.1` declares it without a download.
- `pkg install xcode@<version>` and `pkg remove xcode@<version>` change behavior. Before, the version was ignored on install, and remove uninstalled every unused Xcode.
- The JSON contract only grows: an optional `replaced` field on one receipt entry, one error code, and exit status 4.
- `MISE_XCODE_COOKIE` keeps working and takes precedence.
- The macOS builds link the system's WebKit. The Linux and Windows builds gain commands that refuse to run.

## Implications on adoption

Adoption needs macOS, mise 2026.11.9 or newer as today, and the plugin declared under `[plugins]`. Signing in needs a person at the Mac's screen once per session lifetime. For mise to honor the default marker on its own, the plugin needs the release described above.

Everything is reversible. The declaration is ordinary mise config that mise reads without pkg, `pkg xcode logout` removes the only thing pkg stores, and the current plugin ignores an option it does not know.

## Scope

- The eight `pkg xcode` commands, the slot rule, the default marker and the read-back.
- The session: the Keychain item, the sign-in window, the agent flow, `AUTH_REQUIRED`, and supplying the session to mise.
- Routing `pkg install` and `pkg remove` for `xcode`, and the session in `pkg sync`.
- Xcode Releases as a catalog.
- The specifications, documents and guide listed above, and fixture tests for every command in text and JSON.
- The sign-in proof, first.

### Out of scope

- System setup. Making `xcode-select` follow the default, accepting the license and installing first-launch components need root. They are the next proposal. Mark has already chosen how pkg gets root there: on a terminal `sudo` asks, and a JSON or piped run never waits. Here `status` only reports them.
- Simulator runtimes. A later proposal; the plugin's vision lists them as a non-goal today.
- Several versions of other tools. The slot rule made this unnecessary for Xcode. The wrong answer `pkg install <tool>@<version>` gives for an already declared tool is filed in Beads as `pkg-2wu`.
- Xcode in `pkg outdated` and `pkg upgrade`. pkg declares exact versions, so mise never reports Xcode as outdated. `pkg xcode list` shows what is newer, and `pkg xcode install latest` or `beta` moves a slot.
- Other ways to sign in: a terminal sign-in, an Apple Account password in pkg, a scripted two-factor flow, or a command that stores a session from standard input. The environment variable remains for those cases.
- Installing or pinning the plugin, and any change inside it.
- `mise lock` for Xcode, which downloads every archive to hash it.
- A richer progress display for the download. It shows what pkg shows for any tool install at the time; [proposal 0003](https://github.com/markmals/pkg/pull/3) owns that.

## Preview

- Repository shape: CLI.
- Artifact: the pkg executable built from this branch and installed to a private prefix on Mark's Mac, with its path in the readiness report. The repository is public, so nothing is uploaded.
- Reason: the sign-in window, the Keychain and a real Apple session exist only on a real Mac with a person at it. No fixture can stand in for them.
- Exercise, in text and in JSON: `pkg xcode list` and `pkg xcode status`; a dry run of `install`, `link`, `use` and `remove`; `pkg xcode login` in the window, then `status`; the same login started with `--output json`; an install attempted while signed out, to see `AUTH_REQUIRED`; `pkg xcode link` on an existing application; and `pkg xcode use` in both directions.
- One real download is the only proof that the session works end to end. It moves several gigabytes and replaces the Xcode in a slot, so Mark decides when to run it and with which version.
- Cleanup: remove the private executable, and run `pkg xcode logout` if the session should not outlive the review. Declarations made during the review stay or are removed with `pkg xcode remove`, at Mark's choice.

## Policies and decisions checked

- `policies/` and `decisions/` hold only their READMEs. None apply, and this proposal adds none.
- [VISION.md](../VISION.md), "mise decides, pkg translates": every install, link, uninstall and config write goes through a mise command. pkg reads the declaration and the catalogue itself, as it already reads configs and the Homebrew and App Store catalogs.
- VISION.md, "Edit only what you own": the declaration is changed only in the write target, and an application mise did not download is never deleted.
- VISION.md, "Ask for exactly what was typed" and no prompts: `install` replaces what is in a slot, which is more than installing. It is the rule Mark chose, it is stated in the dry run and the receipt, and the sign-in window opens only for the command that asks for it. This is the one place the proposal presses on a principle.
- VISION.md, non-goals: pkg adds one Keychain item and no record of what is installed. Managing system settings stays a non-goal here, which is why `xcode-select` is reported and not changed.

## Future directions

- System setup and simulator runtimes, each as its own proposal.
- Xcode in `pkg outdated` and `pkg upgrade`, once it is clear whether an everyday upgrade may download several gigabytes.
- The plugin reading the saved session itself, so `mise install` and `mise bootstrap` work without pkg after one `pkg xcode login`.
- A terminal sign-in as a fallback, if the window proves too limited for security keys or remote sessions.
- pkg declaring the plugin on a machine that lacks it, once the plugin publishes releases to pin.

## Alternatives considered

- Only teach the generic commands about versions. Nothing new to learn, but available versions, linking and the session have no place there, and it would change the contract of every tool. Mark chose the group plus routing for `xcode`.
- Any number of Xcodes, with the default listed first. This is what mise does unaided. Mark chose two slots and an explicit marker: the common case is one release and one beta, and order is too easy to change by accident.
- Sign in inside the terminal, as xcodes does. Proven, and it would allow a fully scripted flow. The password would pass through pkg, and Apple changes the protocol without notice. Mark chose Apple's own page.
- Keep the copied cookie as the only way. It is the problem being solved. It stays available through the environment.
- The system browser or `ASWebAuthenticationSession`. Both give the person their saved passwords and passkeys, and neither lets a program read the resulting session.
- Read versions only through mise. mise's list has no build numbers or macOS requirements, so `link` could not name an application and `install` could not refuse a version this macOS cannot run.
- Wrap the xcodes CLI. It works today, but it is a second manager with its own state, it installs into `/Applications`, and the request names the plugin as the backend.
- The App Store's Xcode for the release slot (`mas:497799835`). It needs no session and updates itself, but it offers only the newest release, cannot be pinned, and has no betas.
- A file with owner-only permissions for the session. Simpler than the Keychain and never asks the person anything, but it leaves a credential in plain text on disk.

## Open questions

- [NEEDS CLARIFICATION: 1. You asked for at most two Xcodes installed at once. A project's own `mise.toml` can pin a third, and mise installs it. When a version leaves a slot but another tracked config still requires it, should pkg keep it installed and report it (proposed), or uninstall it anyway?]
- [NEEDS CLARIFICATION: 2. Installing a release removes the beta of that same release, as you specified. Should it also remove a beta of an older release, such as `27.2-beta.3` when `27.3` is installed (proposed), or only the beta of the same version?]
- [NEEDS CLARIFICATION: 3. When a linked Xcode leaves its slot, should pkg drop the link and leave the application in `/Applications` (proposed, because pkg did not put it there), or move the application to the Trash as xcodes does?]
- [NEEDS CLARIFICATION: 4. An Xcode the plugin downloads lives in mise's install directory, where Spotlight does not find it. Should this proposal add a way to open it, such as `pkg xcode open [version]`; should the plugin's proposal place or link it in `/Applications`; or is opening it by path enough for now?]
