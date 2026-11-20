---
title: Configuration
description: Which mise config files and shell startup files pkg reads and writes, how to restore another machine, which mise pkg runs, and the environment variables pkg reads.
sidebar:
    order: 5
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

This prints both files, the mise executable and version, every global config file mise loads, the catalog URLs, and the cached search index. When your mise config directory is a link into a dotfiles checkout, as `pkg bootstrap --from` makes it, pkg resolves the link, so the change lands in that checkout, ready to commit.

## Files pkg does not edit

New entries are written by mise. pkg edits a config file itself only to take an entry out, and only in the write targets above. It deletes the entry's own lines and keeps every comment.

A package declared in any other file, such as a `config.toml` shared with other machines, is left alone. `pkg remove` fails with `PRECONDITION_FAILED` and names the file, so you decide whether the other machines should lose the package too. `pkg tap remove` treats a tap declared in another file the same way.

## Shell startup files

Outside mise's config, pkg writes to one more kind of file. The installer puts `~/.local/bin` on `PATH`, and `pkg setup` adds mise's shims directory when no global config exists yet. Both write inside a block of your shell's startup file that pkg owns:

```sh
# >>> pkg >>>
export PATH="$HOME/.local/bin:$PATH"
# <<< pkg <<<
```

pkg changes nothing outside that block. When the startup file, or a directory between it and your home directory, is a link, such as one your dotfiles deploy, pkg prints the line for you to add and edits nothing. `pkg bootstrap` moves aside a startup file that holds only this block, so dotfiles that deploy the same file do not conflict with it. [Install pkg](/docs/install#path-and-the-current-terminal) lists the file for each shell.

## Restore another machine

Because every install is recorded, the config is the list of what a machine should have. Keep your global mise config in a dotfiles repository, and a new Mac or Linux machine needs the installer and one `pkg bootstrap`:

```sh
curl -fsSL https://gotpkg.dev | sh
pkg bootstrap --from <repository URL> --directory ~/.dotfiles --config-dir <its mise directory>
```

Between the two, run the command the installer prints to load the new `PATH`. `pkg bootstrap` installs mise when it is missing, links mise's config directory to the checkout, and applies everything the config declares. Do not run `pkg setup` first: setup creates a global config, and bootstrap refuses to link your dotfiles over it. [Set up a machine](/docs/setup) covers the options, what a configuration can run, and the limits of a preview. `mise bootstrap` reads the same config, so a machine with mise and no pkg can apply it too.

After that, when you pull config changes made on another machine, choose how much to apply:

```sh
pkg sync --dry-run        # list the missing packages and tools, changing nothing
pkg sync                  # install them
pkg bootstrap --update    # pull new commits, then apply everything the config declares
```

`pkg sync` installs every missing `[bootstrap.packages]` entry and every missing global tool. It also removes packages declared with `state = "absent"` where the manager can, and leaves dotfiles, services, and settings alone. `pkg bootstrap` applies all of those as well, including hooks and the `bootstrap` task, so it runs whatever commands the config declares.

Not every entry applies to every machine. Packages whose manager this machine lacks, such as `apt:` packages on macOS, stay in the config and are skipped here.

## Which mise pkg runs

1. `PKG_MISE`, when set. pkg never falls back from a missing or broken executable you named.
2. Otherwise `mise` on `PATH`. pkg reports a mise there that is too old rather than looking past it.
3. Otherwise `~/.local/bin/mise`, where mise's own installer puts it, even before your shell has that directory on `PATH`.

pkg needs mise 2026.11.9 or newer. Without any of the three, commands that need mise fail with `PRECONDITION_FAILED` and suggest `pkg setup`. `pkg context show` works without mise and reports the mise it found, its version, and whether `pkg update` updates it.

## Environment variables

| Variable             | Default                                    | What it changes                                |
| -------------------- | ------------------------------------------ | ---------------------------------------------- |
| `PKG_MISE`           | `mise` on `PATH`, else `~/.local/bin/mise` | The mise executable pkg runs                   |
| `PKG_MISE_INSTALLER` | `https://mise.run`                         | The mise installer `setup` and `bootstrap` run |
| `PKG_HOMEBREW_API`   | `https://formulae.brew.sh/api`             | Homebrew's API base URL                        |
| `PKG_APP_STORE_API`  | `https://itunes.apple.com`                 | The iTunes Search API base URL                 |
| `XDG_CACHE_HOME`     | `~/.cache`                                 | Where the search index is cached               |

The cache directory is `$XDG_CACHE_HOME/pkg`, else `~/.cache/pkg` (`%LOCALAPPDATA%\pkg\cache` on Windows). Deleting it costs the next search one download. `pkg context show` prints the values in effect.

The repository's [package contract](https://github.com/gotpkg/pkg/blob/main/crates/pkg/docs/packages.md#where-installs-are-recorded), [Setting up a machine](https://github.com/gotpkg/pkg/blob/main/crates/pkg/docs/setup.md#which-mise-pkg-uses), and [catalog notes](https://github.com/gotpkg/pkg/blob/main/crates/pkg/docs/catalogs.md#settings) own these rules.
