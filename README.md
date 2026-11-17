# pkg

`pkg` installs and manages software with [mise](https://mise.jdx.dev)'s bootstrap packages and tools, and records every install in your global mise config, so `mise bootstrap` brings it back on a new machine. When a website says `brew install wget`, type `pkg install brew:wget`.

## Install

From a checkout:

```sh
cargo install --locked --path crates/pkg --root ~/.local
```

Once a release is published, with mise (`mise use -g github:markmals/pkg`) or Homebrew (`brew install markmals/tap/pkg`).

Release archives for macOS (Apple silicon and Intel), Linux (arm64 and x64, static musl builds) and Windows (x64) are attached to every [GitHub release](https://github.com/markmals/pkg/releases), each with a build attestation: `gh attestation verify <archive> --repo markmals/pkg`.

## Use

```sh
pkg search ffmpeg                     # find it in Homebrew, mise's registry, and the App Store
pkg info brew-cask:iina               # what it is, and whether it is installed
pkg install brew:wget brew-cask:iina  # install and record both
pkg install ripgrep npm:prettier      # tools from mise's registry and its backends
pkg list                              # everything your config declares, with versions
pkg outdated                          # what has a newer version
pkg upgrade                           # upgrade everything
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
  src/commands/         each command: install, remove, list, search, info, outdated, upgrade,
                        sync, prune, where, tap, and the discovery commands
  src/package.rs        package names, and whether one is a bootstrap package or a tool
  src/mise/             running mise and reading its answers
  src/declarations.rs   reading mise config files and removing an entry from one
  src/catalog/          Homebrew's API and the App Store
  docs/                 Markdown built into the binary for `docs search`
  tests/cli/            tests that run the binary
xtask/                  release helper: archives and the Homebrew formula
```

## Releasing

Every pull request carries a Knope change file (`workbench change-files create`) or a `Release-Note: none` trailer. On `main`, the Prepare release workflow keeps a release pull request open; merging it runs the gate, builds, smoke-tests and attests the five archives, runs `knope release` to tag `v<version>` and publish the GitHub release with the archives, and then pushes the Homebrew formula to `markmals/homebrew-tap` when the repository variable `HOMEBREW_TAP` is `true` (otherwise that job is skipped).

Credentials: publishing the GitHub release uses the workflow's built-in `GITHUB_TOKEN`; no stored secret is needed. The Homebrew job alone uses the `homebrew` GitHub environment, which holds `HOMEBREW_TAP_TOKEN`: create that environment under Settings → Environments, restrict its deployment branches to `main`, add a fine-grained token with contents write access to `markmals/homebrew-tap` only as its `HOMEBREW_TAP_TOKEN` secret, then set the repository variable `HOMEBREW_TAP` to `true`.
