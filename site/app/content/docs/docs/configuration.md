---
title: Configuration
description: Which mise config files pkg reads and writes, how to restore another machine, and the environment variables pkg reads.
sidebar:
    order: 4
---

pkg keeps no config of its own. Everything it installs is recorded in your global mise config, and `pkg context show` prints exactly which files those are on this machine.

## Only the global config counts

pkg runs every mise command in your home directory (`mise -C ~`). A project's `mise.toml` in the directory you happen to be in never receives an entry and never changes what `pkg list` reports.

## Where new entries go

mise's own settings choose the files:

- New bootstrap packages (`brew:`, `brew-cask:`, `mas:`, and other managers) go to the file named by mise's `write_targets.packages` setting.
- New tools go to the file named by `write_targets.tools`.
- Without those settings, both go to mise's global `config.toml`.

```sh
pkg context show
```

This prints both files, the mise executable and version, every global config file mise loads, the catalog URLs, and the cached search index. When your mise config directory is a link into a dotfiles checkout, pkg resolves the link, so the change lands in that checkout, ready to commit.

## Files pkg does not edit

New entries are written by mise. pkg edits a config file itself only to take an entry out, and only in the write targets above. It deletes the entry's own lines and keeps every comment.

A package declared in any other file, such as a `config.toml` shared with other machines, is left alone. `pkg remove` fails with `PRECONDITION_FAILED` and names the file, so you decide whether the other machines should lose the package too. `pkg tap remove` treats a tap declared in another file the same way.

## Restore another machine

Because every install is recorded, the config is the list of what a machine should have. On a new machine, bring over your mise config (for example, by cloning your dotfiles), install mise, and run `mise bootstrap`, which installs the packages and tools the config declares. pkg itself is not needed for this step.

After that, when you pull config changes made on another machine, install what is missing with pkg:

```sh
pkg sync --dry-run    # list what is missing, changing nothing
pkg sync              # install it
```

`pkg sync` installs every missing `[bootstrap.packages]` entry and every missing global tool. It also removes packages declared with `state = "absent"` where the manager can. Unlike `mise bootstrap`, it leaves dotfiles, services, and settings alone.

Not every entry applies to every machine. Packages whose manager this machine lacks, such as `apt:` packages on macOS, stay in the config and are skipped here.

## Environment variables

| Variable            | Default                        | What it changes                  |
| ------------------- | ------------------------------ | -------------------------------- |
| `PKG_MISE`          | `mise` on `PATH`               | The mise executable pkg runs     |
| `PKG_HOMEBREW_API`  | `https://formulae.brew.sh/api` | Homebrew's API base URL          |
| `PKG_APP_STORE_API` | `https://itunes.apple.com`     | The iTunes Search API base URL   |
| `XDG_CACHE_HOME`    | `~/.cache`                     | Where the search index is cached |

The cache directory is `$XDG_CACHE_HOME/pkg`, else `~/.cache/pkg` (`%LOCALAPPDATA%\pkg\cache` on Windows). Deleting it costs the next search one download. `pkg context show` prints the values in effect.

The repository's [package contract](https://github.com/gotpkg/pkg/blob/main/crates/pkg/docs/packages.md#where-installs-are-recorded) and [catalog notes](https://github.com/gotpkg/pkg/blob/main/crates/pkg/docs/catalogs.md#settings) own these rules.
