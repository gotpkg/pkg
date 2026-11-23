---
title: Packages
description: Name, find, install, remove, and update packages and tools, previewing each change with --dry-run.
sidebar:
    order: 4
---

pkg names packages the way mise does, as `manager:name`, and records each one in your global mise config. This page covers the daily commands. The [package contract](https://github.com/gotpkg/pkg/blob/main/crates/pkg/docs/packages.md) in the repository has the full rules.

## Name a package

| You see                    | You type                       | Where it is recorded   |
| -------------------------- | ------------------------------ | ---------------------- |
| `brew install wget`        | `pkg install brew:wget`        | `[bootstrap.packages]` |
| `brew install --cask iina` | `pkg install brew-cask:iina`   | `[bootstrap.packages]` |
| `brew install owner/tap/x` | `pkg install brew:owner/tap/x` | `[bootstrap.packages]` |
| An App Store app           | `pkg install mas:<app id>`     | `[bootstrap.packages]` |
| `npm install -g prettier`  | `pkg install npm:prettier`     | `[tools]`              |
| A tool in mise's registry  | `pkg install ripgrep`          | `[tools]`              |

The prefix decides what kind of entry pkg makes:

- A prefix that names one of mise's tool backends makes a tool in `[tools]`. `npm`, `cargo`, `github`, `aqua`, `go`, and `pipx` are backends; `mise backends ls` lists all of them.
- No prefix also makes a tool: a bare name such as `ripgrep` comes from mise's registry, exactly as `mise use ripgrep` reads it.
- Any other prefix is a bootstrap package manager, recorded in `[bootstrap.packages]`: `brew`, `brew-cask`, `mas`, `apt`, `dnf`, `flatpak`, `winget`, and the others mise supports.

A package manager has to exist on the machine for mise to install with it. When you install a package whose manager this machine lacks, such as `apt:curl` on macOS, pkg records it in your config and does not install it. Another machine that has the manager installs it when it applies the config.

Homebrew keeps versions in its names (`brew:postgresql@17`), so there `@` is part of the formula or cask name. For every other package, `@<version>` pins a version: `node@24`, `apt:curl@8.5.0-2`. A tool without a version is recorded as `latest`.

## Find a package

`pkg search` looks through Homebrew's formulae and casks, mise's registry, and the App Store, and prints the exact package to pass to `pkg install`, its version, and whether your config already declares it.

```sh
pkg search ffmpeg                              # every source
pkg search video player --manager brew-cask    # casks only; several words need no quotes
pkg info brew:wget                             # one formula: description, homepage, version, dependencies
pkg info mas:937984704                         # one App Store app
```

The first search downloads Homebrew's catalog, about 7 MB, and later searches download it again once the copy is a day old. Formulae in third-party taps are not in Homebrew's catalog, so search does not find them. The repository's [catalog notes](https://github.com/gotpkg/pkg/blob/main/crates/pkg/docs/catalogs.md) cover caching and what happens when a catalog is unreachable.

## Install

Preview first, then install. You can name several packages from different sources in one command:

```sh
pkg install brew:wget brew-cask:iina --dry-run
pkg install brew:wget brew-cask:iina
pkg install ripgrep npm:prettier
```

pkg handles each package in turn and reports what it did:

- `unchanged`: already declared and installed. Installing twice is safe.
- `installed`: declared somewhere already but missing, so mise installs it where it is declared.
- `added`: not declared, so pkg records it in your config and mise installs it.
- `declared`: recorded, but not installed, because this machine lacks the package's manager.

When mise fails, pkg takes the failed package's new entry back out of your config and stops. The error names the packages installed before it, and a broken entry never stops a later `mise bootstrap`.

### Third-party Homebrew taps

mise finds a tap `owner/name` at `https://github.com/owner/homebrew-name` without any setup, so `pkg install brew:owner/name/formula` works directly. Only a tap whose repository lives at another GitHub URL needs its URL recorded first:

```sh
pkg tap add acme/tools https://github.com/acme/brew-tools.git --dry-run
pkg tap add acme/tools https://github.com/acme/brew-tools.git
pkg install brew:acme/tools/<formula>
```

`pkg tap list` shows the recorded URLs, and `pkg tap remove acme/tools` forgets one. Removing a tap leaves its packages declared and installed.

## Remove

```sh
pkg remove brew:wget --dry-run
pkg remove brew:wget
```

mise has no command that uninstalls a single Homebrew package. Its `mise bootstrap packages prune` uninstalls every formula and cask that no config declares. So for a Homebrew package, `pkg remove` takes the entry out of your config, then prunes, and reports everything uninstalled, dependencies included.

Before it touches anything, pkg checks that the prune would remove only what you named. When other undeclared Homebrew packages are already installed, a prune would take them too, so `pkg remove` refuses with `PRECONDITION_FAILED`, lists them, and changes nothing. Then:

```sh
pkg prune --dry-run    # see the undeclared packages
pkg prune              # uninstall them
pkg install brew:jq    # or keep one by declaring it
```

Other kinds of package are removed differently:

- Tools are removed with `mise unuse`, which also uninstalls versions no other config uses.
- mise cannot uninstall App Store apps or other managers' packages. pkg takes them out of your config and lists them under `leftInstalled` for you to delete.
- A formula that a declared package still depends on stays installed and is listed under `leftInstalled`.
- A package declared in a config file pkg does not edit fails with `PRECONDITION_FAILED` and names the file. [Configuration](/docs/configuration#files-pkg-does-not-edit) explains why.

## Keep packages up to date

```sh
pkg outdated            # what has a newer version, mise first
pkg update --dry-run    # what an update would change
pkg update              # mise itself, then everything else
pkg update brew:wget    # only the packages you name
```

`pkg outdated` compares Homebrew formulae and casks, App Store apps, global tools, and mise itself against their newest versions. Casks mise will not update, such as apps that update themselves, appear under `skipped`, and so does a newer mise that a package manager installed.

`pkg update` lists each version that changed. With no names, it first runs `mise self-update`, so packages and tools then update with the new mise. If that step fails, pkg stops before changing anything else. A mise installed by Homebrew, apt, or another package manager refuses to update itself, so pkg leaves it to that package manager and lists it under `skipped`. mise also waits until a release has reached its minimum release age, a day unless you set `minimum_release_age`, so `pkg outdated` can list a mise release that `pkg update` leaves for later.

Once pkg is [handed to mise](/docs/setup#hand-pkg-to-mise), `pkg update` updates pkg too. A package that is declared but not installed is left to [`pkg sync`](/docs/configuration#restore-another-machine).

## Find where something is installed

```sh
pkg where brew:wget    # the formula's opt directory
pkg where jq           # a tool's install directory
```

Keg-only formulae need their `opt` directory on your `PATH` by hand, and `pkg where` prints it. Casks, App Store apps, and other managers' packages have no such directory.

## Follow the progress

In a terminal, `pkg install`, `remove`, `update`, `sync`, and `prune` show a live display while they work, dry runs included. It names the step pkg is on and how long it has taken, the package being worked on and its place in the request (`2 of 3`), and every item mise is working on, dependencies included, with the last thing mise said about it, such as `download` or `link`. Bars show how much is done wherever the total is known: formulae against the plan mise gave for the package, tools against the number mise is installing, and a tool download's bytes and rate. `remove` and `prune` list the packages mise is about to uninstall.

Every figure is the last one mise reported, and pkg never estimates the time left. mise reports no byte counts for Homebrew downloads when pkg runs it, so formulae and casks show their phase only. To know how many formulae a package brings, `pkg install` asks mise for its plan just before installing it, which takes a fraction of a second.

When the command ends, the display disappears. One line per finished formula, cask, or tool stays on stderr, with every warning, error, and line pkg does not recognize, and then pkg's result. mise's version lookups, such as the line per tool in your config that mise prints before `pkg update` installs anything, leave nothing behind unless one fails. If a step fails, pkg first writes the progress lines it held back during that step.

For plain lines instead, redirect stderr (`pkg install brew:wget 2>&1 | cat`) or set `CI`. The display is also off with `--output json` and when `TERM` is `dumb`. The repository's [package contract](https://github.com/gotpkg/pkg/blob/main/crates/pkg/docs/packages.md#progress-in-a-terminal) has the full rules.

## Preview with dry run

`--dry-run` works on every command that changes something: `install`, `remove`, `update`, `sync`, `prune`, `tap add`, `tap remove`, `setup`, and `bootstrap`. A dry run makes the same checks and decisions as the real run, including refusals, and installs, uninstalls, and writes nothing. With `--output json`, its receipt has `status` set to `dry-run`. A bootstrap preview has [limits of its own](/docs/setup#preview-a-bootstrap), because a configuration can run commands.
