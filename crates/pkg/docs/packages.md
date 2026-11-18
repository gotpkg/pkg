# Packages

pkg installs software with mise and records every install in your global mise config, so `mise bootstrap` installs the same software on a new machine. `pkg schema <command>` has the exact arguments, errors and result schema of each command.

## Naming packages

Name a package the way mise does, as `manager:name`:

| You see                    | You type                       | Where it is recorded   |
| -------------------------- | ------------------------------ | ---------------------- |
| `brew install wget`        | `pkg install brew:wget`        | `[bootstrap.packages]` |
| `brew install --cask iina` | `pkg install brew-cask:iina`   | `[bootstrap.packages]` |
| `brew install owner/tap/x` | `pkg install brew:owner/tap/x` | `[bootstrap.packages]` |
| An App Store app           | `pkg install mas:<app id>`     | `[bootstrap.packages]` |
| `npm install -g prettier`  | `pkg install npm:prettier`     | `[tools]`              |
| A tool in mise's registry  | `pkg install ripgrep`          | `[tools]`              |

A prefix that names one of mise's tool backends (`mise backends ls`: `npm`, `cargo`, `github`, `aqua`, `go`, `pipx`, ...) makes a tool, and so does no prefix at all: a bare name is a tool from mise's registry, exactly as `mise use ripgrep` reads it. Any other prefix is a bootstrap package manager: `brew`, `brew-cask`, `mas`, `apt`, `dnf`, `flatpak`, `winget` and the rest, which mise checks.

Homebrew keeps versions in its names (`brew:postgresql@17`), so `@` is part of a formula or cask name. For every other package, `@<version>` pins a version: `node@24`, `apt:curl@8.5.0-2`. A tool without a version is recorded as `latest`.

Not sure of the name? `pkg search <words>` searches Homebrew, mise's registry and the App Store and prints the exact package to install.

## Where installs are recorded

pkg runs every mise command in your home directory (`mise -C ~`), so only the global config counts, never a project's `mise.toml` in the current directory. New bootstrap packages go to the file mise's `write_targets.packages` setting names and new tools to the `write_targets.tools` file; without those settings, both go to the global `config.toml`. When the config directory is a link into a dotfiles checkout, that checkout is where the change lands, ready to commit. `pkg context show` prints both files with links resolved.

pkg edits only those files. A package declared in another config file, such as a `config.toml` shared with other machines, is left alone: `pkg remove` fails with `PRECONDITION_FAILED` and names the file, so you can decide whether the other machines should lose it too.

## Installing

`pkg install` handles each package in turn:

- Declared and installed already: nothing changes; the receipt says `unchanged`. Installing twice is safe.
- Declared but missing: mise installs it where it is declared (`installed`).
- Not declared: pkg declares it in the write target and mise installs it (`added`). A package whose manager this machine lacks, such as `apt:` on macOS, is declared but not installed (`declared`).

mise's progress goes to stderr. When mise fails, pkg takes the failed package's new declaration back out of the config, so a broken entry never stops a later `mise bootstrap`, and stops; the error names the packages installed before it. `--dry-run` shows mise's plan and changes nothing.

## Removing

mise has no command that uninstalls a single Homebrew package. `mise bootstrap packages prune` uninstalls every formula and cask no config declares, with the dependencies nothing else needs. `pkg remove` therefore:

1. Checks with a dry run that a prune would remove nothing yet. When undeclared packages are already installed, removing would uninstall them too, so pkg refuses with `PRECONDITION_FAILED` and lists them in `details.undeclared`, changing nothing. Run `pkg prune --dry-run` to see them, then `pkg prune` to uninstall them or `pkg install` to keep them.
2. Takes the declarations out of the config file. pkg deletes only each entry's own lines and keeps every comment.
3. Prunes, and reports everything uninstalled, dependencies included.

A formula that a declared package still depends on stays installed and is listed under `leftInstalled`. Tools are removed with `mise unuse`, which also uninstalls versions no other config uses. mise cannot uninstall App Store apps or other managers' packages: pkg takes them out of the config and lists them under `leftInstalled` for you to delete.

## Keeping up to date

- `pkg outdated` lists what has a newer version: Homebrew formulae and casks from mise's upgrade plan, App Store apps compared with the App Store, and tools from `mise outdated`. Casks mise will not update, such as apps that update themselves, appear under `skipped`.
- `pkg update` updates everything, or the packages you name, and lists each version that changed.
- `pkg sync` installs whatever the config declares that is missing, for example after pulling config changes made on another machine. Unlike `mise bootstrap`, it leaves dotfiles, services and settings alone.
- `pkg prune` uninstalls Homebrew packages no config declares. `--force` also removes casks mise cannot prove are unchanged.
- `pkg where brew:<formula>` prints a formula's `opt` directory, which keg-only formulae need on `PATH`; `pkg where <tool>` prints a tool's install directory.

## Taps

mise finds a third-party tap `owner/name` at `https://github.com/owner/homebrew-name` without any setup: `pkg install brew:owner/name/formula` works directly. Only a tap whose repository lives elsewhere on GitHub needs `pkg tap add owner/name <url>`. `pkg tap list` shows the recorded URLs and `pkg tap remove` forgets one.

## Previewing with dry run

Every command that changes something accepts `--dry-run`. A dry run makes the same checks and decisions as the real run and changes nothing: no install, no uninstall, no config edit. Its receipt has `status` `dry-run`.

## Output for scripts and agents

Text is the default. `--output json` writes the result as one JSON document on stdout and, on failure, one JSON document on stderr: `{"error": {"code", "message", "retryable", "details", "suggestions"}}`. Exit statuses are stable: `INVALID_ARGUMENT` 2, `NOT_FOUND` 3, `PRECONDITION_FAILED` 3, `UNAVAILABLE` 5 (retryable), `MISE_FAILED` 6, `INTERNAL` 70. mise's progress and pkg's warnings go to stderr in both formats. No command prompts.
