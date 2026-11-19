---
id: guide.new-machine
title: Set Up a New Machine
---

# Set Up a New Machine

## What it does

On a new Mac or Linux machine, pkg is the first thing you install: one command puts it on `PATH` without a compiler or another package manager. From there, `pkg bootstrap` restores the mise configuration you keep in a dotfiles repository, with everything it declares, and `pkg setup` gets a machine with nothing to restore ready for `pkg install`. Either way pkg installs mise when it is missing and then hands itself to mise, so `pkg update` keeps mise, your software, and pkg current from then on. [Setting up a machine](../crates/pkg/docs/setup.md) is the precise contract, and `pkg schema setup` and `pkg schema bootstrap` list every argument and result field.

## How to use it

Restoring a dotfiles repository whose global mise configuration lives in a directory of the checkout, such as `mise/`:

1. Install pkg: `curl -fsSL https://gotpkg.dev | sh`. It installs `~/.local/bin/pkg`, adds `~/.local/bin` to `PATH` in your shell's startup file, and prints the command that loads the change in this terminal, such as `source ~/.zshenv`. Run that command, or open a new terminal.
2. Preview: `pkg bootstrap --from <repository URL> --directory ~/.dotfiles --config-dir mise --dry-run`. Before mise and the checkout exist, the preview says what pkg would install, clone, and link; run it again afterwards to see what the configuration would change.
3. Restore: the same command without `--dry-run`. In a terminal, mise and the programs it runs ask you directly for what they need, such as your password for sudo or a Git sign-in. pkg reports the follow-up work mise lists, such as signing in to the App Store.
4. Later runs need no source: `pkg bootstrap` applies the linked configuration again, and `pkg bootstrap --update` pulls new commits first.

Starting with nothing to restore:

1. Install pkg as above.
2. Run `pkg setup`. It installs mise, puts mise's tools on `PATH`, and hands pkg to mise. Run the command it prints to load the new `PATH` in this terminal.
3. Install software: `pkg install ripgrep`.

## Worked example

A dotfiles repository keeps the global configuration in `mise/`, and its own `mise.toml` only develops the repository. On a new Mac with zsh:

```sh
curl -fsSL https://gotpkg.dev | sh
source ~/.zshenv
pkg bootstrap --from https://github.com/you/dotfiles.git --directory ~/.dotfiles --config-dir mise
```

mise clones the repository into `~/.dotfiles`, links `~/.config/mise` to `~/.dotfiles/mise`, and applies it. The dotfiles deploy their own `~/.zshenv`, so pkg moves the installer's `~/.zshenv` aside first and does not put it back. pkg then links `~/.local/bin/pkg` to mise's shim; from now on `pkg update` updates pkg with everything else.

## Limits and edge cases

- Use `https://gotpkg.dev/install.sh` when downloading with a custom HTTP client or overriding curl's user agent. That endpoint returns the script regardless of headers; the root serves the website to other clients by default, as the [installation contract](../crates/pkg/docs/setup.md#installing-pkg) describes.
- `--config-dir` is required with `--from`: without it, mise would apply the repository's root `mise.toml` as a bootstrap project and link nothing.
- mise links its configuration directory only into an empty place. When `~/.config/mise` already holds something, such as the configuration `pkg setup` creates, `pkg bootstrap --from` stops before changing anything and asks you to move it aside. Restore with `pkg bootstrap` instead of running `pkg setup` first.
- A bootstrap is not a transaction. When it fails, the error says which step failed and whether mise had started; fix the cause and run the same command again. Parts that already match are left alone, but hooks and the `bootstrap` task run again.
- With `--output json`, or without a terminal, nothing waits for an answer: a step that needs one fails, and the error says to run the command in a terminal.
- A configuration can run any command, so restore only one you have reviewed. A dry run runs no hooks, but it leaves out `[bootstrap.files]` entries with `template = true`, which mise renders even in a dry run.
- The installer, `pkg setup`, and `pkg bootstrap` support macOS and Linux only.
