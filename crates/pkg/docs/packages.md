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

- `pkg outdated` lists what has a newer version: Homebrew formulae and casks from mise's upgrade plan, App Store apps compared with the App Store, and tools from `mise outdated`. Casks mise will not upgrade, such as apps that update themselves, appear under `skipped`.
- `pkg upgrade` upgrades everything, or the packages you name, and lists each version that changed.
- `pkg sync` installs whatever the config declares that is missing, for example after pulling config changes made on another machine. Unlike `mise bootstrap`, it leaves dotfiles, services and settings alone.
- `pkg prune` uninstalls Homebrew packages no config declares. `--force` also removes casks mise cannot prove are unchanged.
- `pkg where brew:<formula>` prints a formula's `opt` directory, which keg-only formulae need on `PATH`; `pkg where <tool>` prints a tool's install directory.

## Taps

mise finds a third-party tap `owner/name` at `https://github.com/owner/homebrew-name` without any setup: `pkg install brew:owner/name/formula` works directly. Only a tap whose repository lives elsewhere on GitHub needs `pkg tap add owner/name <url>`. `pkg tap list` shows the recorded URLs and `pkg tap remove` forgets one.

## Shims for other package managers

Habit and agents reach for `brew install`, `apt install` or `npm install -g`, which install software no mise config records. `pkg shims install` writes small executables named for those managers. With their directory first on `PATH`, a global install through one runs nothing and names the `pkg` command that does it; every other command runs the real tool.

```sh
pkg shims install   # write the shims; print the line that puts them first on PATH
pkg shims list      # which shims exist, whether they are on PATH, and what each name runs
pkg shims remove    # delete them
```

The directory is `$XDG_DATA_HOME/pkg/shims` (`~/.local/share/pkg/shims` by default), or `%LOCALAPPDATA%\pkg\shims` on Windows. pkg never edits a startup file: add the printed line to the file your shells read, such as a `~/.zshenv` your dotfiles deploy. Each shim is a `pkg-shim`-marked script (`<name>.cmd` on Windows) that calls `pkg shims run <name> -- <arguments>`; `install` and `remove` touch only files with that marker, and `install` writes nothing and fails with `PRECONDITION_FAILED` when another file already has a shim's name.

A manager's shim is installed only on the systems mise runs that manager on, and the tool managers on every system:

| Systems      | Shims                                                                                             |
| ------------ | ------------------------------------------------------------------------------------------------- |
| All          | `npm`, `pnpm`, `yarn`, `bun`, `cargo`, `pip`, `pip3`, `pipx`, `uv`, `go`, `gem`, `dotnet`, `mise` |
| macOS, Linux | `brew`                                                                                            |
| Linux        | `apt`, `apt-get`, `dnf`, `zypper`, `apk`, `pacman`, `yay`, `paru`, `flatpak`, `snap`              |
| Linux, macOS | `nix`                                                                                             |
| macOS        | `port`, `mas`                                                                                     |
| Windows      | `winget`, `scoop`, `choco`                                                                        |

### What a shim refuses

A shim refuses an install, and for the managers `pkg remove` uninstalls from, a removal. It runs nothing, writes to stderr what to run instead and where to learn more (`pkg search`, `pkg --help`, `pkg cli search`), and exits with status 2. Names are suggested as typed, with the manager's own spelling translated to pkg's:

