# pkg

![pkg logo banner](./assets/pkg-banner.png)

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

The installer supports macOS and Linux only. Release archives for macOS (Apple silicon and Intel), Linux (arm64 and x64, static musl builds) and Windows (x64) are attached to every [GitHub release](https://github.com/gotpkg/pkg/releases), each with a build attestation; on Windows, unpack `pkg-x86_64-pc-windows-msvc.zip` from there. With mise already installed, `mise use -g github:gotpkg/pkg` installs pkg directly.

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

## Website

The website lives in `site/`: an Astro site that builds for Cloudflare Workers, with pages and components in `site/app/`. Starlight serves the documentation under `/docs`, one Markdown file per page in `site/app/content/docs/docs/`. `mise install` provides the pinned Node and pnpm, and `site/` has its own pnpm lockfile.

| Task                        | What it does                                                                          |
| --------------------------- | ------------------------------------------------------------------------------------- |
| `mise run site:dev`         | Install the site's dependencies and serve it at http://localhost:4327                 |
| `mise run site:build`       | Build the site, with the documentation search index, into `site/dist/`                |
| `mise run site:check`       | Build, run browser tests, check formatting, lint and types (`mise run check` runs it) |
| `mise run site:worker:test` | Test the built Worker and static assets with an isolated GitHub fixture               |
| `mise run site:deploy`      | Deploy the site and installer routes to gotpkg.dev after approval                     |

The documentation pages summarize this README and `crates/pkg/docs/`, which own the CLI's behavior. Change those first, then the pages that repeat them.

Oxc discovers `site/.oxfmtrc.jsonc`, which enables Astro formatting. If VS Code has an explicit user-level `oxc.fmt.configPath`, set `"oxc.fmt.configPath": null` in the workspace settings to restore project config discovery without changing other workspaces. Run **Oxc: Restart oxfmt Server** if it retains the old config. Workspace settings remain local and untracked.

### Sharing previews

The landing page and documentation include OpenGraph metadata in the initial HTML, without JavaScript. They share the supplied PNG at `/og-image.png`, with its dimensions and alternative text, and request large-image Twitter cards. Documentation pages retain their own titles, descriptions, and canonical URLs.

Sharing crawlers receive HTML by default under the [installer routing](#installer-routing) contract, without a crawler allowlist.

### Installer routing

The site's Worker runs before static assets for `/` and `/install.sh` only. `/install.sh` always serves `https://github.com/gotpkg/pkg/releases/latest/download/install.sh` as plain text. The root selects HTML when `Accept` explicitly lists `text/html` with positive quality (case-insensitive; omitted quality means one; zero or malformed quality is not an override). Otherwise, a first `User-Agent` product of `curl/<version>` or `Wget/<version>` selects the installer, matching the product name case-insensitively and requiring a nonempty version. Every other root request gets HTML, including unknown clients, missing headers, and wildcard or non-HTML Accept values. A curl token later in another client's user agent does not qualify. This convenience endpoint introduces no `406` response; custom downloaders and overridden user agents should use `/install.sh`. Query parameters do not select a representation. Root responses include both `Accept` and `User-Agent` in `Vary`, preserving other variation fields. Other paths keep their normal static-asset routing.

A successful installer response is `200` and may be cached for at most five minutes. If GitHub cannot provide the installer, including before the first release, the Worker returns `502` with `Cache-Control: no-store`; it never substitutes a placeholder script. The [installation contract](crates/pkg/docs/setup.md#installing-pkg) describes how to use the script.

### Website deployment

The `Site` GitHub Actions workflow checks the exact revision before deploying relevant pushes to `main`, or a manual run on `main`. Set the repository variable `CLOUDFLARE_DEPLOY=true` to enable deployment. The `production` environment must allow only `main` and hold the `CLOUDFLARE_API_TOKEN` secret, `CLOUDFLARE_ACCOUNT_ID` variable, and `SITE_URL=https://gotpkg.dev`.

The deployment job restores no dependency cache. It installs and builds without Cloudflare credentials, then `mise run site:deploy:built` deploys the existing `site/dist/` output without rebuilding. Only that deployment step receives the Cloudflare credentials. `mise run site:deploy` remains the local build-and-deploy entry point.

Each built static response carries `X-Build`, using the first seven characters of `GITHUB_SHA` in CI and `dev` locally. After upload, the workflow checks every built page at its canonical URL with an explicit HTML Accept header and waits for the deployed build stamp. Installer responses are verified separately during the domain cutover; publishing a CLI release is independent of website deployment.

## Layout

```text
crates/pkg/
  src/registry.rs       the command contract every interface is generated from
  src/commands/         each command: install, remove, list, search, info, outdated, update,
                        sync, prune, where, tap, and the discovery commands
  src/progress/         the live display for package changes on a terminal
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

The site Worker serves the latest release's installer through the endpoints described in [installer routing](#installer-routing). With `GOTPKG_DOMAIN` set to `true`, each release waits up to five minutes for `https://gotpkg.dev` and `https://gotpkg.dev/install.sh` to serve the new installer, and fails if they do not. Without the variable that job is skipped.

Publishing the GitHub release uses the workflow's built-in `GITHUB_TOKEN`; no stored secret is needed. Website deployments use the protected `production` environment described in [website deployment](#website-deployment).
