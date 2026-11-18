---
id: proposal.0005
title: Package Manager Shims
authors: [Mark]
status: draft
pull-request: https://github.com/gotpkg/pkg/pull/6
issues: []
supersedes: []
---

# Package Manager Shims

## Summary

Add `pkg shims`, which installs small executables named `brew`, `apt`, `npm`, `cargo`, `mise` and similar. When an agent uses one to install or remove software globally, the shim runs nothing and says which `pkg` command does it. Every other invocation passes through to the real tool.

## Motivation

Agents reach for `brew install`, `apt install`, `npm install -g` and `mise use -g` because that is what they know. They do not know pkg exists. On this machine Homebrew is not installed, so `brew install wget` fails with "command not found", and nothing in that failure points an agent to pkg: it can install Homebrew, or bypass pkg with `mise use -g` (neither observed yet; both are what the failure invites). Either leaves software that the mise config does not declare, so `mise bootstrap` will not restore it, or adds a second package manager that [the vision](../VISION.md) rules out. `npm install -g` and `cargo install` work and install software nothing records.

The zsh `brew` function in the dotfiles does not help: it exists only in interactive zsh. Telling every agent about pkg in its instruction file works only for harnesses that read the file, and cannot stop the first mistaken command.

## Domain grounding

### Established context

