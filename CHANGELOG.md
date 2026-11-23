## 0.1.2 (2026-11-23)

### Fixes

#### Show only the tools pkg update installs, not every version mise looks up

In a terminal, pkg update left one line on stderr for every tool in your mise config, even when you named a single package, and its display counted them as tools being updated ("0/67 tools"). Those lines only report mise looking versions up before it upgrades anything. They now update the display and disappear; a lookup that fails still stays on stderr. Piped output, --output json and CI runs still get every line mise prints.

## 0.1.1 (2026-11-23)

### Features

#### Add pkg shims, which point other package managers' global installs at pkg

pkg shims install writes small executables named brew, apt, npm, pip, winget and the other package managers mise can drive. With their directory first on PATH, a global install through one runs nothing and names the pkg command that does it, so what an agent installs is recorded in your mise config. Everything else runs the real tool, and the mise shim keeps your own mise bootstrap working. pkg shims list shows what each name runs and prints the PATH line to add; pkg shims remove deletes the shims.

#### See progress while pkg changes packages

In a terminal, pkg install, update, sync, remove and prune now show a live display while they work: the step pkg is on and how long it has taken, the package being worked on and how many remain, every formula, cask or tool mise is working on with its last phase, and a bar wherever mise has given a total, such as the formulae a package brings or a tool download's bytes. One line per finished item stays on stderr afterwards, and a failure still leaves every line mise printed. To count formulae, pkg install asks mise for each Homebrew package's plan first, which takes a fraction of a second. With stderr redirected, with --output json, or with CI set, output is exactly as before.

For pkg-core users, Progress gains Step and Plan events and InstallRequest gains report_plans. Code that matches every Progress event or builds an InstallRequest needs the new arm and field; ignoring the new events loses nothing.

### Fixes

#### Install and restore from the pkg website

The website now serves the release installer to command-line clients and includes guides for pkg setup, pkg bootstrap, and pkg update. Browser visits keep the marketing site and searchable documentation.

#### List installed packages without waiting on the network

pkg list, info, install, sync and update no longer wait on mise to fetch every Homebrew cask's definition before saying what is installed. pkg now leaves casks out of mise's check and reads them from Homebrew's Caskroom, so pkg list answers in about 0.2 s instead of 1–3 s and works offline. mise still checks every other package manager, apt included, on the machine. pkg install also stops asking what is installed before it installs a package nothing declares yet.

In JSON output, a package that is not installed now reports its version as null instead of an empty string.

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
