---
title: Getting started
description: What pkg does, what it needs, and the first commands to run.
sidebar:
    order: 1
---

pkg installs software through [mise](https://mise.jdx.dev) and records every install in your global mise config, so `mise bootstrap` installs the same software on a new machine. When a website says `brew install wget`, you type `pkg install brew:wget`.

One vocabulary covers system packages and development tools. Homebrew formulae and casks, App Store apps, and other package managers' packages are recorded in `[bootstrap.packages]`. Tools such as `ripgrep` or `npm:prettier` are recorded in `[tools]`. Homebrew itself does not need to be installed, because mise installs Homebrew's bottles directly.

## What you need

- mise 2026.11.9 or newer. pkg runs mise for every install, removal, and upgrade.
- A pkg binary. No release has been published, so you build it from source with Cargo: [Install pkg](/docs/install) has the steps.

## A first session

Start with commands that only read, then preview a change before making it:

```sh
pkg context show                 # which mise pkg runs and which files it writes to
pkg search ffmpeg                # find it in Homebrew, mise's registry, and the App Store
pkg info brew-cask:iina          # what it is, and whether it is installed
pkg install brew:wget --dry-run  # what installing would change, changing nothing
pkg install brew:wget            # install it and record it in your config
pkg list                         # everything your config declares, with versions
```

`pkg search` prints the exact name to pass to `pkg install`, so you rarely need to work out a prefix yourself. Every command that changes something accepts `--dry-run`, and no command prompts.

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

- [Install pkg](/docs/install): build from a Git checkout and put `pkg` on your `PATH`.
- [Packages](/docs/packages): find, install, remove, and upgrade software.
- [Configuration](/docs/configuration): which mise config files pkg edits, and restoring another machine.
- [Scripts and agents](/docs/automation): JSON output, exit statuses, discovery commands, and the Rust library.
- [Command reference](/docs/reference): every command and its options on one page.

These pages summarize the project's [README](https://github.com/markmals/pkg/blob/main/README.md) and the [CLI reference](https://github.com/markmals/pkg/blob/main/crates/pkg/docs/cli.md) generated from pkg's command registry. When they disagree, those files are correct.
