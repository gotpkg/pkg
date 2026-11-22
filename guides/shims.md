---
id: guide.shims
title: Point Agents at pkg with Shims
---

# Point Agents at pkg with Shims

## What it does

Agents reach for `brew install`, `apt install` or `npm install -g` because those are the commands they know. They have never heard of pkg, and what they install that way is in no mise config, so `mise bootstrap` does not bring it back. `pkg shims install` writes small executables named for those package managers. With their directory first on `PATH`, a global install through one runs nothing, exits with status 2, and writes the `pkg` command that does the same job to stderr. Everything else runs the real tool unchanged. The [package reference](../crates/pkg/docs/packages.md#shims-for-other-package-managers) lists every shim, what it refuses and what it suggests.

## How to use it

1. Run `pkg shims install --dry-run` to see what it would write, then `pkg shims install`. It creates the shims for this system in `~/.local/share/pkg/shims` (`%LOCALAPPDATA%\pkg\shims` on Windows) and prints the line that puts that directory first on `PATH`.
2. Add that line to the startup file your agents' shells read. pkg does not edit startup files. If your dotfiles deploy `~/.zshenv`, put the line there.
3. Start a new shell and run `pkg shims list`. Each shim should be installed and, for the names you use, resolve to the shim. A name that a directory earlier on `PATH` shadows is shown with the path that wins.
4. Run `mise` as `mise`, not by its full path, so your own `mise bootstrap` reaches the real package managers.
5. To undo it, delete the line and run `pkg shims remove`.

## Worked example

An agent that has never seen pkg runs `brew install --cask iina`. Nothing is installed, the command exits 2, and stderr says that this machine installs software with pkg, which records it in your mise config, and tells it to run `pkg install brew-cask:iina`. The agent runs that. The same agent's `npm install` in a project, `cargo build`, `brew --version` and `mise bootstrap` run as they always did.

## Limits and edge cases

- A shim only sees commands found through `PATH`. A tool run by its full path, and `sudo apt install` (sudo searches its own `PATH`), are not intercepted.
- In a shell that runs `mise activate`, the directories of the tools mise provides come first on `PATH`, ahead of the shims, so `npm` from a mise-managed node runs unshimmed there. A shell that only reads a startup file such as `~/.zshenv` keeps the shims first. `pkg shims list`, run in the shell you care about, shows which names a directory earlier on `PATH` shadows.
- A shim refuses a global install only when it can name the package without guessing. A command with a path, a URL, a requirement file or an option such as `--registry` that takes a separate value runs the real tool.
- `pip install` is refused only outside a virtual environment, and `npm install` only with `-g`; project installs are never refused.
- On a Mac without Homebrew, `command -v brew` finds the shim and `brew --version` exits 127 with a message that Homebrew is not installed.
- On Windows, a shim whose `pkg` is no longer on `PATH` exits 127. Take the directory off `PATH` or run `pkg shims remove`.
- `pkg shims install` writes nothing and fails with `PRECONDITION_FAILED` when a file that is not a shim already has a shim's name in the directory.
