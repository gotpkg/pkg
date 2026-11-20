## 0.1.1 (2026-11-20)

### Fixes

#### Install and restore from the pkg website

The website now serves the release installer to command-line clients and includes guides for pkg setup, pkg bootstrap, and pkg update. Browser visits keep the marketing site and searchable documentation.

#### Make pkg links shareable without breaking the installer

The homepage serves HTML to sharing crawlers and other clients by default. Standard curl and GNU Wget requests still receive the installer; custom download clients can use /install.sh to request it explicitly.

#### Meet pkg on the web

Explore pkg through a new marketing site and searchable Starlight documentation covering installation, packages, configuration, and automation.

The landing page has light and dark themes that follow your system setting. The documentation has its own theme picker.

Primaries icons give the landing page and documentation controls a consistent style.

Pages include OpenGraph metadata and a pkg-branded image for sharing previews.

## 0.1.0 (2026-11-19)

### Breaking Changes

#### Rename pkg upgrade to pkg update

pkg update updates everything your mise config declares, or the packages you name, exactly as pkg upgrade did. The upgrade command and its JSON receipt's upgraded list are gone: call pkg update and read updated instead.

### Features

#### Install pkg with one command

curl -fsSL https://gotpkg.dev | sh installs pkg at ~/.local/bin/pkg on macOS and Linux without a compiler or another package manager, checks the download against the release's SHA256SUMS, puts ~/.local/bin on PATH in your shell's startup file, and prints the command that loads it in the current terminal. Every release now carries install.sh and SHA256SUMS.

#### pkg update updates mise first

With no package names, pkg update now runs mise self-update before updating packages and tools, and pkg outdated lists a newer mise. A mise installed by Homebrew or another package manager is left to that package manager and listed as skipped. pkg finds mise on PATH, else in ~/.local/bin, and pkg context show works without mise.

#### Restore a machine with pkg bootstrap

pkg bootstrap runs mise bootstrap for your global configuration, a dotfiles repository (--from, --directory, --config-dir), or one config file (--config). It installs mise when it is missing, refuses before changing anything when ~/.config/mise already holds another configuration, moves the installer's PATH lines aside while your dotfiles deploy their own startup files, lets mise ask for passwords and answers in a terminal, and hands pkg to mise when it succeeds. --dry-run previews without running hooks or templated files.

#### Set up a new machine with pkg setup

pkg setup installs mise with mise's own installer when none is found, puts mise's shims on PATH when no mise config exists yet, and hands pkg to mise: it declares github:gotpkg/pkg and links ~/.local/bin/pkg to mise's shim, so pkg update keeps pkg current from then on. pkg context show reports who installed the pkg you are running.

#### Use pkg as a Rust library

Use the unpublished pkg-core crate through a Git or path dependency for typed package operations, explicit execution settings, progress callbacks, and recoverable errors. The pkg executable keeps its existing commands and output. The previous unpublished pkg implementation-module API is replaced.
