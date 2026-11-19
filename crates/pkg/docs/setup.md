# Setting up a machine

pkg can be the first thing you install on a new Mac or Linux machine. Its installer needs no compiler and no other package manager. `pkg bootstrap` then restores a mise configuration you keep in a dotfiles checkout, and `pkg setup` starts fresh when there is nothing to restore. Windows keeps its release archive and the package commands, but the installer, `pkg setup` and `pkg bootstrap` support macOS and Linux only; on Windows `setup` and `bootstrap` fail with `PRECONDITION_FAILED`.

## Installing pkg

```sh
curl -fsSL https://gotpkg.dev | sh
```

`https://gotpkg.dev/install.sh` is the same script and returns it regardless of request headers. Use that explicit endpoint for custom HTTP clients or an overridden user agent. Ordinary curl and wget requests to `https://gotpkg.dev` receive the installer unless they explicitly request HTML; browsers and other clients receive the website by default. The script comes from the latest GitHub release, and the archives and their `SHA256SUMS` manifest come from that same release, so one installation never mixes versions. Pass options after `sh -s --`:

| Option                | Effect                                                                              |
| --------------------- | ----------------------------------------------------------------------------------- |
| `--version <version>` | Install that released version, such as `0.3.0`, instead of the script's own release |
| `--no-modify-path`    | Leave startup files alone and print the line to add                                 |
| `--dry-run`           | Report the version, platform, destination and files it would change; change nothing |

The script installs `pkg` at `~/.local/bin/pkg` after checking the archive against `SHA256SUMS`, and replaces an older copy only once the new one is downloaded and checked, so a failed download leaves the old one working. It never runs as root, uses sudo, installs mise or restores a configuration. It refuses to replace anything at `~/.local/bin/pkg` that is not pkg, and refuses to replace the link mise manages once pkg has been handed to mise (see below); `pkg update github:gotpkg/pkg` updates that one. The checksum guards against a corrupted download, not against a compromised release: verify an archive independently with `gh attestation verify <archive> --repo gotpkg/pkg`.

### PATH

When `~/.local/bin` is not on `PATH` already, the installer adds it, at the end of the startup file every new shell of your login shell (`$SHELL`) reads:

| Shell | File                                                               |
| ----- | ------------------------------------------------------------------ |
| zsh   | `${ZDOTDIR:-$HOME}/.zshenv`                                        |
| bash  | `~/.bashrc`, and `~/.bash_profile` if it exists, else `~/.profile` |
| fish  | `~/.config/fish/conf.d/pkg.fish`                                   |

The lines sit in a block pkg owns, so a second run adds nothing:

```sh
# >>> pkg >>>
export PATH="$HOME/.local/bin:$PATH"
# <<< pkg <<<
```

In fish the middle line is `set -gx PATH "$HOME/.local/bin" $PATH`. For any other shell, with `--no-modify-path`, or when a startup file or a directory between it and your home directory is a link, such as one your dotfiles deploy, the installer prints the line instead, so it never edits a file in your dotfiles checkout. An installer cannot change the shell that ran it, so it ends with next steps for your shell: first the command that loads the change in the current terminal, such as `source ~/.zshenv`, then `pkg bootstrap` or `pkg setup`. A new terminal needs nothing.

## Which mise pkg uses

1. `PKG_MISE`, when set. pkg never falls back from a missing or broken executable you named.
2. Otherwise `mise` on `PATH`. pkg reports a mise there that is too old rather than looking past it.
3. Otherwise `~/.local/bin/mise`, where mise's own installer puts it, even before your shell has that directory on `PATH`.

Without any of them, commands that need mise fail with `PRECONDITION_FAILED` and suggest `pkg setup`. `pkg context show` works without mise and reports the mise it found, its version, and whether `pkg update` updates it.

## Starting fresh: pkg setup

`pkg setup` gets a machine with no configuration to restore ready for `pkg install`:

1. **mise.** When pkg finds no mise, setup runs mise's own installer (`https://mise.run`, or the URL in `PKG_MISE_INSTALLER`, such as a mirror) for `~/.local/bin/mise`. That script picks the newest stable release past mise's minimum release age and checks its checksum; pkg runs it as fetched, as `curl https://mise.run | sh` would, and does not verify its GPG signature. Setup then checks that the mise it will use is 2026.11.9 or newer. A mise that is already there is reused as it is.
2. **Tools on PATH.** When no global mise configuration exists yet, so nothing else will put mise's tools on `PATH`, and mise's shims directory (`$MISE_DATA_DIR/shims`, normally `~/.local/share/mise/shims`) is not on `PATH`, setup adds it after `~/.local/bin` in pkg's block, creating the block if the installer did not. `--no-modify-path` prints the line instead, and so does setup when a startup file is linked as the installer's rule above describes or cannot be read as UTF-8 text. Setup then prints the command that loads the change in the current terminal.
3. **pkg itself.** Setup hands pkg to mise, as described below.

