---
title: Set up a machine
description: Restore a Mac or Linux machine from the mise configuration in your dotfiles with pkg bootstrap, or start fresh with pkg setup.
sidebar:
    order: 3
---

After [installing pkg](/docs/install) on a Mac or Linux machine, one more command gets the machine ready. Which one depends on what you bring:

| You have                                                        | Run                                                                          |
| --------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| A dotfiles repository that holds your global mise configuration | `pkg bootstrap --from <url> --directory <checkout> --config-dir <directory>` |
| Nothing to restore                                              | `pkg setup`                                                                  |

Both install mise when pkg finds none, and both end by [handing pkg to mise](#hand-pkg-to-mise), so `pkg update` keeps mise, your software, and pkg current. Both support macOS and Linux only. On Windows they fail with `PRECONDITION_FAILED`, and the [package commands](/docs/packages) work with a mise you install yourself.

Choose before you run either. `pkg setup` creates a global mise configuration, and `pkg bootstrap --from` refuses to link your dotfiles over one. To restore, run `pkg bootstrap` directly.

## Restore from your dotfiles

Say your dotfiles repository keeps the global mise configuration in its `mise/` directory, and its own `mise.toml` only develops the repository. On a new Mac with zsh:

```sh
curl -fsSL https://gotpkg.dev | sh
source ~/.zshenv
pkg bootstrap --from https://github.com/you/dotfiles.git --directory ~/.dotfiles --config-dir mise --dry-run
pkg bootstrap --from https://github.com/you/dotfiles.git --directory ~/.dotfiles --config-dir mise
```

mise clones the repository into `~/.dotfiles`, or reuses a checkout there whose `origin` is the same URL. It links `~/.config/mise` to `~/.dotfiles/mise` and applies that configuration. These dotfiles deploy their own `~/.zshenv`, so pkg moves the installer's `~/.zshenv` aside first and does not put it back. Finally pkg links `~/.local/bin/pkg` to mise's shim.

The three source options go together:

- `--from <url>` is the repository's Git URL.
- `--directory <checkout>` is where the checkout is or goes, such as `~/.dotfiles`.
- `--config-dir <directory>` is the directory inside the checkout that holds your global configuration, such as `mise`, or `.` for the root. Only you know which directory that is. Without it, mise would apply the repository's own `mise.toml` and link nothing, so pkg requires it.

mise links its configuration directory, normally `~/.config/mise`, only into an empty place. When that directory already holds something, such as the configuration `pkg setup` creates, pkg stops before changing anything and tells you to move it aside.

After the first run, no source is needed:

```sh
pkg bootstrap                         # apply the linked configuration again
pkg bootstrap --update                # pull new commits and refresh package metadata first
pkg bootstrap --env work --save-env   # select an environment and make it the default
```

`--env` repeats to select several environments. `--config <file>` applies one existing mise config file together with your global configuration, and copies nothing from it. pkg never pulls, prunes, forces over conflicts, or saves environments unless you ask.

## What bootstrap changes

`pkg bootstrap` runs `mise bootstrap`, which applies everything a configuration declares: packages, tools, dotfiles, settings, services, hooks, and the `bootstrap` task. A configuration can run any command. Restore only one you have reviewed.

Around mise's run, pkg itself does three things:

1. When mise is missing, it installs mise first, as [`pkg setup`](#start-fresh) does.
2. It deletes any startup file that holds nothing but pkg's own `PATH` block, so a configuration that deploys that file, such as a `~/.zshenv` linked from your dotfiles, does not stop on a conflict pkg created. Afterwards, whether bootstrap succeeded or not, pkg writes the block back to each such file the configuration did not deploy.
3. When bootstrap succeeds, it hands pkg to mise.

A bootstrap is not a transaction. When one fails, the error says which step failed and whether mise had already started, because earlier changes then remain. Fix the cause and run the same command again: parts that already match are left alone, though hooks and the `bootstrap` task run again.

The receipt names the mise pkg used and whether pkg installed it, the source and configuration directory, the environments, the startup files moved aside and put back, and what happened to pkg itself. It also lists the follow-up work mise reported, such as signing in to the App Store.

For pulling changes later, `pkg sync` is the narrower command: it installs missing packages and tools and leaves dotfiles, services, and settings alone. [Configuration](/docs/configuration#restore-another-machine) compares the two.

### In a terminal and in scripts

With text output in a terminal, mise and the programs it runs can ask you for things directly: a password for `sudo`, Git credentials, an App Store sign-in, or their own questions. No other pkg command lets anything prompt.

With `--output json`, or without a terminal, nothing waits for an answer. mise runs in a session of its own with no terminal, so a step that needs an answer fails instead of opening `/dev/tty`. pkg reports mise's failure and says to run the command in a terminal. Either way, mise's progress goes to stderr and the result to stdout.

### Preview a bootstrap

`--dry-run` installs nothing, clones nothing, links nothing, and saves no answers. How much it can show depends on what exists:

- Before mise or the checkout exists, the preview says what pkg would install, clone, and link. Once they exist, run it again to preview the configuration itself.
- When both exist, pkg runs `mise bootstrap --dry-run`, which prints hooks and the `bootstrap` task instead of running them.
- mise renders `[bootstrap.files]` entries with `template = true` even in a dry run, and a template can run a command. When the configuration has one, the preview skips the whole `files` part and lists it as not previewed.

## Start fresh

```sh
pkg setup --dry-run   # each step it would take, changing nothing
pkg setup
pkg install ripgrep   # then install what you need
```

`pkg setup` gets a machine with no configuration to restore ready for `pkg install`:

1. When pkg finds no mise, setup runs mise's own installer for `~/.local/bin/mise`. It fetches `https://mise.run`, or the URL in `PKG_MISE_INSTALLER`, such as a mirror. That script picks the newest stable release past mise's minimum release age and checks its checksum. pkg runs it as fetched, as `curl https://mise.run | sh` would, and does not verify its GPG signature. A mise that is already there is reused as it is, and either way setup checks that it is 2026.11.9 or newer.
2. When no global mise configuration exists yet and mise's shims directory (normally `~/.local/share/mise/shims`) is not on `PATH`, setup adds that directory after `~/.local/bin` in pkg's block, creating the block if the installer did not. Without a configuration, nothing else would put mise's tools on `PATH`. Setup prints the line instead with `--no-modify-path`, and when a startup file is a link or cannot be read as UTF-8 text. Then it prints the command that loads the change in this terminal.
3. Setup hands pkg to mise.

The dry run reports each step, including the installer it would run and the file it would change, and downloads, runs, and writes nothing. Running setup again changes nothing that is already in place.

## Hand pkg to mise

pkg does not update itself. Once mise is present, a successful `pkg setup` or `pkg bootstrap` makes pkg a tool mise manages, like any other.

When no configuration declares `github:gotpkg/pkg`, pkg installs it as `pkg install github:gotpkg/pkg` would, recorded as `latest` in your tools write target. On a machine with no global configuration, that declaration creates one. A configuration that already declares pkg, such as your dotfiles after the first machine, is left as it is.

Then pkg replaces `~/.local/bin/pkg` with a link to mise's `pkg` shim. The `PATH` entry the installer made keeps working, whether or not your shell activates mise, and runs the version mise selects. pkg replaces only a file there that is pkg, including a [source build](/docs/install#build-from-source). It never swaps the running pkg for an older release: when mise's release is older, for example because of mise's minimum release age, the copy stays and pkg says why. A pkg installed some other way is reported and left unchanged.

From then on, `pkg outdated` and `pkg update` cover pkg, and mise checks each release's digest and attestation. `pkg context show` reports which pkg is running and who owns it: the installer, mise, or something else. `pkg remove github:gotpkg/pkg` removes pkg like any other tool.

The repository's [Setting up a machine](https://github.com/gotpkg/pkg/blob/main/crates/pkg/docs/setup.md) owns these rules. `pkg schema setup` and `pkg schema bootstrap` list every option and result field.