- A mise shim is a small executable named after a tool. On Unix it is a symlink to the `mise` binary, which learns the tool from its own program name, and an unresolved shim runs the next executable of that name on `PATH` ([mise shims](https://mise.jdx.dev/dev-tools/shims.html)). The same page notes that shims make `command -v` succeed for tools that are not installed.
- mise's bootstrap drives system package managers itself. Strings in the installed mise binary include `sudo apt-get install -y`, `sudo dnf install -y`, `brew install --cask` and `mas install` (observed; that they run through `PATH` is [INFERENCE]).
- pkg already maps other managers' commands to its own: the [naming table](../crates/pkg/docs/packages.md#naming-packages) pairs `brew install wget`, `npm install -g prettier` and `mise use -g ripgrep` with their `pkg install` forms.

### Relevant constraints and principles

- An executable on `PATH` reaches every process, interactive or not. A shell function reaches only the shell that defines it. Observed on this machine on 2026-11-18: `zsh -c 'type brew'` finds nothing, and `zsh -ic 'type brew'` finds the function in `~/.config/zsh/brew.sh`. `brew` and `apt` are not installed here; `npm` is at `~/.vite-plus/bin/npm`.
- pkg owns no state and edits only files it owns ([vision](../VISION.md#design-principles)). It never edits another file to get itself onto `PATH` except where a proposal says so.
- A failing command's stderr is the one channel guaranteed to reach an agent that has never heard of pkg [INFERENCE].

### Quality bar

An agent that has never seen pkg and runs `brew install wget` learns the exact command to run from that one failure, without reading any file. A person's `npm install` in a project, `cargo build` and `mise run` behave exactly as before. pkg's own work, which runs mise and, through it, other managers, never meets a shim.

### Remaining uncertainty

How each agent harness builds its `PATH` is not verified; the design leaves putting the directory on `PATH` to the person. Linux installs through `apt:` need `sudo`, which pkg cannot prompt for yet (Beads item `pkg-4kn`), so a redirect to `pkg install apt:curl` can fail for that reason.

## Existing baseline

The dotfiles' `~/.config/zsh/brew.sh` is the closest prior art: it makes `brew install` and `brew uninstall` run mise and declare the package, and says which mise command does anything else. It translates and runs, in interactive zsh only, and duplicates logic that pkg now owns. Keep its lesson, that the person's habit of typing `brew install` should end up in the config. Do not keep its shape: shell-specific, silent about pkg, and unreachable by agents.

mise's shims are the baseline for the mechanism: a directory of tiny executables that either act or fall through to the next tool on `PATH`. Preserve the fall-through. Do not copy the symlink shape (see Alternatives considered).

## Proposed solution

`pkg shims install` writes one small script per package manager into a directory pkg owns. The person puts that directory first on `PATH` once. A shim recognises the few invocations that install or remove software globally and refuses them with the `pkg` equivalent. It hands everything else to the real tool, or reports a missing tool the way a shell would.

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

- [`crates/pkg/src/registry.rs`](../crates/pkg/src/registry.rs) and `crates/pkg/src/commands/`: add a `shims` group with four registry entries, each with its adapter and text renderer. The feature lives in the CLI crate only; shims are environment integration for people and agents, not a package operation, so [pkg-core's API](../guides/rust-library.md) does not change.
  - `pkg shims install`: creates the shim directory and writes the shim scripts below. Idempotent. Overwrites only a file that carries the shim marker line; any other file with a shim's name is left alone and reported as a conflict. Accepts `--dry-run`. Reports per shim `created`, `updated`, `unchanged` or `conflict`.
  - `pkg shims remove`: deletes the files that carry the marker, then the directory if empty. Accepts `--dry-run`.
  - `pkg shims list`: reads only. Reports the directory, whether it is on `PATH`, and for each shim name whether the script exists and what that name resolves to on the current `PATH`: the shim, another executable (its path), or nothing. This is how a person sees that a shim is shadowed by an earlier directory. When the directory is not on `PATH`, text output prints the line that puts it first, for the shell named by `$SHELL` (bash, zsh or fish).
  - `pkg shims run <name> -- <arguments>`: what every shim script calls. Listed in the reference but not in the everyday `--help` list. Has no result document: its output is the refusal or the real tool's own.
- Shim directory: `$XDG_DATA_HOME/pkg/shims`, else `~/.local/share/pkg/shims`, resolved in [`environment.rs`](../crates/pkg/src/environment.rs) like the cache directory. `pkg shims` on Windows fails with `PRECONDITION_FAILED`; macOS and Linux only.
- Each shim is a POSIX shell script: a marker comment, then `exec pkg shims run <name> -- "$@"`, guarded so that when `pkg` is not on `PATH` it says so on stderr and exits 127. It names `pkg` rather than a path, so a pkg that mise updates or moves keeps working.
- What each shim intercepts. A name that is positional and not an option counts as a package name; an invocation naming a local path, a URL or an archive (starts with `.`, `/` or `~`, contains `://`, ends `.tgz`, `.deb` or `.rb`) is not intercepted. An invocation with no package name is not intercepted. Options are ignored when building the suggestion, except `--cask`.

  | Shim             | Intercepts                                                                             | Suggested name                                     |
  | ---------------- | -------------------------------------------------------------------------------------- | -------------------------------------------------- |
  | `brew`           | `install`; `uninstall`, `remove`, `rm`                                                 | `brew:<name>`, or `brew-cask:<name>` with `--cask` |
  | `apt`, `apt-get` | `install`; `remove`, `purge`                                                           | `apt:<name>`                                       |
  | `npm`            | `install`, `i`, `add`; `uninstall`, `un`, `rm`, `remove`; each with `-g` or `--global` | `npm:<name>`                                       |
  | `pnpm`, `bun`    | `add`, `install`, `i`; `remove`, `rm`, `uninstall`; each with `-g` or `--global`       | `npm:<name>`                                       |
  | `yarn`           | `global add`; `global remove`                                                          | `npm:<name>`                                       |
  | `cargo`          | `install`, `uninstall`, unless `--path`, `--git` or `--list` is present                | `cargo:<name>`                                     |
  | `mise`           | `use` with `-g` or `--global`                                                          | the name as typed                                  |

  Installs suggest `pkg install <names>`, removals `pkg remove <names>`.
- A refusal writes the message to stderr and exits 2. It tells the reader that nothing ran, that software on this machine is installed with pkg and recorded so `mise bootstrap` restores it, the exact command, and where to learn more (`pkg search`, `pkg --help`, `pkg cli search`). It also says `--dry-run` previews. It does not mention how to bypass the shim.
- Anything not intercepted runs the first executable of that name on `PATH` after skipping the shim directory and any file that carries the marker, with the arguments unchanged and the real tool's exit status. When there is none, the shim writes that the tool is not installed on this machine, points to pkg, and exits 127. For `brew` the message adds that Homebrew is not installed here and mise provides its packages.
- Never meeting a shim: [`environment.rs`](../crates/pkg/src/environment.rs) resolves `mise` as the first `mise` on `PATH` that is not in the shim directory (`PKG_MISE` still overrides), and removes the shim directory from `PATH` in the environment of every process pkg starts. mise and the managers it drives then reach the real ones.
- [`crates/pkg/docs/packages.md`](../crates/pkg/docs/packages.md): add a section on shims, listing the intercepted commands, the suggested names, exit statuses 2 and 127, and the bypass: run the tool by full path, or take the directory off `PATH`. `docs/cli.md` is regenerated. [README](../README.md): one line under Use.
- New guide `guides/shims.md`: install the shims, put the directory first on the `PATH` that agents' shells read, see what an agent sees, and remove them. It is written from this contract, before the code.
- [VISION.md](../VISION.md) changes only at acceptance, by the human's Phase 5: its Beyond-the-minimum item about routing `brew install` through pkg gains the shims. No principle or non-goal changes.

## Compatibility

No pkg command changes. Shims exist only when a person runs `pkg shims install` and puts the directory on `PATH`. After that, any process with that `PATH` is affected, not only agents: a script that runs `npm install -g` or `cargo install <crate>` is refused, and `command -v brew` succeeds although Homebrew is not installed, as mise documents for its own shims. Everything else passes through with the real tool's arguments and status. A shimmed manager fails with a clear message while `pkg` is not on `PATH`. The library API does not change.

## Implications on adoption

Nothing happens automatically. A person runs `pkg shims install` and adds one line to the startup file that their agents' shells read; here that is `~/.zshenv`, deployed from the dotfiles. macOS and Linux only. Reversible: `pkg shims remove`, and delete the line.

## Scope

- The `pkg shims` entries, the shim scripts, the intercept table and the refusal.
- Resolving `mise` and the child `PATH` so pkg never meets its own shims.
- The reference section, README line, generated reference and guide.
- Tests that run the real binary through each shim against a fake mise and a fake real manager: each row refuses with the right suggestion; a non-intercepted call reaches the fake manager with its status; a missing tool exits 127; `install`, `list` and `remove` with and without `--dry-run`; a conflicting file is left alone; pkg's mise calls skip the shim.

### Out of scope

- Editing shell startup files or `PATH`: the person's dotfiles own those, and the vision's "edit only what you own" holds. Folding the line into the installer or `pkg setup` waits on the first-machine proposal (0002).
- Translating the foreign command and running it: see Alternatives considered.
- Other managers (`pip`, `pipx`, `gem`, `go`, `dnf`, `pacman`, `winget`): each is a row in the table, to add when needed.
- Per-name selection of shims, translating anything but package names, Windows, and generated agent instruction files.
- The `sudo` limit on Linux installs (`pkg-4kn`).

## Preview

- Artifact: installable executable. `mise run package` writes `artifacts/pkg-<target>.tar.gz`.
- Reason: the reviewer extracts it to a scratch directory, runs `pkg shims install` with `XDG_DATA_HOME` pointing there, and in a shell with that directory first on `PATH` runs `brew install wget`, `npm install -g prettier` and `npm --version`. Then `pkg shims list` and `pkg shims install --dry-run` on the real machine, in text and `--output json`. Delete the scratch directory when done; `pkg shims remove` undoes a real install.

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

## Alternatives considered

- Translate and run, as the dotfiles wrapper does: an agent's `brew install` would work unchanged. Not chosen: the agent never learns pkg, translating options is lossy, and a command that looks like plain `brew` would edit the dotfiles. See the open question.
- Symlinks to the pkg executable that dispatch on program name, as mise does: no script. Not chosen: when mise provides pkg, the stable path is mise's own shim, which would treat `brew` as a mise tool, and a link to the versioned binary goes stale on update.
- Instruction text in each agent's instruction file: complements shims but reaches only harnesses that read it and cannot prevent the first mistaken command.
- Extending the zsh function: does not reach non-interactive shells.

## Open questions

- [NEEDS CLARIFICATION: Should a shim refuse and name the pkg command (recommended: the agent learns pkg from one failure, and nothing happens that was not asked for), or translate the command and run pkg itself?]
- [NEEDS CLARIFICATION: Should `pkg shims install` only print the line that puts the directory on `PATH` (recommended: your dotfiles deploy `~/.zshenv`), or edit a startup file as the 0002 installer does?]
- [NEEDS CLARIFICATION: Is the first set right: `brew`, `apt`, `apt-get`, `npm`, `pnpm`, `yarn`, `bun`, `cargo`, `mise`?]
