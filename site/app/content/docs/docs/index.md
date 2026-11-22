---
title: Getting started
description: What pkg does, what it needs, how to install it, and the first commands to run.
sidebar:
    order: 1
---

pkg installs software through [mise](https://mise.jdx.dev) and records every install in your global mise config, so `mise bootstrap` installs the same software on a new machine. When a website says `brew install wget`, you type `pkg install brew:wget`.

One vocabulary covers system packages and development tools. Homebrew formulae and casks, App Store apps, and other package managers' packages are recorded in `[bootstrap.packages]`. Tools such as `ripgrep` or `npm:prettier` are recorded in `[tools]`. Homebrew itself does not need to be installed, because mise installs Homebrew's bottles directly.

## What you need

- mise 2026.11.9 or newer. pkg runs mise for every install, removal, and update. On macOS and Linux, `pkg setup` and `pkg bootstrap` install mise when pkg finds none.
- macOS or Linux for the installer, `pkg setup`, and `pkg bootstrap`. On Windows, pkg comes from a release archive and runs the package commands with a mise you install yourself.

## Install and set up

On macOS or Linux:

```sh
curl -fsSL https://gotpkg.dev | sh
```

Run the command the installer prints to load the new `PATH` in this terminal, such as `source ~/.zshenv`. Then pick one:

- To restore the mise configuration you keep in a dotfiles repository, run `pkg bootstrap --from <url> --directory ~/.dotfiles --config-dir <directory>`.
- With nothing to restore, run `pkg setup`.

Pick before you run either, because `pkg setup` creates a global config that `pkg bootstrap --from` will not link over. [Install pkg](/docs/install) covers the installer's options, checksums, and building from source, and [Set up a machine](/docs/setup) covers both paths.

## A first session

Start with commands that only read, then preview a change before making it:

```sh
pkg context show                 # which pkg and mise run, and which files pkg writes to
pkg search ffmpeg                # find it in Homebrew, mise's registry, and the App Store
pkg info brew-cask:iina          # what it is, and whether it is installed
pkg install brew:wget --dry-run  # what installing would change, changing nothing
pkg install brew:wget            # install it and record it in your config
pkg list                         # everything your config declares, with versions
pkg update --dry-run             # what updating mise and everything else would change
```

`pkg search` prints the exact name to pass to `pkg install`, so you rarely need to work out a prefix yourself. Every command that changes something accepts `--dry-run`. No command prompts, except that `pkg bootstrap` in a terminal lets mise ask you for passwords and answers.

## Translate an install command

| You see                    | You type                       |
| -------------------------- | ------------------------------ |
| `brew install wget`        | `pkg install brew:wget`        |
| `brew install --cask iina` | `pkg install brew-cask:iina`   |
| `brew install owner/tap/x` | `pkg install brew:owner/tap/x` |
| `mas install 497799835`    | `pkg install mas:497799835`    |
| `npm install -g prettier`  | `pkg install npm:prettier`     |
| `mise use -g ripgrep`      | `pkg install ripgrep`          |

The prefix decides where the entry is recorded and which machines can install it. [Packages](/docs/packages#name-a-package) explains the rules, including what happens to an `apt:` package on a Mac.

## Where to go next

- [Install pkg](/docs/install): the installer and its options, `PATH`, checksums, Windows, and building from source.
- [Set up a machine](/docs/setup): restore your dotfiles with `pkg bootstrap`, or start fresh with `pkg setup`.
- [Packages](/docs/packages): find, install, remove, and update software.
- [Configuration](/docs/configuration): which files pkg edits, restoring another machine, and which mise pkg runs.
- [Scripts and agents](/docs/automation): JSON output, exit statuses, discovery commands, and the Rust library.
- [Command reference](/docs/reference): every command and its options on one page.

These pages summarize the project's [README](https://github.com/gotpkg/pkg/blob/main/README.md), the [documentation in `crates/pkg/docs`](https://github.com/gotpkg/pkg/tree/main/crates/pkg/docs), and the [CLI reference](https://github.com/gotpkg/pkg/blob/main/crates/pkg/docs/cli.md) generated from pkg's command registry. When they disagree, those files are correct.
