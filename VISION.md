---
title: A Vision for pkg
updated: 2026-11-17
---

# A Vision for pkg

## Introduction

pkg is a package manager front end for one person's machines. mise already installs Homebrew formulae and casks, App Store apps, other system packages and developer tools, and records them in a config that rebuilds a machine with `mise bootstrap`. But using it day to day means remembering long commands (`mise -C ~ bootstrap packages use -g --yes brew-cask:iina`) and translating every `brew install` instruction by hand. pkg makes the everyday operations short and familiar: install, remove, find, describe, list, and keep current, with every change recorded in the dotfiles config mise reads. Its main user is a person at a terminal; agents and scripts drive the same commands with `--output json`.

## Goals

- Turn any install instruction into one short command that installs the software and records it, so the config always describes the machine.
- Cover the whole life of a package from one tool: find it, learn what it is, install it, see what is outdated, upgrade it, and remove it cleanly.
- Never surprise the person: say what changed and where, preview any change with `--dry-run`, and refuse rather than do more than was asked.
- Keep mise the single owner of installation and of the config, so pkg adds no state, no second source of truth, and nothing to migrate.

## Minimum viable product

On a Mac set up with Mise Bootstrap, `pkg install <manager>:<name>` installs any bootstrap package or tool and declares it in the file mise writes to; `pkg remove` takes it out and uninstalls it without touching anything else; `pkg search` and `pkg info` find and describe packages from Homebrew, mise's registry and the App Store; `pkg list`, `pkg outdated`, `pkg upgrade` and `pkg sync` keep the machine current. Each has readable text output and an equivalent JSON contract.

## Design principles

- **Human-first, agent-complete** — text is the default and `--help` shows only the everyday commands; the JSON contract and the discovery commands are complete but opt-in.
- **mise decides, pkg translates** — installation, version resolution and config writes go through mise's own commands; pkg edits a config file only to remove an entry mise cannot remove itself.
- **Edit only what you own** — pkg writes to mise's write target and refuses to change a declaration in any other file, such as one shared with other machines.
- **Ask for exactly what was typed** — no prompts; a command that would do more than asked, like a prune that would take undeclared packages, refuses and explains.

When principles conflict, safety of the config and of installed software comes before brevity.

## Beyond the minimum

- Routing the shell's `brew install` and `brew uninstall` through pkg, so an instruction pasted unchanged works; today a zsh wrapper in the dotfiles does this separately.
- Searching third-party Homebrew taps, which Homebrew's API does not cover.
- Cleaning up superseded Homebrew kegs, which mise does not remove after an upgrade.

## Non-goals

- Replacing mise's installers or keeping a database of its own — mise already owns both, and a second record would drift.
- Managing dotfiles, services or system settings — `mise bootstrap` does that, and pkg's commands stay about packages.
- Installing Homebrew itself or calling `brew` — on these machines Homebrew is not installed; mise pours bottles directly.

## Revision history

| Date       | Change               | Related proposal |
| ---------- | -------------------- | ---------------- |
| 2026-11-17 | Created this vision. | N/A              |
