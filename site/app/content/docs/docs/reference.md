---
title: Command reference
description: Every pkg command with its arguments and options, and where to find the complete generated reference.
sidebar:
    order: 7
---

This page indexes every command. The complete [CLI reference](https://github.com/gotpkg/pkg/blob/main/crates/pkg/docs/cli.md), generated from pkg's command registry, owns each command's full description, errors, and JSON result schema. The same reference ships inside the binary: `pkg docs show docs/cli.md` prints it, and `pkg schema <command>` prints one command's entry.

## Options every command accepts

`--output text` is the default and prints results for people. `--output json` prints one JSON document on stdout and, on failure, one on stderr. [Scripts and agents](/docs/automation) covers the JSON contract and exit statuses.

## Packages

| Command                      | Options                                     | What it does                                                               |
| ---------------------------- | ------------------------------------------- | -------------------------------------------------------------------------- |
| `pkg install <packages>...`  | `--dry-run`                                 | Install packages and record them in your mise config                       |
| `pkg remove <packages>...`   | `--dry-run`                                 | Uninstall packages and take them out of your mise config                   |
| `pkg list`                   | `--manager <name>`, `--limit`, `--cursor`   | List the packages and tools your config declares, with versions            |
| `pkg search <query>...`      | `--manager <source>`, `--limit`, `--cursor` | Find packages in Homebrew, mise's registry, and the App Store              |
| `pkg info <package>`         |                                             | Describe one package and whether it is installed                           |
| `pkg outdated`               |                                             | List installed packages, and mise, that have a newer version               |
| `pkg update [<packages>...]` | `--dry-run`                                 | Update the named packages, or mise and then everything when none are named |
| `pkg sync`                   | `--dry-run`                                 | Install everything your config declares that is missing                    |
| `pkg prune`                  | `--force`, `--dry-run`                      | Uninstall Homebrew packages no config declares                             |
| `pkg where <package>`        |                                             | Print where a formula or tool is installed                                 |
| `pkg tap list`               |                                             | List the Homebrew tap URLs your config declares                            |
| `pkg tap add <tap> <url>`    | `--dry-run`                                 | Record the GitHub URL of a tap named as `owner/name`                       |
| `pkg tap remove <tap>`       | `--dry-run`                                 | Forget a Homebrew tap's URL                                                |

`--manager` repeats to select several. For `list` it takes a package manager such as `brew`, `brew-cask`, or `mas`, or `tool` for mise tools. For `search` it takes `brew`, `brew-cask`, `tool`, or `mas`. `search` returns 10 matches by default and at most 50. `prune --force` also uninstalls casks mise cannot prove are unchanged.

## Machine setup

| Command         | Options                                                                                                                                        | What it does                                                         |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `pkg setup`     | `--no-modify-path`, `--dry-run`                                                                                                                | Install mise and hand pkg to it on a machine with nothing to restore |
| `pkg bootstrap` | `--from <url>`, `--directory <checkout>`, `--config-dir <directory>`, `--config <file>`, `--env <name>`, `--save-env`, `--update`, `--dry-run` | Restore a machine from your mise configuration with `mise bootstrap` |

Both commands support macOS and Linux only. `--from` needs `--directory` and `--config-dir`, and `--env` repeats to select several environments. [Set up a machine](/docs/setup) explains when to use each and what they change.

The installer script takes its own options, `--version <version>`, `--no-modify-path`, and `--dry-run`, which [Install pkg](/docs/install#installer-options) describes.

## Discovery

| Command                      | Options               | What it does                                                    |
| ---------------------------- | --------------------- | --------------------------------------------------------------- |
| `pkg cli search <query>...`  | `--limit`, `--cursor` | Find the command for a task described in plain words            |
| `pkg schema [<command>...]`  |                       | List every command, or show one command's full specification    |
| `pkg docs search <query>...` | `--limit`, `--cursor` | Search the documentation compiled into pkg                      |
| `pkg docs show <path>`       |                       | Print one of those documents, such as `docs/setup.md`           |
| `pkg context show`           |                       | Show which pkg and mise run, the config files, and the catalogs |

These commands read only and need nothing beyond pkg. `cli search`, `schema`, and `docs` run offline. `context show` asks mise for its config when mise is installed, and says so when it is not.

## Package names

Every `<package>` argument uses mise's syntax: `brew:wget`, `brew-cask:firefox`, `mas:497799835`, `npm:prettier`, or a bare tool name from mise's registry such as `ripgrep`. [Packages](/docs/packages#name-a-package) explains prefixes and versions.

## Exit statuses

| Status | Codes                              |
| ------ | ---------------------------------- |
| 0      | Success                            |
| 2      | `INVALID_ARGUMENT`                 |
| 3      | `NOT_FOUND`, `PRECONDITION_FAILED` |
| 5      | `UNAVAILABLE`, which is retryable  |
| 6      | `MISE_FAILED`                      |
| 70     | `INTERNAL`                         |
