---
id: proposal.0005
title: Package Manager Shims
authors: [Mark]
status: implemented
pull-request: https://github.com/gotpkg/pkg/pull/6
issues: []
supersedes: []
---

# Package Manager Shims

## Summary

Add `pkg shims`, which installs small executables named for the package managers mise can drive: `brew`, `apt`, `npm`, `pip`, `winget` and the rest. When an agent uses one to install software globally, the shim runs nothing and names the `pkg` command that does. Everything else passes through.

## Motivation

Agents reach for `brew install`, `apt install`, `npm install -g` and `mise use -g` because that is what they know. They do not know pkg exists. On this machine Homebrew is not installed, so `brew install wget` fails with "command not found", and nothing in that failure points an agent to pkg: it can install Homebrew, or bypass pkg with `mise use -g` (neither observed yet; both are what the failure invites). Either leaves software that the mise config does not declare, so `mise bootstrap` will not restore it, or adds a second package manager that [the vision](../VISION.md) rules out. `npm install -g` and `cargo install` work and install software nothing records.

The goal is that everything installed globally goes through pkg and mise, whichever manager the agent reaches for. The zsh `brew` function in the dotfiles does not help: it exists only in interactive zsh. Telling every agent about pkg in its instruction file works only for harnesses that read the file, and cannot stop the first mistaken command.

## Domain grounding

### Established context

