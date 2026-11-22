---
title: Install pkg
description: Install pkg on macOS or Linux with one command, check what the installer changes, or build pkg from source.
sidebar:
    order: 2
---

On macOS and Linux, one command installs pkg at `~/.local/bin/pkg`. It needs no compiler, no mise, and no other package manager:

```sh
curl -fsSL https://gotpkg.dev | sh
```

Run the command the installer prints last to load the new `PATH` in this terminal. Then go to [Set up a machine](/docs/setup): `pkg bootstrap` restores the mise configuration in your dotfiles, and `pkg setup` starts fresh.

## Before you start

- macOS or Linux, on an arm64 or x86_64 machine. Windows uses a [release archive](#windows) instead.
- `curl` or `wget`, `tar`, and `sha256sum` or `shasum`. The installer names whichever is missing.
- An ordinary user account. The installer refuses to run as root and never uses `sudo`.

## Installer options

Pass options after `sh -s --`:

```sh
curl -fsSL https://gotpkg.dev | sh -s -- --dry-run
```

| Option                | Effect                                                                              |
| --------------------- | ----------------------------------------------------------------------------------- |
| `--version <version>` | Install that released version, such as `0.3.0`, instead of the latest               |
| `--no-modify-path`    | Leave startup files alone and print the line to add                                 |
| `--dry-run`           | Report the version, platform, destination and files it would change; change nothing |

`https://gotpkg.dev/install.sh` returns the same script regardless of request headers. Use it for custom HTTP clients or if you override curl's user agent. To read it before it runs, download it first:

```sh
curl -fsSL https://gotpkg.dev/install.sh -o install.sh
less install.sh
sh install.sh --dry-run
sh install.sh
```

The root serves this website to browsers and sharing crawlers by default. Ordinary curl and wget requests receive the installer unless they explicitly request HTML.

## What the installer changes

The script comes from the latest [GitHub release](https://github.com/gotpkg/pkg/releases), and so do the archive and its `SHA256SUMS` manifest, so one installation never mixes versions. The installer downloads the archive for your platform, checks it against `SHA256SUMS`, and only then replaces an older `~/.local/bin/pkg`. A failed download leaves the old copy working.

Apart from `PATH`, below, it changes nothing else. It does not install mise or restore a configuration. It refuses to replace anything at `~/.local/bin/pkg` that is not pkg, and it leaves alone the link mise manages once pkg has been [handed to mise](/docs/setup#hand-pkg-to-mise); `pkg update github:gotpkg/pkg` updates that one.

### Checksums and attestations

The checksum catches a corrupted download. It cannot catch a compromised release, because `SHA256SUMS` comes from that same release. Every release archive also has a build attestation, which you can verify independently with the GitHub CLI:

```sh
gh attestation verify <archive> --repo gotpkg/pkg
```

Once mise manages pkg, mise checks the digest and attestation of each update itself.

## PATH and the current terminal

When `~/.local/bin` is not on `PATH` already, the installer adds it at the end of the startup file that every new shell of your login shell (`$SHELL`) reads:

| Shell | File                                                               |
| ----- | ------------------------------------------------------------------ |
| zsh   | `${ZDOTDIR:-$HOME}/.zshenv`                                        |
| bash  | `~/.bashrc`, and `~/.bash_profile` if it exists, else `~/.profile` |
| fish  | `~/.config/fish/conf.d/pkg.fish`                                   |

The lines sit in a block pkg owns, so running the installer again adds nothing:

```sh
# >>> pkg >>>
export PATH="$HOME/.local/bin:$PATH"
# <<< pkg <<<
```

In fish the middle line is `set -gx PATH "$HOME/.local/bin" $PATH`. The installer prints the line for you to add, and edits no file, for any other shell, with `--no-modify-path`, and when the startup file or a directory between it and your home directory is a link, such as one your dotfiles deploy. It never edits a file in your dotfiles checkout.

An installer cannot change the shell that ran it. So it ends with next steps: first the command that loads the change in this terminal, such as `source ~/.zshenv`, then `pkg bootstrap` or `pkg setup`. A new terminal needs nothing.

To check which `pkg` your shell runs:

```sh
command -v pkg
pkg context show
```

`command -v pkg` should print a path ending in `.local/bin/pkg`. When it prints another path, a different program named `pkg` comes earlier on your `PATH`; move `~/.local/bin` ahead of it, or run `~/.local/bin/pkg` by its full path. `pkg context show` reads only and works before mise is installed. It reports which pkg is running and who owns it, the mise it found, and the config files pkg would write to.

## When the installer fails

The installer's own errors start with `error:`, often followed by a `hint:` that says what to do next. The common failures:

- When GitHub cannot provide the installer, gotpkg.dev answers `502`. `curl` reports the error, `sh` receives nothing, and nothing on the machine changes. That happens before the first release is published and while GitHub cannot be reached. The [releases page](https://github.com/gotpkg/pkg/releases) lists what is published; until it lists a release, [build pkg from source](#build-from-source).
- `--version` with a version that was never released fails to download the archive and points at the releases page.
- On Windows, or on a machine that is neither arm64 nor x86_64, the installer names the platform and stops.

## Update pkg

Until pkg is handed to mise, run the installer again. It replaces the old copy only after the new one is downloaded and checked.

A successful `pkg setup` or `pkg bootstrap` hands pkg to mise. From then on `pkg outdated` lists new pkg releases and `pkg update` installs them along with everything else. [Hand pkg to mise](/docs/setup#hand-pkg-to-mise) explains what changes on disk.

## Other ways to install

### Windows

The installer, `pkg setup`, and `pkg bootstrap` support macOS and Linux only; on Windows, `setup` and `bootstrap` fail with `PRECONDITION_FAILED`. The package commands still work. Download `pkg-x86_64-pc-windows-msvc.zip` from a [GitHub release](https://github.com/gotpkg/pkg/releases), unpack it, put `pkg.exe` on your `PATH`, and install mise 2026.11.9 or newer yourself.

### With mise

With mise 2026.11.9 or newer already installed, `mise use -g github:gotpkg/pkg` installs pkg as a mise tool, and `pkg update` keeps it current.

### Build from source

Building needs Git, Rust 1.100 or newer with Cargo, and no release. The checkout pins Rust 1.100.0 in `rust-toolchain.toml`, which rustup reads when you build inside it.

```sh
git clone https://github.com/gotpkg/pkg.git
cd pkg
cargo install --locked --path crates/pkg --root ~/.local
```

`--locked` builds with the exact dependency versions in `Cargo.lock`. `--root ~/.local` puts the binary at `~/.local/bin/pkg`, where the installer puts it. Cargo does not change `PATH`, so add `~/.local/bin` yourself with the [line above](#path-and-the-current-terminal). To update, run `git pull` and the same `cargo install` command.

pkg treats this build like the installer's copy. A successful `pkg setup` or `pkg bootstrap` replaces it with a link to mise's release of pkg, unless that release is older than your build.

## Uninstall

Once mise manages pkg, `pkg remove github:gotpkg/pkg` removes it like any other tool. A copy from the installer is the single file `~/.local/bin/pkg`: delete it, or for a source build run `cargo uninstall pkg --root ~/.local`. The `# >>> pkg >>>` block stays in your startup file until you delete it. Your mise config and everything pkg installed stay as they are.

The [README](https://github.com/gotpkg/pkg/blob/main/README.md#install) and [Setting up a machine](https://github.com/gotpkg/pkg/blob/main/crates/pkg/docs/setup.md#installing-pkg) own these instructions.
