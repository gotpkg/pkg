# pkg

`pkg` installs and manages software with [mise](https://mise.jdx.dev)'s bootstrap packages and tools, and records every install in your global mise config, so `mise bootstrap` brings it back on a new machine. When a website says `brew install wget`, type `pkg install brew:wget`.

## Install

On macOS or Linux:

```sh
curl -fsSL https://gotpkg.dev | sh
```

The installer puts `pkg` at `~/.local/bin/pkg` and, when `~/.local/bin` is not on `PATH` yet, adds it in your shell's startup file (zsh, bash or fish; for another shell it prints the line to add). It ends with the command that loads the change in the current terminal, then `pkg bootstrap` to restore your configuration or `pkg setup` to start fresh, both described in [Setting up a machine](crates/pkg/docs/setup.md). Pass options after `sh -s --`, as in `curl -fsSL https://gotpkg.dev | sh -s -- --dry-run`:

| Option                | Effect                                                                              |
| --------------------- | ----------------------------------------------------------------------------------- |
| `--version <version>` | Install that released version, such as `0.3.0`, instead of the latest               |
| `--no-modify-path`    | Leave startup files alone and print the line to add                                 |
| `--dry-run`           | Report the version, platform, destination and files it would change; change nothing |

Downloading `https://gotpkg.dev/install.sh`, reading it, and running it with `sh install.sh` is the same installation. The installer checks the archive against the release's `SHA256SUMS`, which catches a corrupted download but not a compromised release, because both come from that release; verify an archive independently with `gh attestation verify <archive> --repo gotpkg/pkg`.

The installer supports macOS and Linux only. Release archives for macOS (Apple silicon and Intel), Linux (arm64 and x64, static musl builds) and Windows (x64) are attached to every [GitHub release](https://github.com/gotpkg/pkg/releases), each with a build attestation; on Windows, unpack `pkg-x86_64-pc-windows-msvc.zip` from there. pkg also installs with mise (`mise use -g github:gotpkg/pkg`) or Homebrew (`brew install markmals/tap/pkg`).

From a checkout:

```sh
cargo install --locked --path crates/pkg --root ~/.local
```

## Use

```sh
pkg search ffmpeg                     # find it in Homebrew, mise's registry, and the App Store
pkg info brew-cask:iina               # what it is, and whether it is installed
pkg install brew:wget brew-cask:iina  # install and record both
pkg install ripgrep npm:prettier      # tools from mise's registry and its backends
pkg list                              # everything your config declares, with versions
pkg outdated                          # what has a newer version
pkg update                            # update everything, mise itself first
pkg remove brew:wget                  # uninstall and take it out of your config
pkg sync                              # install whatever the config declares that is missing
```

| You see                    | You type                       |
| -------------------------- | ------------------------------ |
| `brew install wget`        | `pkg install brew:wget`        |
| `brew install --cask iina` | `pkg install brew-cask:iina`   |
| `brew install owner/tap/x` | `pkg install brew:owner/tap/x` |
| `mas install 497799835`    | `pkg install mas:497799835`    |
| `npm install -g prettier`  | `pkg install npm:prettier`     |
| `mise use -g ripgrep`      | `pkg install ripgrep`          |

Any package manager mise bootstrap knows (`brew`, `brew-cask`, `mas`, `apt`, `flatpak`, `winget`, ...) goes in `[bootstrap.packages]`; a tool backend prefix (`npm`, `cargo`, `github`, `aqua`, ...) or no prefix at all makes a tool in `[tools]`. New entries go to the files mise's `write_targets.packages` and `write_targets.tools` settings name; `pkg context show` prints them, with links into your dotfiles resolved. Every command that changes something accepts `--dry-run`.

`pkg remove` uninstalls Homebrew packages through `mise bootstrap packages prune`, which removes everything no config declares. When other undeclared packages are installed, `pkg remove` refuses and lists them rather than taking them too; run `pkg prune --dry-run` to see them.

### For scripts and agents

Add `--output json` to any command for JSON results on stdout and structured errors on stderr with stable exit statuses. `pkg cli search "<task>"`, `pkg schema <command>`, `pkg docs search "<topic>"` and `pkg context show` describe every command from the binary. The [CLI reference](crates/pkg/docs/cli.md) is generated from the command registry.

### From Rust

Use `pkg-core` through a Git or path dependency for typed package operations without invoking the CLI. See [Use pkg From Rust](guides/rust-library.md) and the [API contract](crates/pkg-core/src/lib.rs). The library is unpublished; the previous `pkg` implementation-module API is replaced rather than retained as aliases. CLI users need no migration.

## Develop

Prerequisites: [Mise](https://mise.jdx.dev) 2026.11.9 or newer. It installs Rust, Workbench, hk, Beads, dprint, Knope and the other tools `mise.toml` pins.

```sh
mise trust && mise install
mise run setup            # crates, Beads, git hooks
mise run check            # the gate CI runs: tests, rustdoc, formatting, Clippy, workflows, records
```

| Task                             | What it does                                          |
| -------------------------------- | ----------------------------------------------------- |
| `mise run dev -- <args>`         | Run the CLI from source                               |
| `mise run test`                  | Unit tests and tests that run the binary              |
| `mise run fmt`                   | Format everything (dprint, rustfmt)                   |
| `mise run lint`                  | Clippy over every target, warnings as errors          |
| `mise run cli:reference`         | Regenerate `crates/pkg/docs/cli.md` from the registry |
| `mise run package [-- <target>]` | Build a release archive into `artifacts/`             |
| `mise run status`                | Where the work stands in the Workbench process        |

The tests never run the real mise or reach the network: they run the binary against a fake mise script and a local catalog server in a temporary home.

## Layout

```text
crates/pkg/
  src/registry.rs       the command contract every interface is generated from
  src/commands/         each command: install, remove, list, search, info, outdated, update,
                        sync, prune, where, tap, and the discovery commands
  docs/                 Markdown built into the binary for `docs search`
  tests/cli/            tests that run the binary
crates/pkg-core/
  src/lib.rs            public Rust API and execution contract
  src/                  typed operations, settings, errors, and progress
  src/mise/             running mise with an explicit child environment
  src/declarations.rs   reading config and removing an entry from its write target
  src/catalog/          Homebrew's API and the App Store
  tests/                direct library isolation and progress tests
xtask/                  release helper: archives, the installer and its checksums
```

## Releasing

Every pull request carries a Knope change file (`workbench change-files create`) or a `Release-Note: none` trailer. On `main`, the Prepare release workflow keeps a release pull request open; merging it runs the gate, builds, smoke-tests and attests the five archives, writes `install.sh` (the installer, which embeds the release's version) and `SHA256SUMS` (the archives' checksums) and installs pkg with them, then runs `knope release` to tag `v<version>` and publish the GitHub release with the archives, `install.sh` and `SHA256SUMS`.

`https://gotpkg.dev` serves the latest release's `install.sh`. Once the site serving gotpkg.dev is deployed, set the repository variable `GOTPKG_DOMAIN` to `true`: each release then waits up to five minutes for `https://gotpkg.dev` and `https://gotpkg.dev/install.sh` to serve the new installer, and fails if they do not. Without the variable that job is skipped.

Credentials: publishing the GitHub release uses the workflow's built-in `GITHUB_TOKEN`; no stored secret is needed.