- A mise shim is a small executable named after a tool. On Unix it is a symlink to the `mise` binary, which learns the tool from its own program name, and an unresolved shim runs the next executable of that name on `PATH` ([mise shims](https://mise.jdx.dev/dev-tools/shims.html)). The same page notes that shims make `command -v` succeed for tools that are not installed.
- mise's [bootstrap packages](https://mise.jdx.dev/bootstrap/packages/) drive these managers, each with the systems it runs on: `apk` (Alpine), `apt` (Debian, Ubuntu), `aur` (Arch, through `yay` or `paru`), `brew` and `brew-cask` (macOS, Linux), `choco`, `scoop` and `winget` (Windows), `dnf` (Fedora, RHEL), `flatpak` and `flatpak-user` (Linux), `macports` (macOS), `mas` (macOS), `nix` (Linux, macOS), `pacman` (Arch), `snap` (Linux) and `zypper` (SUSE). The tool backends in `mise backends ls` on mise 2026.11.12 include `npm`, `cargo`, `pypi`, `go`, `gem` and `dotnet`; `pipx:` is also accepted.
- pkg already maps other managers' commands to its own: the [naming table](../crates/pkg/docs/packages.md#naming-packages) pairs `brew install wget`, `npm install -g prettier` and `mise use -g ripgrep` with their `pkg install` forms.
- sudo replaces the caller's `PATH` with its `secure_path` when it looks up a command, and [sudo 1.9.16 enabled that by default](https://www.sudo.ws/posts/2024/09/why-sudo-1.9.16-enables-secure_path-by-default/). A shim directory on the user's `PATH` is not consulted for `sudo apt install`.

### Relevant constraints and principles

- An executable on `PATH` reaches every process, interactive or not. A shell function reaches only the shell that defines it. Observed on this machine on 2026-11-18: `zsh -c 'type brew'` finds nothing, and `zsh -ic 'type brew'` finds the function in `~/.config/zsh/brew.sh`. `brew` and `apt` are not installed here; `npm` is at `~/.vite-plus/bin/npm`.
- mise runs system managers by name through `PATH`. Observed on 2026-11-22 with mise 2026.11.12: with a `mas:` package declared and a stand-in `mas` first on `PATH`, `mise bootstrap packages status` ran `mas list --json` and `mas list` through it, as a direct child of `mise`. A stand-in `npm` first on `PATH` was not run by `mise install npm:cowsay@1.6.0` with mise-managed node; whether the tool backends reach a `PATH` `npm` when node is not mise's is unverified [INFERENCE]. A shim directory on the person's `PATH` would therefore refuse the person's own `mise bootstrap`, which is how a machine is restored.
- pkg owns no state and edits only files it owns ([vision](../VISION.md#design-principles)). It never edits a startup file to get itself onto `PATH`.
- A failing command's stderr is the one channel guaranteed to reach an agent that has never heard of pkg [INFERENCE].
- `pkg remove` uninstalls Homebrew packages and mise tools; for every other manager's packages it only takes the declaration out and lists them under `leftInstalled` ([Packages, Removing](../crates/pkg/docs/packages.md#removing)).

### Quality bar

An agent that has never seen pkg and runs `brew install wget` learns the exact command to run from that one failure, without reading any file. A person's `npm install` in a project, `cargo build` and `mise run` behave exactly as before, and so does a plain `mise bootstrap`. pkg's own work, which runs mise and, through it, other managers, never meets a shim. Removing pkg never leaves a manager unusable on Linux and macOS.

### Remaining uncertainty

How each agent harness builds its `PATH` is not verified; the design leaves putting the directory on `PATH` to the person. Linux installs through `apt:` need `sudo`, which pkg cannot prompt for yet (Beads item `pkg-4kn`). The Windows shims are exercised by CI only; the author has no Windows machine.

## Existing baseline

The dotfiles' `~/.config/zsh/brew.sh` is the closest prior art: it makes `brew install` and `brew uninstall` run mise and declare the package, and says which mise command does anything else. It translates and runs, in interactive zsh only, and duplicates logic that pkg now owns. Keep its lesson, that the person's habit of typing `brew install` should end up in the config. Do not keep its shape: shell-specific, silent about pkg, and unreachable by agents.

mise's shims are the baseline for the mechanism: a directory of tiny executables that either act or fall through to the next tool on `PATH`. Preserve the fall-through. Do not copy the symlink shape (see Alternatives considered).

## Proposed solution

`pkg shims install` writes one small script per package manager into a directory pkg owns. The person puts that directory first on `PATH` once, in the dotfiles. A shim recognises the invocations that install software globally and refuses them with the `pkg` equivalent. It hands everything else to the real tool, or reports a missing tool the way a shell would. Each system gets the shims for the managers mise runs there, plus the tool managers on every system.

```text
brew install --cask iina
→ nothing runs; exit 2; stderr says this machine installs software with pkg,
  which records it so `mise bootstrap` restores it, and to run: pkg install brew-cask:iina
```

An illustration of what the message must tell the reader, not its words:

```text
brew: not run. On this machine software is installed with pkg, which records it in your mise config.
  pkg install brew-cask:iina
Find a name with `pkg search <words>`. See `pkg --help` or `pkg cli search "<task>"`.
```

## Detailed design

- [`crates/pkg/src/registry.rs`](../crates/pkg/src/registry.rs) and `crates/pkg/src/commands/`: add a `shims` group with four registry entries, each with its adapter and text renderer. The feature lives in the CLI crate only; shims are environment integration for people and agents, not a package operation, so [pkg-core's API](../guides/rust-library.md) does not change. Which invocations are intercepted, and what is suggested, is a pure function of the shim name, the arguments and two environment variables, kept apart from the file and process code.
  - `pkg shims install`: creates the shim directory and writes the shims for this system. Idempotent. Overwrites only a file that carries the marker line `pkg-shim`. When any file with a shim's name lacks the marker, writes nothing and fails with `PRECONDITION_FAILED` naming the files. Accepts `--dry-run`. Reports per shim `created`, `updated` or `unchanged`. Never edits a startup file; it ends by printing the line that puts the directory first on `PATH` (below).
  - `pkg shims remove`: deletes the files that carry the marker, then the directory if empty. Accepts `--dry-run`.
  - `pkg shims list`: reads only. Reports the directory, whether it is on `PATH`, and for each shim whether the file exists and what that name resolves to on the current `PATH`: the shim, another executable (its path), or nothing. This is how a person sees that a shim is shadowed by an earlier directory. When the directory is not on `PATH`, text output prints the line that puts it first, for the shell named by `$SHELL` (bash, zsh or fish) on Unix and for PowerShell on Windows.
  - `pkg shims run <name> -- <arguments>`: what every shim calls. Listed in the reference, not in the everyday `--help` list; its registry entry declares that it runs another program instead of reading or modifying pkg's state. Its output is the refusal, which is pkg's usual error (text on stderr, or one JSON document with `--output json`, which a shim never passes), or the real tool's own; it has no result document.
- Shim directory: `$XDG_DATA_HOME/pkg/shims`, else `~/.local/share/pkg/shims`; on Windows `%LOCALAPPDATA%\pkg\shims`. Resolved from the process environment like the cache directory.
- Each shim is a one-line launcher, `exec pkg shims run <name> -- "$@"` in a POSIX shell script on macOS and Linux, and `pkg shims run <name> -- %*` in `<name>.cmd` on Windows, each with a `pkg-shim` marker comment. It names `pkg`, not a path, so a pkg that mise updates or moves keeps working. When `pkg` is not on `PATH`, the POSIX script runs the first executable of that name after its own directory, silently; the Windows script says pkg was not found, names `pkg shims remove` and the directory to take off `PATH`, and exits 127.
- Reading an invocation. After the subcommand, an argument that does not start with `-` is a package name. An invocation is not intercepted when it names no package; when any name looks like a path, URL or archive (starts with `.`, `/`, `~` or a drive letter, contains `://`, or ends `.tgz`, `.tar.gz`, `.zip`, `.deb`, `.rpm`, `.whl`) or does not start with a letter, digit or `@`; or when it uses an option its tool documents as taking a separate value, such as `--registry`, `--index-url`, `-r` or `--prefix`, because that value cannot be told from a package name. A suggestion is never built from a guess. Options a row below names decide the suggestion; all others are ignored. Two tools put something else among the arguments, and are read accordingly: `flatpak install` may begin with a remote name, which has no `.` where an application ID always has one (so a name without one is not refused either), and `winget install --id <id>` names the package in an option.
- What each shim intercepts, and where it is installed. Shims marked "All" are installed on every system and record in `[tools]`; the others are installed only on the systems mise runs that manager on, and record in `[bootstrap.packages]`. A removal is intercepted only where `pkg remove` itself uninstalls (Homebrew and tools); other managers' removals pass through, since the real tool is the only way to finish them.

  | Shim                       | Installed on | Install intercepted                                                                                                          | Removal intercepted                   | Suggested name                                                       |
  | -------------------------- | ------------ | ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- | -------------------------------------------------------------------- |
  | `brew`                     | macOS, Linux | `install`                                                                                                                    | `uninstall`, `remove`, `rm`           | `brew:<name>`; `brew-cask:<name>` with `--cask`                      |
  | `apt`, `apt-get`           | Linux        | `install`                                                                                                                    | no                                    | `apt:<name>`; `name=version` becomes `name@version`                  |
  | `dnf`                      | Linux        | `install`                                                                                                                    | no                                    | `dnf:<name>`                                                         |
  | `zypper`                   | Linux        | `install`, `in`                                                                                                              | no                                    | `zypper:<name>`                                                      |
  | `apk`                      | Linux        | `add`                                                                                                                        | no                                    | `apk:<name>`                                                         |
  | `pacman`                   | Linux        | `-S` or `--sync` naming packages, without a search, info, list, group or clean modifier (`-Ss`, `-Si`, `-Sl`, `-Sg`, `-Sc`)  | no                                    | `pacman:<name>`                                                      |
  | `yay`, `paru`              | Linux        | as `pacman`                                                                                                                  | no                                    | `aur:<name>`; the message adds `pacman:<name>` for official packages |
  | `flatpak`                  | Linux        | `install`                                                                                                                    | no                                    | `flatpak:<id>`; `flatpak-user:<id>` with `--user`                    |
  | `snap`                     | Linux        | `install`                                                                                                                    | no                                    | `snap:<name>`                                                        |
  | `nix`                      | Linux, macOS | `profile install`, `profile add`                                                                                             | no                                    | `nix:<reference>` as typed                                           |
  | `port`                     | macOS        | `install`                                                                                                                    | no                                    | `macports:<name>`                                                    |
  | `mas`                      | macOS        | `install`                                                                                                                    | no                                    | `mas:<id>`                                                           |
  | `winget`, `scoop`, `choco` | Windows      | `install`                                                                                                                    | no                                    | `winget:<id>`, `scoop:<name>`, `choco:<name>`                        |
  | `npm`, `pnpm`, `bun`       | All          | `install`, `i`, `add` with `-g` or `--global`                                                                                | `uninstall`, `rm`, `remove` with `-g` | `npm:<name>`                                                         |
  | `yarn`                     | All          | `global add`                                                                                                                 | `global remove`                       | `npm:<name>`                                                         |
  | `cargo`                    | All          | `install`, unless `--path`, `--git`, `--root` or `--list`                                                                    | `uninstall`                           | `cargo:<name>`                                                       |
  | `pip`, `pip3`              | All          | `install` while neither `VIRTUAL_ENV` nor `CONDA_PREFIX` is set, unless `--target`, `-t`, `--prefix`, `--root`, `-r` or `-e` | no                                    | `pypi:<name>`; `name==version` becomes `name@version`                |
  | `pipx`                     | All          | `install`                                                                                                                    | `uninstall`                           | `pypi:<name>`                                                        |
  | `uv`                       | All          | `tool install`                                                                                                               | `tool uninstall`                      | `pypi:<name>`                                                        |
  | `go`                       | All          | `install` of a `module@version`                                                                                              | no                                    | `go:<module>`, with `@version` unless it is `latest`                 |
  | `gem`                      | All          | `install`, unless `-i` or `--install-dir`                                                                                    | `uninstall`                           | `gem:<name>`                                                         |
  | `dotnet`                   | All          | `tool install` with `-g` or `--global`                                                                                       | `tool uninstall` with `-g`            | `dotnet:<name>`                                                      |
  | `mise`                     | All          | `use` with `-g` or `--global`                                                                                                | `unuse`, `rm`, `remove` with `-g`     | the names as typed                                                   |

  Installs suggest `pkg install <names>`, removals `pkg remove <names>`. Every other subcommand, including `update`, `upgrade`, `search`, `list`, `build` and `run`, passes through.
- A refusal writes the message to stderr and exits 2. It tells the reader that nothing ran, that software on this machine is installed with pkg and recorded so `mise bootstrap` restores it, the exact command, and where to learn more (`pkg search`, `pkg --help`, `pkg cli search`). It also says `--dry-run` previews. It does not mention how to bypass the shim.
- Anything not intercepted runs the first executable of that name on `PATH` after skipping the shim directory (paths compared with symlinks resolved) and any file that carries the marker. The real tool receives the arguments unchanged and its status is the shim's. On Unix pkg replaces itself with the tool, so signals and the terminal are the tool's. When there is no such executable, the shim says the tool is not installed on this machine and points to pkg, and exits 127; for `brew` it adds that Homebrew is not installed here and mise provides its packages.
- Never meeting a shim:
  - [`environment.rs`](../crates/pkg/src/environment.rs) resolves `mise` as the first `mise` on `PATH` outside the shim directory (`PKG_MISE` still overrides) and removes the shim directory from `PATH` in the environment of every process pkg starts.
  - The `mise` shim runs the real mise without the shim directory on `PATH`, so the person's own `mise bootstrap`, `mise install` and `mise use` reach the real managers. It keeps the directory on `PATH` for `exec`, `x`, `run`, `r`, `watch` and `w`, which run the caller's own commands, so those commands meet the shims.
- [`crates/pkg/docs/packages.md`](../crates/pkg/docs/packages.md): add a section on shims: the systems each shim is installed on, what is intercepted and what is suggested (the table above, in the source's own wording), exit statuses 2 and 127, what `sudo` does not reach, and the bypass (run the tool by full path, or take the directory off `PATH`). `docs/cli.md` is regenerated. [README](../README.md): one line under Use.
- New guide `guides/shims.md` (`id: guide.shims`): install the shims, add the printed line to the dotfiles' startup file that agents' shells read, see what an agent sees, run `mise` rather than its full path, and remove them. It is written from this contract, before the code.
- [VISION.md](../VISION.md) changes only at acceptance, by the human's Phase 5: its Beyond-the-minimum item about routing `brew install` through pkg gains the shims. No principle or non-goal changes.

## Compatibility

No pkg command changes. Shims exist only when a person runs `pkg shims install` and puts the directory on `PATH`. After that, any process with that `PATH` is affected, not only agents: a script that runs `npm install -g` or `cargo install <crate>` is refused, and `command -v brew` succeeds on a Mac without Homebrew, as mise documents for its own shims, so a script that probes for Homebrew that way will run `brew` and get exit 127. Everything else passes through with the real tool's arguments and status. `sudo apt install` is not affected: sudo uses its own `PATH`. The library API does not change.

## Implications on adoption

Nothing happens automatically. A person runs `pkg shims install` and adds the printed line to the startup file their agents' shells read; here that is `~/.zshenv`, deployed from the dotfiles, so the line goes in the dotfiles. macOS, Linux and Windows. Reversible: `pkg shims remove`, and delete the line.

## Scope

- The `pkg shims` entries, the shims for the managers above, the intercept table and the refusal, on macOS, Linux and Windows.
- Resolving `mise` and the child `PATH` so pkg never meets its own shims, and the `mise` shim's `PATH` for the real mise.
- The reference section, README line, generated reference and guide.
- Tests. The intercept function, table-driven: every row refuses with the right suggestion, and the cases above that pass through (project-local installs, paths and URLs, value options, `pip` in a virtual environment, `--root`). The real binary through a shim against a fake mise and a fake real manager: a non-intercepted call reaches the fake manager with its arguments and status; a missing tool exits 127; the `mise` shim removes the directory from the real mise's `PATH` except for `exec`; `install`, `list` and `remove`, with and without `--dry-run`; a conflicting file leaves everything untouched; pkg's mise calls skip the shim.

### Out of scope

- Editing shell startup files or `PATH`: the person's dotfiles own those, and the vision's "edit only what you own" holds. Folding the line into the installer or `pkg setup` waits on the first-machine proposal (0002).
- Translating the foreign command and running it: see Alternatives considered.
- Upgrades and updates (`brew upgrade`, `apt upgrade`, `winget upgrade`): `pkg` has its own commands for them, and the aim is to stop installs; they pass through.
- Removing packages of managers `pkg remove` cannot uninstall from: they pass through, so a package can still be removed with the real tool.
- Which Linux distribution a machine runs: a Linux machine gets every Linux shim, so `dnf` exists on Ubuntu and its refusal suggests `dnf:`. Narrowing by the managers mise reports available is possible later.
- Managers with no global install: `conda` (installs into an environment), `spm`, and the `github`, `aqua` and other backends.
- `sudo`'s own `PATH` (`secure_path`), and anything reached by a full path: neither is intercepted.
- The `sudo` limit on Linux installs (`pkg-4kn`).

## Preview

- Artifact: installable executable. `mise run package` writes `artifacts/pkg-<target>.tar.gz`.
- Reason: the reviewer extracts it to a scratch directory, runs `pkg shims install` with `XDG_DATA_HOME` pointing there, and in a shell with that directory first on `PATH` runs `brew install wget`, `npm install -g prettier`, `npm --version` and `mise bootstrap packages status`. Then `pkg shims list` and `pkg shims install --dry-run` on the real machine, in text and `--output json`. Delete the scratch directory when done; `pkg shims remove` undoes a real install. Linux and Windows behavior is covered by the tests and CI, not by the preview.

## Policies and decisions checked

None: `policies/` and `decisions/` hold only their README files. [VISION.md](../VISION.md) was read:

- Never surprise the person, refuse rather than do more than was asked: a shim refuses instead of translating.
- Edit only what you own: pkg writes only its shim directory and never a startup file.
- No second source of truth: shims are stateless scripts; `list` reads the filesystem.
- Non-goal "calling `brew`": pkg never runs `brew` for its own operations. A shim hands a person's own non-install `brew` command to a `brew` that already exists.

## Future directions

- Rows for more managers, as agents are seen reaching for them.
- `pkg setup` or the installer installing the shims and the `PATH` line, once the first-machine proposal lands.
- `pkg context show` reporting whether shims are active.
- Installing only the shims for managers mise reports available on a Linux machine.

## Alternatives considered

- Translate and run, as the dotfiles wrapper does: an agent's `brew install` would work unchanged. Not chosen, by the human: the agent never learns pkg, translating options is lossy, and a command that looks like plain `brew` would edit the dotfiles.
- Edit a startup file to put the directory on `PATH`, as the 0002 installer does: not chosen, by the human, because the dotfiles deploy `~/.zshenv`.
- Symlinks to the pkg executable that dispatch on program name, as mise does: no script. Not chosen: when mise provides pkg, the stable path is mise's own shim, which would treat `brew` as a mise tool, and a link to the versioned binary goes stale on update.
- Take the shim directory off `PATH` only for pkg's own children, as the first draft did: a person's plain `mise bootstrap` runs `mas install`, `flatpak install` and the others through `PATH` and would be refused.
- Recognise a call from mise by its parent process, so the shim passes it through: needs process lookup on three systems, and sudo sits between mise and `apt-get`.
- Instruction text in each agent's instruction file: complements shims but reaches only harnesses that read it and cannot prevent the first mistaken command.
- Extending the zsh function: does not reach non-interactive shells.
- Install every shim on every system: a Mac would have a `winget` and an `apt` that suggest declarations for managers it cannot run.

## Open questions

None. The human answered the three questions of the first draft on 2026-11-22: shims refuse and name the pkg command; `pkg shims` prints the `PATH` line and never edits a startup file; and the set grows to every manager mise can provide, including `pip`, `pipx`, `uv`, `go`, `gem`, `dotnet`, `dnf`, `pacman`, `yay`, `paru`, `apk`, `port`, `flatpak`, `snap`, `winget`, `scoop`, `choco` and `mas`. `zypper` and `nix` were added because mise provides them, and Windows support follows from `winget`, `scoop` and `choco`.

The human approved the completed implementation on 2026-11-22 and asked to merge it and clean up. The accepted implementation and final records are committed in the feature pull request before merge. Delivery is merge-only; no release is authorized, so the change file waits for the next release pull request. The deferred Windows `PATH` ordering limit is Beads `pkg-5fn`.
