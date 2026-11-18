---
title: Install pkg
description: Build pkg from a Git checkout with Cargo, put it on your PATH, and check that it finds mise.
sidebar:
    order: 2
---

pkg has no published release yet, so there is no prebuilt binary or package to install. You build it from the source repository with Cargo, which takes a few commands.

## Prerequisites

- Git, to clone [github.com/gotpkg/pkg](https://github.com/gotpkg/pkg).
- Rust 1.100 or newer, with Cargo. The checkout pins 1.100.0 in `rust-toolchain.toml`, which rustup reads when you build inside it.
- mise 2026.11.9 or newer. The build does not need it, but pkg runs mise for nearly every command. Check your version with `mise --version`, and see [mise's documentation](https://mise.jdx.dev) to install it.

The commands on this page are for a POSIX shell such as zsh or bash on macOS or Linux.

## Build and install

```sh
git clone https://github.com/gotpkg/pkg.git
cd pkg
cargo install --locked --path crates/pkg --root ~/.local
```

`--locked` builds with the exact dependency versions in `Cargo.lock`. `--root ~/.local` puts the binary at `~/.local/bin/pkg`.

## Put ~/.local/bin on your PATH

Cargo does not change your `PATH`. Ask your shell which `pkg` it runs:

```sh
command -v pkg
```

It should print a path ending in `.local/bin/pkg`. If it prints nothing, the directory is not on your `PATH` yet. Add it in your shell's startup file; for zsh:

```sh
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
```

Open a new terminal and run `command -v pkg` again. If it prints a different path, another program named `pkg` comes earlier on your `PATH`; move `~/.local/bin` ahead of it, or run `~/.local/bin/pkg` by its full path.

## Check that pkg finds mise

```sh
pkg context show
```

This reads only. It prints the mise executable and version pkg runs, and the config files new packages and tools will be recorded in. When mise is missing, the command fails with `PRECONDITION_FAILED` and suggests how to install it or how to point the `PKG_MISE` variable at a mise executable outside your `PATH`.

[Configuration](/docs/configuration) explains the files it lists. To start installing, go to [Packages](/docs/packages).

## Update or uninstall

To update, pull the checkout and run the same install command. Cargo always rebuilds and reinstalls a package installed with `--path`:

```sh
git pull
cargo install --locked --path crates/pkg --root ~/.local
```

To remove the binary, run `cargo uninstall pkg --root ~/.local`. Your mise config and everything pkg installed stay as they are.

The [README](https://github.com/gotpkg/pkg/blob/main/README.md#install) owns these install instructions.
