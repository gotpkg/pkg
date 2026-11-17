# Catalogs

`pkg search`, `pkg info` and `pkg outdated` read three catalogs. Homebrew itself is not installed, since mise pours its bottles directly, so pkg asks Homebrew's public API instead of `brew`.

## Homebrew

Formulae and casks come from Homebrew's API at `https://formulae.brew.sh/api`. `pkg info brew:<name>` and `pkg info brew-cask:<name>` fetch one package's document: description, homepage, version, license, dependencies, the apps a cask installs, and caveats.

Search needs every name and description, which Homebrew publishes as two documents of about 7 MB together. pkg downloads them on the first search, keeps the parts it searches in `homebrew-index.json` in its cache directory, and downloads them again once the copy is a day old. When Homebrew cannot be reached, an older copy answers with a warning on stderr; with no copy at all, search fails with `UNAVAILABLE`, which is retryable.

The cache directory is `$XDG_CACHE_HOME/pkg`, else `~/.cache/pkg` (`%LOCALAPPDATA%\pkg\cache` on Windows). Deleting it only costs the next search a download.

Third-party taps are not in Homebrew's API. Search does not find their formulae, and `pkg info brew:owner/tap/name` shows only what mise knows locally.

## mise's registry

Tools come from mise: `mise registry` for search and `mise tool <name>` for info. Both run locally.

## The App Store

`mas:` apps come from Apple's iTunes Search API at `https://itunes.apple.com`: search asks it on every query, info looks up one app ID, and outdated compares each installed app with the store's version. When the App Store cannot be reached, search and outdated still answer from the other catalogs, with a warning.

## Settings

| Variable            | Default                        | What it changes                  |
| ------------------- | ------------------------------ | -------------------------------- |
| `PKG_MISE`          | `mise` on `PATH`               | The mise executable pkg runs     |
| `PKG_HOMEBREW_API`  | `https://formulae.brew.sh/api` | Homebrew's API base URL          |
| `PKG_APP_STORE_API` | `https://itunes.apple.com`     | The iTunes Search API base URL   |
| `XDG_CACHE_HOME`    | `~/.cache`                     | Where the search index is cached |

`pkg context show` prints the values in effect.
