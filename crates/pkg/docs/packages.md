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

When mise fails, pkg takes the failed package's new declaration back out of the config, so a broken entry never stops a later `mise bootstrap`, and stops; the error names the packages installed before it. `--dry-run` shows mise's plan and changes nothing. Progress goes to stderr, as a live display in a terminal ([Progress in a terminal](#progress-in-a-terminal)).

## Removing

mise has no command that uninstalls a single Homebrew package. `mise bootstrap packages prune` uninstalls every formula and cask no config declares, with the dependencies nothing else needs. `pkg remove` therefore:

1. Checks with a dry run that a prune would remove nothing yet. When undeclared packages are already installed, removing would uninstall them too, so pkg refuses with `PRECONDITION_FAILED` and lists them in `details.undeclared`, changing nothing. Run `pkg prune --dry-run` to see them, then `pkg prune` to uninstall them or `pkg install` to keep them.
2. Takes the declarations out of the config file. pkg deletes only each entry's own lines and keeps every comment.
3. Prunes, and reports everything uninstalled, dependencies included.

A formula that a declared package still depends on stays installed and is listed under `leftInstalled`. Tools are removed with `mise unuse`, which also uninstalls versions no other config uses. mise cannot uninstall App Store apps or other managers' packages: pkg takes them out of the config and lists them under `leftInstalled` for you to delete.

## Keeping up to date

- `pkg outdated` lists what has a newer version: Homebrew formulae and casks from mise's upgrade plan, App Store apps compared with the App Store, tools from `mise outdated`, and mise itself, from the newest release `mise version` reports. Casks mise will not update, such as apps that update themselves, appear under `skipped`, and so does a newer mise that a package manager installed.
- `pkg update` updates everything, or the packages you name, and lists each version that changed. With no names it first runs `mise self-update`, so packages and tools then update with the new mise; if that fails, pkg stops before changing anything else. A mise installed by Homebrew, apt or another package manager refuses to update itself, so pkg leaves it to that package manager and lists it under `skipped`. mise waits until a release has reached its minimum release age, a day unless you set `minimum_release_age`, so `pkg outdated` can list a mise release that `pkg update` leaves for later. Once pkg is handed to mise ([Setting up a machine](setup.md#handing-pkg-to-mise)), `pkg update` updates pkg too.
- `pkg sync` installs whatever the config declares that is missing, for example after pulling config changes made on another machine. Unlike `mise bootstrap`, it leaves dotfiles, services and settings alone.
- `pkg prune` uninstalls Homebrew packages no config declares. `--force` also removes casks mise cannot prove are unchanged.
- `pkg where brew:<formula>` prints a formula's `opt` directory, which keg-only formulae need on `PATH`; `pkg where <tool>` prints a tool's install directory.

## Where installed state comes from

`pkg list`, `info`, `install`, `sync` and `update` ask mise which declared packages are installed, and mise checks each one on the machine: a formula's `opt` link, `mas list`, `dpkg-query` and the like. Homebrew casks are the exception. mise checks a cask against its current definition, which it fetches from Homebrew or the tap's repository on every run, so on macOS and Linux pkg leaves casks out of mise's check, through mise's `system_packages.managers` setting, and reads them from Homebrew's Caskroom instead. A cask is installed when `<prefix>/Caskroom/<token>/` holds exactly one version; the prefix is `MISE_SYSTEM_BREW_PREFIX`, else `/opt/homebrew` on macOS and `/home/linuxbrew/.linuxbrew` on Linux. Reading state this way needs no network.

Because pkg does not compare a cask with its current definition, its answer can differ from `mise bootstrap packages status`:

- An app you deleted by hand while its Caskroom version remains counts as installed, so `pkg install` leaves it `unchanged` and `pkg sync` skips it. `mise bootstrap packages apply` reinstalls it.
- A cask whose current definition installs a different app than the one on disk counts as installed.
- A renamed cask declared under its old name counts as missing.
- A cask that is not available for this macOS version or architecture counts as missing rather than skipped.
- A cask with several version directories counts as missing.

When your config sets `system_packages.managers` without `brew-cask`, or names `brew-cask` in `system_packages.prefer`, pkg leaves casks to mise.

## Taps

mise finds a third-party tap `owner/name` at `https://github.com/owner/homebrew-name` without any setup: `pkg install brew:owner/name/formula` works directly. Only a tap whose repository lives elsewhere on GitHub needs `pkg tap add owner/name <url>`. `pkg tap list` shows the recorded URLs and `pkg tap remove` forgets one.

## Previewing with dry run

Every command that changes something accepts `--dry-run`. A dry run makes the same checks and decisions as the real run and changes nothing: no install, no uninstall, no config edit. Its receipt has `status` `dry-run`.

## Progress in a terminal

With text output on a terminal, `pkg install`, `update`, `sync`, `remove` and `prune` show a live display on stderr while they work, dry runs included. It shows:

- The step pkg is on, such as reading what is installed or installing `brew:wget`, and how long it has taken. Something on it moves several times a second, even while mise prints nothing.
- For a request that names more than one package, the package being worked on, its place in the request (`2 of 3`) and a bar of packages finished. A package that needs no work counts as finished at once.
- Every item mise is working on, dependencies included: its name, its version once mise has stated it, and the last thing mise said it was doing, such as `download` or `link`.
- How much is done, with a bar wherever the total is known: formulae and casks finished against the plan mise gave for the package, tools finished against the number mise is installing, and a download's bytes, total and rate. Without a total it shows how many have finished. For `remove` and `prune` it lists the packages mise is about to uninstall.

Every figure is the last one mise reported. pkg does not fill in numbers between reports or estimate the time left. mise reports a tool download's bytes every 3 seconds at best and a Homebrew download's bytes not at all, so formulae and casks show their phase and no byte counts. To know how many formulae a package brings, `pkg install` asks mise for the package's plan just before installing it, a read-only query that takes a fraction of a second.

mise's progress lines update the display instead of scrolling past. The lines worth keeping stay on stderr above it: one line for each finished formula, cask or tool, and every warning, error and line pkg does not recognize. When the command ends, the display disappears and pkg's result follows those lines. When a step fails, pkg first writes the progress lines it held back during that step, so the error keeps the context it has without the display.

The display stays off, and mise's progress lines go to stderr as they arrive, when stderr is not a terminal (`pkg install brew:wget 2>&1 | cat`), with `--output json`, when the `CI` environment variable is set to anything but an empty string, and when `TERM` is `dumb`, or unset outside Windows. No option turns it on or off: redirect stderr or set `CI` to get plain lines for one run. Ctrl-C leaves the terminal usable, though the last state drawn may stay on screen.

## Output for scripts and agents

Text is the default. `--output json` writes the result as one JSON document on stdout and, on failure, one JSON document on stderr: `{"error": {"code", "message", "retryable", "details", "suggestions"}}`. Exit statuses are stable: `INVALID_ARGUMENT` 2, `NOT_FOUND` 3, `PRECONDITION_FAILED` 3, `UNAVAILABLE` 5 (retryable), `MISE_FAILED` 6, `INTERNAL` 70. Progress and pkg's warnings go to stderr in both formats; with `--output json`, or whenever stderr is not a terminal, progress is mise's own lines as they arrive, never the live display. pkg never prompts. The one exception is `pkg bootstrap` with text output in a terminal: mise and the programs it runs may ask you for passwords and answers there ([Setting up a machine](setup.md#in-a-terminal-and-in-scripts)).