`pkg setup --dry-run` reports each step, including the installer it would run and the file it would change, and downloads, runs and writes nothing. Running setup again changes nothing that is already in place.

Setup's declaration of pkg creates a global configuration on a machine that has none. To restore a dotfiles checkout, run `pkg bootstrap --from …` instead of setup: mise links its configuration only into an empty place.

## Restoring a configuration: pkg bootstrap

`pkg bootstrap` runs `mise bootstrap`, which applies everything a configuration declares: packages, tools, dotfiles, settings, services, hooks and the `bootstrap` task. A configuration can run arbitrary commands, so name only one you have reviewed. pkg adds no prompts, and keeps `pkg sync`, which installs only packages and tools, as it was.

Name the configuration in one of three ways:

| Source                                                         | What mise applies                                                                                                                                                                                                           |
| -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| None                                                           | Your global configuration, as every later run does                                                                                                                                                                          |
| `--from <url> --directory <checkout> --config-dir <directory>` | A dotfiles repository: mise clones it into `<checkout>` (or reuses a checkout whose `origin` is `<url>`) and links its global configuration directory to `<config-dir>`, a directory inside the checkout (`.` for its root) |
| `--config <file>`                                              | One existing mise config file, applied together with your global configuration; pkg copies nothing from it                                                                                                                  |

`--config-dir` is required with `--from`, because only you know which directory of the checkout holds your global configuration, and without it mise would apply the repository's own `mise.toml` and link nothing. mise refuses to link when its configuration directory (normally `~/.config/mise`) already holds something else; pkg checks that before changing anything and tells you to move it aside. After the first run, `pkg bootstrap` alone applies the linked configuration.

`--env <name>` selects a configuration environment (repeat it for several), `--save-env` makes that selection the default for later runs, and `--update` pulls new commits and refreshes package metadata first. pkg never pulls, prunes, forces over conflicts, or saves environments unless you ask.

When mise is missing, bootstrap installs it first, as setup does. Before mise applies the configuration, pkg deletes any startup file that holds nothing but pkg's own `PATH` block, so a configuration that deploys that file, such as a `~/.zshenv` linked from your dotfiles, does not stop on a conflict pkg created. Afterwards, whether bootstrap succeeded or not, pkg puts the block back in any such file the configuration did not deploy. A successful bootstrap ends by handing pkg to mise.

### In a terminal and in scripts

With text output in a terminal, mise and the programs it runs can ask you for things directly: passwords for sudo, Git credentials, App Store sign-in, and their own questions. With `--output json`, or without a terminal, nothing waits for an answer: mise runs in a session of its own with no terminal, so a step that needs an answer fails rather than opening `/dev/tty`, and pkg reports mise's failure and says to run the command in a terminal. Either way mise's progress goes to stderr and the result to stdout.

### What bootstrap reports

The receipt names the mise it used and whether pkg installed it, the source and the configuration directory, the environments, the startup files it moved aside and put back, the follow-up work mise listed, and what happened when pkg was handed to mise. A failure says which step failed. A bootstrap is not a transaction: the error says whether mise had already started, because earlier changes then remain. Fix the cause and run the same command again; parts that already match are left alone, though hooks and the `bootstrap` task run again.

### Previewing a bootstrap

`--dry-run` changes nothing: it installs nothing, clones nothing, links nothing, and saves no answers. When mise and the configuration are present, pkg runs `mise bootstrap --dry-run`, which prints hooks and the `bootstrap` task instead of running them. mise renders `[bootstrap.files]` entries with `template = true` even in a dry run, and a template can run a command, so when the configuration has one, the preview skips the `files` part and lists it as not previewed. Before mise or the checkout exists, the preview says what pkg would install, clone and link, and that the configuration itself can be previewed once they exist.

## Handing pkg to mise

pkg does not update itself. Once mise is present, a successful `pkg setup` or `pkg bootstrap` makes pkg a tool mise manages, like any other:

- **Declaring it.** When no configuration declares `github:gotpkg/pkg`, pkg installs it exactly as `pkg install github:gotpkg/pkg` would, recording it as `latest` in your tools write target. A configuration that already declares it, such as your dotfiles after the first machine, is left as it is.
- **Retiring the installer's copy.** Once mise's pkg runs, pkg replaces the installer's `~/.local/bin/pkg` with a link to mise's `pkg` shim, so the `PATH` entry the installer made keeps working whether or not your shell activates mise, and runs the version mise selects. pkg replaces only a file there that is pkg, including one you built with `cargo install --root ~/.local`. It never replaces the running pkg with an older release: when mise has an older one, for example because of its minimum release age, the installer's copy stays and pkg says why. A pkg installed some other way, such as with Homebrew, is reported, not changed.
- **Updating it.** `pkg outdated` and `pkg update` cover pkg from then on; mise checks the release's digest and attestation. Before the handoff, run the installer again to update pkg.

`pkg context show` reports which pkg is running and who owns it: the installer, mise, or something else. `pkg remove github:gotpkg/pkg` removes pkg as it removes any tool.