| Shim                       | Install refused                                                                                                                                                                  | Removal refused                       | Suggested                                                                 |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- | ------------------------------------------------------------------------- |
| `brew`                     | `install`                                                                                                                                                                        | `uninstall`, `remove`, `rm`           | `brew:<name>`; `brew-cask:<name>` with `--cask`                           |
| `apt`, `apt-get`           | `install`                                                                                                                                                                        |                                       | `apt:<name>`; `name=version` becomes `name@version`                       |
| `dnf`                      | `install`                                                                                                                                                                        |                                       | `dnf:<name>`                                                              |
| `zypper`                   | `install`, `in`                                                                                                                                                                  |                                       | `zypper:<name>`                                                           |
| `apk`                      | `add`                                                                                                                                                                            |                                       | `apk:<name>`                                                              |
| `pacman`                   | `-S` or `--sync` naming packages, without a search, info, list, group or clean modifier                                                                                          |                                       | `pacman:<name>`                                                           |
| `yay`, `paru`              | as `pacman`                                                                                                                                                                      |                                       | `aur:<name>`, and `pacman:<name>` for packages in Arch's own repositories |
| `flatpak`                  | `install`                                                                                                                                                                        |                                       | `flatpak:<id>`; `flatpak-user:<id>` with `--user`                         |
| `snap`                     | `install`                                                                                                                                                                        |                                       | `snap:<name>`                                                             |
| `nix`                      | `profile install`, `profile add`                                                                                                                                                 |                                       | `nix:<reference>`                                                         |
| `port`                     | `install`                                                                                                                                                                        |                                       | `macports:<name>`                                                         |
| `mas`                      | `install`                                                                                                                                                                        |                                       | `mas:<id>`                                                                |
| `winget`, `scoop`, `choco` | `install`                                                                                                                                                                        |                                       | `winget:<id>`, `scoop:<name>`, `choco:<name>`                             |
| `npm`, `pnpm`, `bun`       | `install`, `i`, `add` with `-g` or `--global`                                                                                                                                    | `uninstall`, `rm`, `remove` with `-g` | `npm:<name>`                                                              |
| `yarn`                     | `global add`                                                                                                                                                                     | `global remove`                       | `npm:<name>`                                                              |
| `cargo`                    | `install`, unless `--path`, `--git`, `--root` or `--list`                                                                                                                        | `uninstall`, unless `--root`          | `cargo:<name>`                                                            |
| `pip`, `pip3`              | `install` while neither `VIRTUAL_ENV` nor `CONDA_PREFIX` is set, unless `--target`, `-t`, `--prefix`, `--root`, `-r` or `-e` (or their long forms `--requirement`, `--editable`) |                                       | `pypi:<name>`; `name==version` becomes `name@version`                     |
| `pipx`                     | `install`                                                                                                                                                                        | `uninstall`                           | `pypi:<name>`                                                             |
| `uv`                       | `tool install`                                                                                                                                                                   | `tool uninstall`                      | `pypi:<name>`                                                             |
| `go`                       | `install` of a `module@version`                                                                                                                                                  |                                       | `go:<module>`, with `@version` unless it is `latest`                      |
| `gem`                      | `install`, unless `-i` or `--install-dir`                                                                                                                                        | `uninstall`, unless the same          | `gem:<name>`                                                              |
| `dotnet`                   | `tool install` with `-g` or `--global`                                                                                                                                           | `tool uninstall` with `-g`            | `dotnet:<name>`                                                           |
| `mise`                     | `use` with `-g` or `--global`                                                                                                                                                    | `unuse`, `rm`, `remove` with `-g`     | the names as typed                                                        |

Installs suggest `pkg install <names>` and removals `pkg remove <names>`. A suggestion is never a guess, so a shim does not refuse an invocation that names no package, names a path, URL or archive (starts with `.`, `/`, `~` or a drive letter, contains `://`, or ends `.tgz`, `.tar.gz`, `.zip`, `.deb`, `.rpm` or `.whl`) or something that does not start with a letter, digit or `@`, or uses an option that takes a separate value, such as `--registry`, `--index-url`, `-r` or `--prefix`. Two tools are read as they are written: a `flatpak install` may begin with a remote name (it has no `.`, which every application ID has, so a name without one is not refused either), and `winget install --id <id>` names the package in `--id`.

### Everything else runs the real tool

Every other command (`update`, `upgrade`, `search`, `list`, `build`, `run` and so on) runs the first executable of that name on `PATH` after the shim directory, with the same arguments and its exit status. When there is none, the shim says the tool is not installed on this machine and exits 127. On Unix a shim whose `pkg` has gone from `PATH` runs the real tool directly; on Windows it says pkg was not found and exits 127, so take the directory off `PATH` or run `pkg shims remove`.

pkg itself never meets a shim: it runs the first `mise` outside the shim directory, and every process it starts gets a `PATH` without that directory. The `mise` shim does the same for the real mise, so your own `mise bootstrap`, `mise install` and `mise use` reach the real package managers; `mise exec`, `x`, `run`, `r`, `watch` and `w` keep the shim directory on `PATH`, so the commands they run meet the shims.

### Limits

- A shim is found through `PATH`. Running a tool by its full path, or `sudo apt install` (sudo searches its own `secure_path`), is not intercepted.
- In a shell that runs `mise activate`, the directories of the tools mise provides come first on `PATH` (see [mise's shims and activation](https://mise.jdx.dev/dev-tools/shims.html#shims-vs-path)), ahead of the shim directory, so `npm` from a mise-managed node runs unshimmed there. A shell that only reads a startup file such as `~/.zshenv` keeps the shims first. `pkg shims list` shows the name a shell resolves to.
- `command -v brew` succeeds on a Mac without Homebrew, as it does for mise's own shims; running it exits 127.
- A Linux machine gets every Linux shim, whichever distribution it runs.
- Upgrades and updates (`brew upgrade`, `apt upgrade`, `winget upgrade`), and removals through managers `pkg remove` cannot uninstall from, run the real tool.

## Previewing with dry run

Every command that changes something accepts `--dry-run`. A dry run makes the same checks and decisions as the real run and changes nothing: no install, no uninstall, no config edit. Its receipt has `status` `dry-run`.

## Output for scripts and agents

Text is the default. `--output json` writes the result as one JSON document on stdout and, on failure, one JSON document on stderr: `{"error": {"code", "message", "retryable", "details", "suggestions"}}`. Exit statuses are stable: `INVALID_ARGUMENT` 2, `NOT_FOUND` 3, `PRECONDITION_FAILED` 3, `UNAVAILABLE` 5 (retryable), `MISE_FAILED` 6, `INTERNAL` 70. mise's progress and pkg's warnings go to stderr in both formats. No command prompts.
