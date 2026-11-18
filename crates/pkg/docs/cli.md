# CLI reference

`pkg` installs, removes, and finds packages with mise and records every install in your global mise config. People use the text output; scripts and agents add `--output json`. This page is generated from the command registry by `mise run cli:reference`; do not edit it by hand.

Every command follows one contract:

- **Results** are text for people by default. `--output json` writes the same result as one JSON document on stdout, compact when piped and indented on a terminal. mise's progress, warnings, and errors go to stderr.
- **Collections** return `items`, `hasMore`, and `nextCursor`; pass `nextCursor` back as `--cursor` until `hasMore` is false.
- **Modifying commands** accept `--dry-run`, which makes the same checks and decisions as the real run and installs, uninstalls, and writes nothing. Receipts carry a `status` (`completed` or `dry-run`) and what changed.
- **Failures** write a message and hints to stderr, or with `--output json` one JSON document, and exit with the code's status (see below). Nothing is written to stdout.
- **No command prompts.** Missing input fails at once with `INVALID_ARGUMENT`.
- **mise runs in your home directory**, so only the global config counts. New declarations go to the files mise's `write_targets.packages` and `write_targets.tools` settings name. `pkg context show` prints them.

Discover a command with `pkg cli search "<task>"`, inspect it with `pkg schema <command>`, preview it with `--dry-run`, run it, and verify the result with `pkg list` or `pkg info <package>`.

## Options every command accepts

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `--output <choice>` | `text`, `json` | no | `text` | Result format: text for people, or json for scripts and agents (results on stdout, errors as one JSON document on stderr) |

## Errors

| Code | Exit | Retryable | Meaning |
| --- | --- | --- | --- |
| `INVALID_ARGUMENT` | 2 | no | Input is missing or malformed; `details` names the argument and what it accepts. |
| `NOT_FOUND` | 3 | no | A named package, tap, command, or document does not exist, or is not declared. |
| `PRECONDITION_FAILED` | 3 | no | The machine or the config is not ready: mise is missing, the package is declared in a config file pkg does not edit, or removing it would also uninstall packages no config declares. |
| `UNAVAILABLE` | 5 | yes | The Homebrew or App Store catalog could not be reached; retry the same command. |
| `MISE_FAILED` | 6 | no | mise reported a failure; `details.output` holds the end of its output, and anything pkg declared for the failed install was taken back out of the config. |
| `INTERNAL` | 70 | no | A defect in pkg itself, or a stdout that cannot take the result; `message` says which. |

Every failure writes this document to stderr:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "ErrorDocument",
  "description": "What a failing command writes to stderr with `--output json`, as one JSON document.",
  "type": "object",
  "properties": {
    "error": {
      "$ref": "#/$defs/ErrorBody"
    }
  },
  "required": [
    "error"
  ],
  "$defs": {
    "ErrorBody": {
      "type": "object",
      "properties": {
        "code": {
          "description": "Stable code; branch on this, not on the message.",
          "$ref": "#/$defs/ErrorCode"
        },
        "message": {
          "description": "One sentence for a person.",
          "type": "string"
        },
        "retryable": {
          "description": "Whether running the same command again, unchanged, can succeed.",
          "type": "boolean"
        },
        "details": {
          "description": "Facts about the failure, such as the argument, the package, the config file, or mise's\noutput."
        },
        "suggestions": {
          "description": "Commands or changes that recover from the failure, most useful first.",
          "type": "array",
          "items": {
            "type": "string"
          }
        }
      },
      "required": [
        "code",
        "message",
        "retryable",
        "details",
        "suggestions"
      ]
    },
    "ErrorCode": {
      "description": "A stable error code. Codes, exit statuses, and retryability are part of the CLI contract.",
      "oneOf": [
        {
          "description": "Input is missing or malformed.",
          "type": "string",
          "const": "INVALID_ARGUMENT"
        },
        {
          "description": "A named package, tap, command, or document does not exist.",
          "type": "string",
          "const": "NOT_FOUND"
        },
        {
          "description": "The machine or the config is not in a state the command can work with.",
          "type": "string",
          "const": "PRECONDITION_FAILED"
        },
        {
          "description": "A package catalog could not be reached; retrying can succeed.",
          "type": "string",
          "const": "UNAVAILABLE"
        },
        {
          "description": "mise ran and reported a failure.",
          "type": "string",
          "const": "MISE_FAILED"
        },
        {
          "description": "A defect in `pkg` itself, or a stdout that cannot take the result.",
          "type": "string",
          "const": "INTERNAL"
        }
      ]
    }
  }
}
```

## Commands

### `install`

Install packages and record them in your mise config.

#### `pkg install`

Install packages and record them in your mise config.

Installs each package with mise and declares it in your global mise config, so `mise bootstrap` installs it again on a new machine. Homebrew formulae (`brew:`), casks (`brew-cask:`), App Store apps (`mas:`), and every other mise package manager go in `[bootstrap.packages]` of the file mise's `write_targets.packages` setting names; tools (`npm:prettier`, `github:cli/cli`, or a registry name such as `ripgrep`) go in `[tools]` of the `write_targets.tools` file, at `latest` unless you pass `@<version>`. A package some config already declares is installed where it is declared, and one that is already installed is left alone, so installing twice is safe. Packages install one at a time in the order given; when mise fails, pkg takes that package's new declaration back out and stops, and the error lists what was installed before it. mise's progress goes to stderr.

- **Effect:** modifies installed software and your global mise config
- **Requires:** `mise`
- **Dry run:** yes
- **Idempotent:** yes
- **Errors:** `INVALID_ARGUMENT`, `PRECONDITION_FAILED`, `MISE_FAILED`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `<packages>` | package: `^([a-z0-9][a-z0-9-]*:)?\S+$` | yes | — | One or more packages: `brew:wget`, `brew-cask:firefox`, `mas:497799835`, `npm:prettier`, or a tool from mise's registry such as `ripgrep` |
| `--dry-run` | boolean | no | — | Show what would change without installing, removing, or writing anything |

Examples:

```sh
# Preview installing GNU Hello, changing nothing
pkg install brew:hello --dry-run
# Install a formula and a cask
pkg install brew:hello brew-cask:iina
# Install ripgrep from mise's registry
pkg install ripgrep
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "status": {
      "$ref": "#/$defs/ReceiptStatus"
    },
    "packages": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/Installed"
      },
      "description": "One entry per package, in the order given."
    },
    "plan": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "For `dry-run`, the lines mise printed for what it would do; empty otherwise."
    }
  },
  "required": [
    "status",
    "packages",
    "plan"
  ],
  "description": "The outcome of `pkg install`.",
  "title": "InstallReceipt",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "ReceiptStatus": {
      "oneOf": [
        {
          "type": "string",
          "const": "completed",
          "description": "The change is made."
        },
        {
          "type": "string",
          "const": "dry-run",
          "description": "Nothing changed; the receipt shows what running without --dry-run would do."
        }
      ],
      "description": "Whether a modifying command made its change."
    },
    "Installed": {
      "type": "object",
      "properties": {
        "package": {
          "type": "string"
        },
        "kind": {
          "$ref": "#/$defs/Kind",
          "description": "`package` for `[bootstrap.packages]`, `tool` for `[tools]`."
        },
        "action": {
          "$ref": "#/$defs/Action",
          "description": "For `dry-run`, what running without --dry-run would do."
        },
        "config": {
          "type": "string",
          "description": "The config file that declares it, symlinks resolved."
        },
        "version": {
          "type": [
            "string",
            "null"
          ],
          "description": "The installed version, when mise reports one."
        }
      },
      "required": [
        "package",
        "kind",
        "action",
        "config"
      ]
    },
    "Kind": {
      "type": "string",
      "enum": [
        "package",
        "tool"
      ]
    },
    "Action": {
      "oneOf": [
        {
          "type": "string",
          "const": "added",
          "description": "pkg declared it and mise installed it."
        },
        {
          "type": "string",
          "const": "installed",
          "description": "A config already declared it; mise installed it."
        },
        {
          "type": "string",
          "const": "declared",
          "description": "pkg declared it, but its package manager is not available on this machine."
        },
        {
          "type": "string",
          "const": "unchanged",
          "description": "It was declared and installed already; nothing changed."
        }
      ]
    }
  }
}
```

### `remove`

Uninstall packages and take them out of your mise config.

#### `pkg remove`

Uninstall packages and take them out of your mise config.

Takes each package out of the config file mise writes to and uninstalls it. Homebrew formulae and casks are uninstalled by `mise bootstrap packages prune`, which removes every package no config declares, together with dependencies nothing else needs; pkg first checks that the prune would remove nothing besides what you named, and refuses with PRECONDITION_FAILED, changing nothing, when other undeclared packages are installed (`details.undeclared`; see `pkg prune --dry-run`). Tools are removed with `mise unuse`, which also uninstalls versions no other config uses. mise cannot uninstall other managers' packages, such as App Store apps; pkg takes them out of the config and lists them under `leftInstalled`. pkg edits only mise's write target: a package declared in another file, such as a config shared with other machines, fails with PRECONDITION_FAILED. --dry-run makes the same checks and changes nothing.

- **Effect:** modifies installed software and your global mise config
- **Requires:** `mise`
- **Dry run:** yes
- **Idempotent:** no
- **Errors:** `INVALID_ARGUMENT`, `NOT_FOUND`, `PRECONDITION_FAILED`, `MISE_FAILED`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `<packages>` | package: `^([a-z0-9][a-z0-9-]*:)?\S+$` | yes | — | One or more packages: `brew:wget`, `brew-cask:firefox`, `mas:497799835`, `npm:prettier`, or a tool from mise's registry such as `ripgrep` |
| `--dry-run` | boolean | no | — | Show what would change without installing, removing, or writing anything |

Examples:

```sh
# Preview removing a formula, changing nothing
pkg remove brew:wget --dry-run
# Remove a formula
pkg remove brew:wget
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "status": {
      "$ref": "#/$defs/ReceiptStatus"
    },
    "removed": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/Removed"
      },
      "description": "The declarations taken out of the config (for `dry-run`: that would be)."
    },
    "uninstalled": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Homebrew packages uninstalled, dependencies included, as `manager:name@version`; for\n`dry-run`, the packages you named, since their dependencies are known only after the\nconfig changes."
    },
    "leftInstalled": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/LeftInstalled"
      },
      "description": "Packages that stay installed, with the reason."
    }
  },
  "required": [
    "status",
    "removed",
    "uninstalled",
    "leftInstalled"
  ],
  "description": "The outcome of `pkg remove`.",
  "title": "RemoveReceipt",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "ReceiptStatus": {
      "oneOf": [
        {
          "type": "string",
          "const": "completed",
          "description": "The change is made."
        },
        {
          "type": "string",
          "const": "dry-run",
          "description": "Nothing changed; the receipt shows what running without --dry-run would do."
        }
      ],
      "description": "Whether a modifying command made its change."
    },
    "Removed": {
      "type": "object",
      "properties": {
        "package": {
          "type": "string"
        },
        "config": {
          "type": "string",
          "description": "The config file it was declared in, symlinks resolved."
        }
      },
      "required": [
        "package",
        "config"
      ]
    },
    "LeftInstalled": {
      "type": "object",
      "properties": {
        "package": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        }
      },
      "required": [
        "package",
        "reason"
      ]
    }
  }
}
```

### `list`

List the packages and tools your mise config declares.

#### `pkg list`

List the packages and tools your mise config declares.

Lists every `[bootstrap.packages]` entry of the global mise config, grouped by package manager, then every global `[tools]` entry, each with its installed version or `missing`. JSON has one item per entry with its `state` and `config` file. Pass --manager to show one manager (`tool` for tools), and --limit with --cursor to page; without --limit every entry is returned. Never changes anything.

- **Effect:** reads only
- **Requires:** `mise`
- **Dry run:** no
- **Idempotent:** yes
- **Errors:** `INVALID_ARGUMENT`, `PRECONDITION_FAILED`, `MISE_FAILED`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `--manager <string>` | string, repeatable | no | — | Only this package manager, such as `brew`, `brew-cask`, or `mas`, or `tool` for mise tools; repeat for several |
| `--limit <integer>` | integer, at least 1 | no | — | The most entries to return; all of them when absent |
| `--cursor <string>` | string | no | — | Resume after a previous page: the nextCursor it returned |

Examples:

```sh
# List everything
pkg list
# List the casks
pkg list --manager brew-cask
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "items": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/Listed"
      }
    },
    "hasMore": {
      "type": "boolean",
      "description": "Whether more items follow this page."
    },
    "nextCursor": {
      "type": [
        "string",
        "null"
      ],
      "description": "Pass as `--cursor` to get the next page; null when there is none."
    }
  },
  "required": [
    "items",
    "hasMore"
  ],
  "description": "One page of a collection.",
  "title": "Page",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "Listed": {
      "type": "object",
      "properties": {
        "package": {
          "type": "string"
        },
        "manager": {
          "type": "string",
          "description": "The package manager, such as `brew`, or `tool` for a mise tool."
        },
        "name": {
          "type": [
            "string",
            "null"
          ],
          "description": "A name mise knows besides the package, such as an App Store app's."
        },
        "requested": {
          "type": "string",
          "description": "The version the config asks for, usually `latest`."
        },
        "installed": {
          "type": [
            "string",
            "null"
          ]
        },
        "state": {
          "type": "string",
          "description": "`installed`, `missing`, or another state mise reports, such as `unavailable` for a\nmanager this machine does not have."
        },
        "config": {
          "type": [
            "string",
            "null"
          ],
          "description": "The config file that declares it, symlinks resolved."
        }
      },
      "required": [
        "package",
        "manager",
        "requested",
        "state"
      ],
      "description": "One declared package or tool."
    }
  }
}
```

### `search`

Find packages in Homebrew, mise's registry, and the App Store.

#### `pkg search`

Find packages in Homebrew, mise's registry, and the App Store.

Searches Homebrew's formulae (`brew:`) and casks (`brew-cask:`), the tools in mise's registry, and Mac apps in the App Store (`mas:`) by name and description, and returns the best matches with the exact `package` to pass to `pkg install`, its version, and whether your config already declares it. Homebrew's catalog is downloaded once a day into pkg's cache (about 7 MB); when Homebrew cannot be reached an older copy answers with a warning. The App Store is asked on every search; if it cannot be reached the other sources still answer, with a warning. Pass --manager to search fewer sources. Changes nothing but the cache.

- **Effect:** reads only
- **Requires:** `mise`, `network`
- **Dry run:** no
- **Idempotent:** yes
- **Errors:** `INVALID_ARGUMENT`, `PRECONDITION_FAILED`, `UNAVAILABLE`, `MISE_FAILED`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `<query>` | string, repeatable | yes | — | What to look for, such as `ffmpeg` or `video player`; several words need no quotes |
| `--manager <choice>` | `brew`, `brew-cask`, `tool`, `mas` | no | — | Search only this source: `brew`, `brew-cask`, `tool` (mise's registry), or `mas`; repeat for several |
| `--limit <integer>` | integer, 1–50 | no | `10` | The most matches to return |
| `--cursor <string>` | string | no | — | Resume after a previous page: the nextCursor it returned |

Examples:

```sh
# Find ffmpeg everywhere
pkg search ffmpeg
# Find a video player app among the casks
pkg search video player --manager brew-cask
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "items": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/Found"
      }
    },
    "hasMore": {
      "type": "boolean",
      "description": "Whether more items follow this page."
    },
    "nextCursor": {
      "type": [
        "string",
        "null"
      ],
      "description": "Pass as `--cursor` to get the next page; null when there is none."
    }
  },
  "required": [
    "items",
    "hasMore"
  ],
  "description": "One page of a collection.",
  "title": "Page",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "Found": {
      "type": "object",
      "properties": {
        "package": {
          "type": "string",
          "description": "What to pass to `pkg install`."
        },
        "manager": {
          "type": "string",
          "description": "`brew`, `brew-cask`, `tool`, or `mas`."
        },
        "name": {
          "type": "string",
          "description": "The display name: a cask's or app's title, else the package's name."
        },
        "description": {
          "type": "string"
        },
        "version": {
          "type": [
            "string",
            "null"
          ]
        },
        "declared": {
          "type": "boolean",
          "description": "Whether your mise config already declares it."
        },
        "score": {
          "type": "number",
          "format": "double",
          "description": "Relevance; higher is better."
        }
      },
      "required": [
        "package",
        "manager",
        "name",
        "description",
        "declared",
        "score"
      ],
      "description": "One package that matches the query."
    }
  }
}
```

### `info`

Describe one package and whether it is installed.

#### `pkg info`

Describe one package and whether it is installed.

Describes a package from its catalog: Homebrew's API for `brew:` and `brew-cask:` (description, homepage, version, license, dependencies, the apps a cask installs, caveats), the App Store for `mas:`, and mise for tools (backend, description, versions). Then adds what mise knows locally: the config file that declares it and the installed version. Formulae and casks from third-party taps are not in Homebrew's API, so only the local part is shown for them. A package no catalog knows and no config declares fails with NOT_FOUND. Never changes anything.

- **Effect:** reads only
- **Requires:** `mise`, `network`
- **Dry run:** no
- **Idempotent:** yes
- **Errors:** `INVALID_ARGUMENT`, `NOT_FOUND`, `PRECONDITION_FAILED`, `UNAVAILABLE`, `MISE_FAILED`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `<package>` | package: `^([a-z0-9][a-z0-9-]*:)?\S+$` | yes | — | The package: `brew:wget`, `brew-cask:firefox`, `mas:497799835`, `npm:prettier`, or a tool from mise's registry such as `ripgrep` |

Examples:

```sh
# Describe a formula
pkg info brew:wget
# Describe a cask
pkg info brew-cask:firefox
# Describe an App Store app
pkg info mas:937984704
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "package": {
      "type": "string"
    },
    "manager": {
      "type": "string",
      "description": "The package manager, such as `brew`, or `tool` for a mise tool."
    },
    "name": {
      "type": [
        "string",
        "null"
      ],
      "description": "A display name, such as a cask's or an app's title."
    },
    "description": {
      "type": [
        "string",
        "null"
      ]
    },
    "homepage": {
      "type": [
        "string",
        "null"
      ]
    },
    "version": {
      "type": [
        "string",
        "null"
      ],
      "description": "The newest version the catalog offers."
    },
    "license": {
      "type": [
        "string",
        "null"
      ]
    },
    "dependencies": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Formulae a Homebrew formula needs."
    },
    "apps": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Apps a cask puts in /Applications."
    },
    "caveats": {
      "type": [
        "string",
        "null"
      ]
    },
    "backend": {
      "type": [
        "string",
        "null"
      ],
      "description": "For a tool, the mise backend that installs it, such as `aqua:jqlang/jq`."
    },
    "deprecated": {
      "type": "boolean",
      "description": "Whether the catalog marks it deprecated."
    },
    "config": {
      "type": [
        "string",
        "null"
      ],
      "description": "The config file that declares it, symlinks resolved; null when none does."
    },
    "installed": {
      "type": [
        "string",
        "null"
      ],
      "description": "The installed version; null when it is not installed."
    }
  },
  "required": [
    "package",
    "manager",
    "dependencies",
    "apps",
    "deprecated"
  ],
  "description": "Everything pkg knows about one package.",
  "title": "PackageInfo",
  "$schema": "https://json-schema.org/draft/2020-12/schema"
}
```

### `outdated`

List installed packages that have a newer version.

#### `pkg outdated`

List installed packages that have a newer version.

Lists every installed package and tool that `pkg update` would update, with the installed and the newest version, mise itself first. Homebrew formulae and casks come from `mise bootstrap packages upgrade --dry-run`, which refreshes Homebrew's metadata; App Store apps are compared with the App Store's current version; tools come from `mise outdated --global`; mise's newest release comes from `mise version`, before mise's minimum release age, so `pkg update` can leave a very new mise release for later. Casks mise will not update, such as apps that update themselves, and a newer mise that a package manager installed are listed under `skipped` with the reason. The whole list is returned at once. Never changes anything.

- **Effect:** reads only
- **Requires:** `mise`, `network`
- **Dry run:** no
- **Idempotent:** yes
- **Errors:** `PRECONDITION_FAILED`, `MISE_FAILED`, `INTERNAL`

Examples:

```sh
# See what update would update
pkg outdated
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "items": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/OutdatedPackage"
      }
    },
    "skipped": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/Skipped"
      },
      "description": "Packages mise will not update, with its reason."
    }
  },
  "required": [
    "items",
    "skipped"
  ],
  "description": "Every outdated package, never paged.",
  "title": "OutdatedReport",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "OutdatedPackage": {
      "type": "object",
      "properties": {
        "package": {
          "type": "string"
        },
        "installed": {
          "type": [
            "string",
            "null"
          ]
        },
        "latest": {
          "type": "string"
        }
      },
      "required": [
        "package",
        "latest"
      ]
    },
    "Skipped": {
      "type": "object",
      "properties": {
        "package": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        }
      },
      "required": [
        "package",
        "reason"
      ]
    }
  }
}
```

### `update`

Update installed packages to their newest versions.

#### `pkg update`

Update installed packages to their newest versions.

Updates the named packages, or with none mise itself and then every installed package and global tool: `mise self-update`, then `mise bootstrap packages upgrade` for Homebrew, the App Store, and the other package managers, and `mise upgrade --global` for tools, which include pkg once pkg is handed to mise. A mise that a package manager installed is left to it and listed under `skipped`; when `mise self-update` fails, nothing else changes. Packages that are declared but not installed are left to `pkg sync`. The receipt lists each package whose installed version changed, mise first. --dry-run lists what would be updated, as `pkg outdated` does, and changes nothing. mise's progress goes to stderr.

- **Effect:** modifies installed software
- **Requires:** `mise`, `network`
- **Dry run:** yes
- **Idempotent:** yes
- **Errors:** `INVALID_ARGUMENT`, `NOT_FOUND`, `PRECONDITION_FAILED`, `MISE_FAILED`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `<packages>` | package: `^([a-z0-9][a-z0-9-]*:)?\S+$` | no | — | Only these packages, such as `brew:wget` or `ripgrep`; everything when absent |
| `--dry-run` | boolean | no | — | Show what would change without installing, removing, or writing anything |

Examples:

```sh
# Preview updating everything
pkg update --dry-run
# Update one formula
pkg update brew:wget
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "status": {
      "$ref": "#/$defs/ReceiptStatus"
    },
    "updated": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/Updated"
      },
      "description": "Packages whose version changed (for `dry-run`: would change), mise first."
    },
    "skipped": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/Skipped"
      },
      "description": "What is left alone, with the reason: mise when a package manager installed it, and for\n`dry-run` the packages mise will not update."
    }
  },
  "required": [
    "status",
    "updated",
    "skipped"
  ],
  "description": "The outcome of `pkg update`.",
  "title": "UpdateReceipt",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "ReceiptStatus": {
      "oneOf": [
        {
          "type": "string",
          "const": "completed",
          "description": "The change is made."
        },
        {
          "type": "string",
          "const": "dry-run",
          "description": "Nothing changed; the receipt shows what running without --dry-run would do."
        }
      ],
      "description": "Whether a modifying command made its change."
    },
    "Updated": {
      "type": "object",
      "properties": {
        "package": {
          "type": "string"
        },
        "from": {
          "type": [
            "string",
            "null"
          ]
        },
        "to": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "package"
      ]
    },
    "Skipped": {
      "type": "object",
      "properties": {
        "package": {
          "type": "string"
        },
        "reason": {
          "type": "string"
        }
      },
      "required": [
        "package",
        "reason"
      ]
    }
  }
}
```

### `sync`

Install everything your mise config declares that is missing.

#### `pkg sync`

Install everything your mise config declares that is missing.

Brings this machine in line with the config's packages and tools: `mise bootstrap packages apply` installs every missing `[bootstrap.packages]` entry (and removes packages declared with `state = "absent"`, where the manager can), and `mise install` installs every missing global tool. Use it after pulling config changes made on another machine; unlike `mise bootstrap` it leaves dotfiles, services, and settings alone. Packages whose manager this machine lacks, such as `apt:` on macOS, are skipped. --dry-run lists what is missing and changes nothing. mise's progress goes to stderr.

- **Effect:** modifies installed software
- **Requires:** `mise`
- **Dry run:** yes
- **Idempotent:** yes
- **Errors:** `INVALID_ARGUMENT`, `PRECONDITION_FAILED`, `MISE_FAILED`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `--dry-run` | boolean | no | — | Show what would change without installing, removing, or writing anything |

Examples:

```sh
# See what is missing
pkg sync --dry-run
# Install what is missing
pkg sync
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "status": {
      "$ref": "#/$defs/ReceiptStatus"
    },
    "installed": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Packages and tools that were missing and are now installed (for `dry-run`: are missing)."
    },
    "missing": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Packages still missing after the run; empty for `dry-run`."
    }
  },
  "required": [
    "status",
    "installed",
    "missing"
  ],
  "description": "The outcome of `pkg sync`.",
  "title": "SyncReceipt",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "ReceiptStatus": {
      "oneOf": [
        {
          "type": "string",
          "const": "completed",
          "description": "The change is made."
        },
        {
          "type": "string",
          "const": "dry-run",
          "description": "Nothing changed; the receipt shows what running without --dry-run would do."
        }
      ],
      "description": "Whether a modifying command made its change."
    }
  }
}
```

### `prune`

Uninstall Homebrew packages no config declares.

#### `pkg prune`

Uninstall Homebrew packages no config declares.

Runs `mise bootstrap packages prune` for formulae and casks: it uninstalls every formula that no config declares and no declared package needs, and every cask mise installed that no config declares. Casks that update themselves or run install scripts cannot be proven unchanged and are kept unless you pass --force, which removes the files mise recorded but not what the cask's own scripts created. Packages that `pkg remove` would otherwise take with it show up here first. --dry-run lists what would be uninstalled and changes nothing.

- **Effect:** modifies installed software
- **Requires:** `mise`
- **Dry run:** yes
- **Idempotent:** yes
- **Errors:** `INVALID_ARGUMENT`, `PRECONDITION_FAILED`, `MISE_FAILED`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `--force` | boolean | no | — | Also uninstall casks mise cannot prove are unchanged |
| `--dry-run` | boolean | no | — | Show what would change without installing, removing, or writing anything |

Examples:

```sh
# See what a prune would uninstall
pkg prune --dry-run
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "status": {
      "$ref": "#/$defs/ReceiptStatus"
    },
    "uninstalled": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "What was uninstalled (for `dry-run`: would be), as `manager:name@version`."
    }
  },
  "required": [
    "status",
    "uninstalled"
  ],
  "description": "The outcome of `pkg prune`.",
  "title": "PruneReceipt",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "ReceiptStatus": {
      "oneOf": [
        {
          "type": "string",
          "const": "completed",
          "description": "The change is made."
        },
        {
          "type": "string",
          "const": "dry-run",
          "description": "Nothing changed; the receipt shows what running without --dry-run would do."
        }
      ],
      "description": "Whether a modifying command made its change."
    }
  }
}
```

### `where`

Print where a formula or tool is installed.

#### `pkg where`

Print where a formula or tool is installed.

Prints the directory an installed Homebrew formula (its `opt` path, such as /opt/homebrew/opt/wget, which keg-only formulae need on PATH by hand) or mise tool lives in, as `brew --prefix` and `mise where` do. Casks, App Store apps, and other managers' packages have no such directory and fail with INVALID_ARGUMENT; something not installed fails with MISE_FAILED. Never changes anything.

- **Effect:** reads only
- **Requires:** `mise`
- **Dry run:** no
- **Idempotent:** yes
- **Errors:** `INVALID_ARGUMENT`, `PRECONDITION_FAILED`, `MISE_FAILED`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `<package>` | package: `^([a-z0-9][a-z0-9-]*:)?\S+$` | yes | — | The package: `brew:wget`, `brew-cask:firefox`, `mas:497799835`, `npm:prettier`, or a tool from mise's registry such as `ripgrep` |

Examples:

```sh
# Find wget's prefix
pkg where brew:wget
# Find jq's install directory
pkg where jq
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "package": {
      "type": "string"
    },
    "path": {
      "type": "string",
      "description": "The absolute directory."
    }
  },
  "required": [
    "package",
    "path"
  ],
  "title": "Location",
  "$schema": "https://json-schema.org/draft/2020-12/schema"
}
```

### `tap`

List, add, and remove Homebrew tap URLs.

#### `pkg tap list`

List the Homebrew tap URLs your mise config declares.

Lists `[bootstrap.brew.taps]` across the global config files: each tap with its Git URL and the file that declares it. mise finds a tap at `https://github.com/<owner>/homebrew-<name>` without an entry, so most taps are not listed. Never changes anything.

- **Effect:** reads only
- **Requires:** `mise`
- **Dry run:** no
- **Idempotent:** yes
- **Errors:** `PRECONDITION_FAILED`, `MISE_FAILED`, `INTERNAL`

Examples:

```sh
# List tap URLs
pkg tap list
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "items": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/TapEntry"
      }
    }
  },
  "required": [
    "items"
  ],
  "description": "Every declared tap URL.",
  "title": "TapList",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "TapEntry": {
      "type": "object",
      "properties": {
        "tap": {
          "type": "string"
        },
        "url": {
          "type": "string"
        },
        "config": {
          "type": "string",
          "description": "The config file that declares it, symlinks resolved."
        }
      },
      "required": [
        "tap",
        "url",
        "config"
      ]
    }
  }
}
```

#### `pkg tap add`

Record where a Homebrew tap's Git repository lives.

Adds `"<owner>/<name>" = "<url>"` to `[bootstrap.brew.taps]` in the file mise writes packages to, with `mise bootstrap packages brew tap`. Only taps whose repository is not `https://github.com/<owner>/homebrew-<name>` need one, and only GitHub URLs are supported. Then install from it with `pkg install brew:<owner>/<name>/<formula>`. --dry-run shows the change without writing it.

- **Effect:** modifies your global mise config
- **Requires:** `mise`
- **Dry run:** yes
- **Idempotent:** yes
- **Errors:** `INVALID_ARGUMENT`, `PRECONDITION_FAILED`, `MISE_FAILED`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `<tap>` | string | yes | — | The tap, as `owner/name`, which packages name as `brew:owner/name/<formula>` |
| `<url>` | string | yes | — | The tap's GitHub repository URL |
| `--dry-run` | boolean | no | — | Show what would change without installing, removing, or writing anything |

Examples:

```sh
# Preview recording a tap's URL
pkg tap add acme/tools https://github.com/acme/brew-tools.git --dry-run
# Record a tap's URL
pkg tap add acme/tools https://github.com/acme/brew-tools.git
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "status": {
      "$ref": "#/$defs/ReceiptStatus"
    },
    "action": {
      "type": "string",
      "description": "`added` or `removed`; for `dry-run`, what would happen."
    },
    "tap": {
      "type": "string"
    },
    "url": {
      "type": [
        "string",
        "null"
      ]
    },
    "config": {
      "type": "string",
      "description": "The config file changed, symlinks resolved."
    }
  },
  "required": [
    "status",
    "action",
    "tap",
    "config"
  ],
  "description": "The outcome of `pkg tap add` and `pkg tap remove`.",
  "title": "TapReceipt",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "ReceiptStatus": {
      "oneOf": [
        {
          "type": "string",
          "const": "completed",
          "description": "The change is made."
        },
        {
          "type": "string",
          "const": "dry-run",
          "description": "Nothing changed; the receipt shows what running without --dry-run would do."
        }
      ],
      "description": "Whether a modifying command made its change."
    }
  }
}
```

#### `pkg tap remove`

Forget a Homebrew tap's URL.

Removes the tap's entry from `[bootstrap.brew.taps]` with `mise bootstrap packages brew untap`. Packages from the tap stay declared and installed; check `pkg list --manager brew` first. pkg edits only mise's write target, so a tap declared in another file fails with PRECONDITION_FAILED. --dry-run shows the change without writing it.

- **Effect:** modifies your global mise config
- **Requires:** `mise`
- **Dry run:** yes
- **Idempotent:** no
- **Errors:** `NOT_FOUND`, `PRECONDITION_FAILED`, `MISE_FAILED`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `<tap>` | string | yes | — | The tap, as `owner/name`, which packages name as `brew:owner/name/<formula>` |
| `--dry-run` | boolean | no | — | Show what would change without installing, removing, or writing anything |

Examples:

```sh
# Preview forgetting a tap
pkg tap remove acme/tools --dry-run
# Forget a tap
pkg tap remove acme/tools
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "status": {
      "$ref": "#/$defs/ReceiptStatus"
    },
    "action": {
      "type": "string",
      "description": "`added` or `removed`; for `dry-run`, what would happen."
    },
    "tap": {
      "type": "string"
    },
    "url": {
      "type": [
        "string",
        "null"
      ]
    },
    "config": {
      "type": "string",
      "description": "The config file changed, symlinks resolved."
    }
  },
  "required": [
    "status",
    "action",
    "tap",
    "config"
  ],
  "description": "The outcome of `pkg tap add` and `pkg tap remove`.",
  "title": "TapReceipt",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "ReceiptStatus": {
      "oneOf": [
        {
          "type": "string",
          "const": "completed",
          "description": "The change is made."
        },
        {
          "type": "string",
          "const": "dry-run",
          "description": "Nothing changed; the receipt shows what running without --dry-run would do."
        }
      ],
      "description": "Whether a modifying command made its change."
    }
  }
}
```

### `setup`

Install mise and get pkg ready on a machine with nothing to restore.

#### `pkg setup`

Install mise and get pkg ready on a machine with nothing to restore.

Gets a new Mac or Linux machine ready for `pkg install` when there is no configuration to restore (use `pkg bootstrap --from` for that). When pkg finds no mise (`PKG_MISE`, `mise` on PATH, `~/.local/bin/mise`), it runs mise's own installer (https://mise.run, or `PKG_MISE_INSTALLER`) for `~/.local/bin/mise`, and it refuses a mise older than 2026.11.9. When no global mise config exists, so nothing else will put mise's tools on PATH, it adds mise's shims directory to pkg's block in the startup file of your login shell (`~/.zshenv`, `~/.bashrc` and `~/.profile`, or fish's `conf.d/pkg.fish`) and prints the command that loads it now; --no-modify-path prints the line instead. Then it hands pkg to mise: it declares and installs `github:gotpkg/pkg` as `pkg install` would, unless a config declares it, and replaces the installer's `~/.local/bin/pkg` with a link to mise's shim, never with an older release. Its declaration creates a global config on a machine without one, which `pkg bootstrap --from` then refuses to link over. Running it again changes nothing already in place. Fails on Windows.

- **Effect:** modifies mise, shell startup files, and pkg's own installation
- **Requires:** `network`
- **Dry run:** yes
- **Idempotent:** yes
- **Errors:** `PRECONDITION_FAILED`, `UNAVAILABLE`, `MISE_FAILED`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `--no-modify-path` | boolean | no | — | Leave shell startup files alone and print the PATH line to add instead |
| `--dry-run` | boolean | no | — | Show what would change without installing, removing, or writing anything |

Examples:

```sh
# See what setup would do
pkg setup --dry-run
# Install mise and hand pkg to it
pkg setup
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "status": {
      "$ref": "#/$defs/ReceiptStatus"
    },
    "mise": {
      "$ref": "#/$defs/MiseSetup"
    },
    "path": {
      "anyOf": [
        {
          "$ref": "#/$defs/PathChange"
        },
        {
          "type": "null"
        }
      ],
      "description": "What pkg did to put mise's shims on `PATH`; null when nothing was needed, because a\nconfiguration exists or the shims are on `PATH` already."
    },
    "handoff": {
      "$ref": "#/$defs/Handoff"
    }
  },
  "required": [
    "status",
    "mise",
    "handoff"
  ],
  "description": "The outcome of `pkg setup`.",
  "title": "SetupReceipt",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "ReceiptStatus": {
      "oneOf": [
        {
          "type": "string",
          "const": "completed",
          "description": "The change is made."
        },
        {
          "type": "string",
          "const": "dry-run",
          "description": "Nothing changed; the receipt shows what running without --dry-run would do."
        }
      ],
      "description": "Whether a modifying command made its change."
    },
    "MiseSetup": {
      "type": "object",
      "properties": {
        "program": {
          "type": "string",
          "description": "The executable."
        },
        "version": {
          "type": [
            "string",
            "null"
          ],
          "description": "Its version; null in a dry run that would install it."
        },
        "action": {
          "$ref": "#/$defs/MiseAction",
          "description": "`found` when pkg used the mise it found, `installed` when it ran mise's installer (for\n`dry-run`: would run)."
        }
      },
      "required": [
        "program",
        "action"
      ],
      "description": "The mise `setup` and `bootstrap` use."
    },
    "MiseAction": {
      "type": "string",
      "enum": [
        "found",
        "installed"
      ]
    },
    "PathChange": {
      "type": "object",
      "properties": {
        "action": {
          "$ref": "#/$defs/PathAction",
          "description": "`added` when pkg wrote its block, `unchanged` when the block already had the shims, and\n`printed` when the line is for you to add (another shell, a linked startup file, or\n`--no-modify-path`)."
        },
        "directory": {
          "type": "string",
          "description": "mise's shims directory."
        },
        "files": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "The startup files pkg's block is in."
        },
        "line": {
          "type": "string",
          "description": "The line that puts `~/.local/bin` and the shims first on `PATH`."
        },
        "load": {
          "type": [
            "string",
            "null"
          ],
          "description": "The command that loads the change in the current terminal, when a file changed."
        }
      },
      "required": [
        "action",
        "directory",
        "files",
        "line"
      ],
      "description": "What `setup` did about mise's shims being on `PATH`."
    },
    "PathAction": {
      "type": "string",
      "enum": [
        "added",
        "unchanged",
        "printed"
      ]
    },
    "Handoff": {
      "type": "object",
      "properties": {
        "declaration": {
          "$ref": "#/$defs/Declaration",
          "description": "`added` when pkg declared `github:gotpkg/pkg`, `existing` when a config already did."
        },
        "config": {
          "type": [
            "string",
            "null"
          ],
          "description": "The config file that declares it; null in a dry run that would add it."
        },
        "version": {
          "type": [
            "string",
            "null"
          ],
          "description": "The version of pkg mise runs; null when it is not known."
        },
        "link": {
          "$ref": "#/$defs/Link",
          "description": "What happened to `~/.local/bin/pkg`: `linked` (replaced by a link to mise's shim),\n`already-linked`, `kept` (see `reason`), or `none` (nothing there)."
        },
        "path": {
          "type": "string",
          "description": "`~/.local/bin/pkg`."
        },
        "reason": {
          "type": [
            "string",
            "null"
          ],
          "description": "Why `~/.local/bin/pkg` was kept."
        },
        "others": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Other pkg executables on `PATH`, which a shell may run instead."
        }
      },
      "required": [
        "declaration",
        "link",
        "path",
        "others"
      ],
      "description": "What happened when pkg was handed to mise."
    },
    "Declaration": {
      "type": "string",
      "enum": [
        "added",
        "existing"
      ]
    },
    "Link": {
      "type": "string",
      "enum": [
        "linked",
        "already-linked",
        "kept",
        "none"
      ]
    }
  }
}
```

### `bootstrap`

Restore a machine from your mise configuration.

#### `pkg bootstrap`

Restore a machine from your mise configuration with mise bootstrap.

Runs `mise bootstrap`, which applies everything a configuration declares: packages, tools, dotfiles, settings, services, hooks, and the `bootstrap` task. Name a dotfiles repository with --from <url> --directory <checkout> --config-dir <directory>: mise clones it (or reuses a checkout whose origin is <url>) and links its global config directory, normally `~/.config/mise`, to <config-dir> inside the checkout; pkg refuses before changing anything when that directory already holds another configuration. Name one existing config file with --config, applied with the global configuration. With no source it applies the global configuration, as every run after the first does. --env selects configuration environments, --save-env makes them the default, and --update pulls new commits and refreshes package metadata first; pkg never prunes, forces, or answers for you. When mise is missing it is installed first, as `pkg setup` does. Startup files that hold only the installer's PATH block are moved aside while mise runs and put back where the configuration deployed nothing. With text output in a terminal, mise and the programs it runs may ask you for passwords and answers there; with --output json or without a terminal nothing waits, and a step that needs you fails. A successful run hands pkg to mise. A configuration can run any command, so name only one you have reviewed. --dry-run runs `mise bootstrap --dry-run`, without the `files` part when a templated `[bootstrap.files]` entry could run a command, and changes nothing. Fails on Windows.

- **Effect:** modifies the whole machine as the configuration declares it, and pkg's own installation
- **Requires:** `network`
- **Dry run:** yes
- **Idempotent:** no
- **Errors:** `INVALID_ARGUMENT`, `NOT_FOUND`, `PRECONDITION_FAILED`, `UNAVAILABLE`, `MISE_FAILED`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `--from <string>` | string | no | — | A dotfiles repository's Git URL; needs --directory and --config-dir |
| `--directory <string>` | string | no | — | Where the repository's checkout is or goes, such as ~/.dotfiles |
| `--config-dir <string>` | string | no | — | The checkout's directory that holds your global mise config, such as `mise` (`.` for the root) |
| `--config <string>` | string | no | — | One existing mise config file to apply with the global configuration |
| `--env <string>` | string, repeatable | no | — | A configuration environment to select, such as `work`; repeatable |
| `--save-env` | boolean | no | — | Make the --env selection the default for later runs |
| `--update` | boolean | no | — | Pull new commits and refresh package metadata first |
| `--dry-run` | boolean | no | — | Show what would change without installing, removing, or writing anything |

Examples:

```sh
# Preview applying your global configuration
pkg bootstrap --dry-run
# Apply your global configuration
pkg bootstrap
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "status": {
      "$ref": "#/$defs/ReceiptStatus"
    },
    "mise": {
      "$ref": "#/$defs/MiseSetup"
    },
    "source": {
      "$ref": "#/$defs/SourceReport"
    },
    "configDir": {
      "type": "string",
      "description": "mise's global configuration directory, which `--from` links into the checkout."
    },
    "environments": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "The configuration environments selected."
    },
    "startupFiles": {
      "$ref": "#/$defs/StartupFiles"
    },
    "notPreviewed": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "For `dry-run`, what the preview leaves out, and why."
    },
    "followUp": {
      "type": [
        "array",
        "null"
      ],
      "items": {
        "type": "string"
      },
      "description": "The follow-up work mise listed; null when mise wrote to the terminal instead."
    },
    "handoff": {
      "anyOf": [
        {
          "$ref": "#/$defs/Handoff"
        },
        {
          "type": "null"
        }
      ],
      "description": "What happened when pkg was handed to mise; null in a dry run from a repository, whose\nconfiguration does not exist yet."
    }
  },
  "required": [
    "status",
    "mise",
    "source",
    "configDir",
    "environments",
    "startupFiles",
    "notPreviewed"
  ],
  "description": "The outcome of `pkg bootstrap`.",
  "title": "BootstrapReceipt",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "ReceiptStatus": {
      "oneOf": [
        {
          "type": "string",
          "const": "completed",
          "description": "The change is made."
        },
        {
          "type": "string",
          "const": "dry-run",
          "description": "Nothing changed; the receipt shows what running without --dry-run would do."
        }
      ],
      "description": "Whether a modifying command made its change."
    },
    "MiseSetup": {
      "type": "object",
      "properties": {
        "program": {
          "type": "string",
          "description": "The executable."
        },
        "version": {
          "type": [
            "string",
            "null"
          ],
          "description": "Its version; null in a dry run that would install it."
        },
        "action": {
          "$ref": "#/$defs/MiseAction",
          "description": "`found` when pkg used the mise it found, `installed` when it ran mise's installer (for\n`dry-run`: would run)."
        }
      },
      "required": [
        "program",
        "action"
      ],
      "description": "The mise `setup` and `bootstrap` use."
    },
    "MiseAction": {
      "type": "string",
      "enum": [
        "found",
        "installed"
      ]
    },
    "SourceReport": {
      "type": "object",
      "properties": {
        "kind": {
          "$ref": "#/$defs/SourceKind",
          "description": "`global`, `repository`, or `config`."
        },
        "url": {
          "type": [
            "string",
            "null"
          ],
          "description": "The repository's URL."
        },
        "directory": {
          "type": [
            "string",
            "null"
          ],
          "description": "The checkout."
        },
        "configDir": {
          "type": [
            "string",
            "null"
          ],
          "description": "The checkout's directory that is the global configuration."
        },
        "config": {
          "type": [
            "string",
            "null"
          ],
          "description": "The config file applied with the global configuration."
        }
      },
      "required": [
        "kind"
      ],
      "description": "The configuration bootstrap applied."
    },
    "SourceKind": {
      "type": "string",
      "enum": [
        "global",
        "repository",
        "config"
      ]
    },
    "StartupFiles": {
      "type": "object",
      "properties": {
        "movedAside": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "The files pkg deleted before mise applied the configuration (for `dry-run`: would delete)."
        },
        "restored": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "The ones pkg wrote back because the configuration deployed nothing there."
        }
      },
      "required": [
        "movedAside",
        "restored"
      ],
      "description": "Startup files that held nothing but pkg's `PATH` block."
    },
    "Handoff": {
      "type": "object",
      "properties": {
        "declaration": {
          "$ref": "#/$defs/Declaration",
          "description": "`added` when pkg declared `github:gotpkg/pkg`, `existing` when a config already did."
        },
        "config": {
          "type": [
            "string",
            "null"
          ],
          "description": "The config file that declares it; null in a dry run that would add it."
        },
        "version": {
          "type": [
            "string",
            "null"
          ],
          "description": "The version of pkg mise runs; null when it is not known."
        },
        "link": {
          "$ref": "#/$defs/Link",
          "description": "What happened to `~/.local/bin/pkg`: `linked` (replaced by a link to mise's shim),\n`already-linked`, `kept` (see `reason`), or `none` (nothing there)."
        },
        "path": {
          "type": "string",
          "description": "`~/.local/bin/pkg`."
        },
        "reason": {
          "type": [
            "string",
            "null"
          ],
          "description": "Why `~/.local/bin/pkg` was kept."
        },
        "others": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Other pkg executables on `PATH`, which a shell may run instead."
        }
      },
      "required": [
        "declaration",
        "link",
        "path",
        "others"
      ],
      "description": "What happened when pkg was handed to mise."
    },
    "Declaration": {
      "type": "string",
      "enum": [
        "added",
        "existing"
      ]
    },
    "Link": {
      "type": "string",
      "enum": [
        "linked",
        "already-linked",
        "kept",
        "none"
      ]
    }
  }
}
```

### `cli`

Find the command for a task.

#### `pkg cli search`

Find the pkg command for a task described in plain words.

Ranks every command against the query by its name, summary, description, arguments, and declared synonyms, and returns the best matches with the exact command to run and whether it reads or modifies state. Runs locally and never changes anything. Inspect a match with `pkg schema <command>` before running it.

- **Effect:** reads only
- **Requires:** nothing beyond `pkg`
- **Dry run:** no
- **Idempotent:** yes
- **Errors:** `INVALID_ARGUMENT`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `<query>` | string, repeatable | yes | — | What you want to do, in plain words; several words need no quotes |
| `--limit <integer>` | integer, 1–20 | no | `5` | The most matches to return |
| `--cursor <string>` | string | no | — | Resume after a previous page: the nextCursor it returned |

Examples:

```sh
# Find how to install a package
pkg cli search 'install a cask'
# Find how to uninstall something, three matches at most
pkg cli search 'uninstall a formula' --limit 3
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "items": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/CommandMatch"
      }
    },
    "hasMore": {
      "type": "boolean",
      "description": "Whether more items follow this page."
    },
    "nextCursor": {
      "type": [
        "string",
        "null"
      ],
      "description": "Pass as `--cursor` to get the next page; null when there is none."
    }
  },
  "required": [
    "items",
    "hasMore"
  ],
  "description": "One page of a collection.",
  "title": "Page",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "CommandMatch": {
      "type": "object",
      "properties": {
        "command": {
          "type": "string",
          "description": "The exact command to run, without its arguments."
        },
        "id": {
          "type": "string",
          "description": "The registry identifier, such as `tap.add`."
        },
        "summary": {
          "type": "string"
        },
        "effect": {
          "type": "string",
          "description": "`reads`, or `modifies <what>`."
        },
        "score": {
          "type": "number",
          "format": "double",
          "description": "Relevance from 0 to 1."
        }
      },
      "required": [
        "command",
        "id",
        "summary",
        "effect",
        "score"
      ],
      "description": "One command that fits the query."
    }
  }
}
```

### `schema`

Inspect a command's full specification.

#### `pkg schema`

Show a command's arguments, defaults, effects, errors, examples, and output schema.

With no command, lists every command with its summary and effect. With a command path such as `tap add` (or its identifier, `tap.add`, or an alias such as `rm`), returns that command's full registry entry: each argument with its type, whether it is required, its default, and its accepted values or bounds; whether it reads or modifies state and what it requires; whether it supports --dry-run and is idempotent; the error codes it can return with their exit statuses; examples; and the JSON Schema of its result. Never changes anything.

- **Effect:** reads only
- **Requires:** nothing beyond `pkg`
- **Dry run:** no
- **Idempotent:** yes
- **Errors:** `NOT_FOUND`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `<command>` | string, repeatable | no | — | The command path, such as `install` or `tap add`; omit to list every command |

Examples:

```sh
# List every command
pkg schema
# Inspect install
pkg schema install
```

Result schema:

```json
{
  "anyOf": [
    {
      "$ref": "#/$defs/CommandList",
      "description": "With no command: every command."
    },
    {
      "$ref": "#/$defs/CommandDescription",
      "description": "With a command: its full specification."
    }
  ],
  "title": "SchemaResult",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "CommandList": {
      "type": "object",
      "properties": {
        "items": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/CommandSummary"
          }
        }
      },
      "required": [
        "items"
      ]
    },
    "CommandSummary": {
      "type": "object",
      "properties": {
        "command": {
          "type": "string"
        },
        "id": {
          "type": "string"
        },
        "summary": {
          "type": "string"
        },
        "effect": {
          "type": "string",
          "description": "`reads`, or `modifies <what>`."
        }
      },
      "required": [
        "command",
        "id",
        "summary",
        "effect"
      ]
    },
    "CommandDescription": {
      "type": "object",
      "properties": {
        "id": {
          "type": "string",
          "description": "Stable identifier: the command path joined with dots."
        },
        "command": {
          "type": "string",
          "description": "The command to run, without arguments."
        },
        "aliases": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "Other names the command's last word answers to, such as `rm` for `remove`."
        },
        "summary": {
          "type": "string"
        },
        "description": {
          "type": "string"
        },
        "arguments": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/ArgumentDescription"
          },
          "description": "The command's own arguments, then the options every command accepts."
        },
        "effect": {
          "$ref": "#/$defs/EffectDescription"
        },
        "requires": {
          "type": "array",
          "items": {
            "type": "string"
          },
          "description": "What the command needs besides pkg: `mise` (the executable, run in your home directory)\nor `network` (the Homebrew or App Store catalog)."
        },
        "dryRun": {
          "type": "boolean",
          "description": "Whether the command accepts `--dry-run`."
        },
        "idempotent": {
          "type": "boolean",
          "description": "Whether running it twice with the same input leaves the same state as running it once."
        },
        "errors": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/ErrorDescription"
          },
          "description": "Every error the command can report on stderr."
        },
        "examples": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/ExampleDescription"
          }
        },
        "output": {
          "description": "JSON Schema of the result `--output json` writes to stdout."
        }
      },
      "required": [
        "id",
        "command",
        "aliases",
        "summary",
        "description",
        "arguments",
        "effect",
        "requires",
        "dryRun",
        "idempotent",
        "errors",
        "examples",
        "output"
      ],
      "description": "A command's full specification, as `pkg schema <command>` returns it."
    },
    "ArgumentDescription": {
      "type": "object",
      "properties": {
        "name": {
          "type": "string"
        },
        "kind": {
          "type": "string",
          "description": "`positional`, `option`, or `flag`."
        },
        "usage": {
          "type": "string",
          "description": "How to write it: `<package>`, `--limit <integer>`, `--dry-run`."
        },
        "type": {
          "type": "string",
          "description": "`string`, `integer`, `package` (matches `pattern`), `choice`, or `boolean`."
        },
        "required": {
          "type": "boolean"
        },
        "default": {
          "type": [
            "string",
            "null"
          ]
        },
        "multiple": {
          "type": "boolean",
          "description": "Whether it accepts several values."
        },
        "acceptedValues": {
          "type": [
            "array",
            "null"
          ],
          "items": {
            "type": "string"
          },
          "description": "The only values a `choice` accepts."
        },
        "minimum": {
          "type": [
            "integer",
            "null"
          ],
          "format": "uint64",
          "minimum": 0,
          "description": "The smallest `integer` accepted."
        },
        "maximum": {
          "type": [
            "integer",
            "null"
          ],
          "format": "uint64",
          "minimum": 0,
          "description": "The largest `integer` accepted."
        },
        "pattern": {
          "type": [
            "string",
            "null"
          ],
          "description": "The regular expression a `package` matches."
        },
        "summary": {
          "type": "string"
        }
      },
      "required": [
        "name",
        "kind",
        "usage",
        "type",
        "required",
        "multiple",
        "summary"
      ]
    },
    "EffectDescription": {
      "type": "object",
      "properties": {
        "kind": {
          "type": "string",
          "description": "`reads` or `modifies`."
        },
        "what": {
          "type": [
            "string",
            "null"
          ],
          "description": "What a modifying command changes."
        }
      },
      "required": [
        "kind"
      ]
    },
    "ErrorDescription": {
      "type": "object",
      "properties": {
        "code": {
          "type": "string"
        },
        "exit": {
          "type": "integer",
          "format": "uint8",
          "minimum": 0,
          "maximum": 255,
          "description": "The exit status the error sets."
        },
        "retryable": {
          "type": "boolean",
          "description": "Whether retrying the same command can succeed."
        },
        "meaning": {
          "type": "string"
        }
      },
      "required": [
        "code",
        "exit",
        "retryable",
        "meaning"
      ]
    },
    "ExampleDescription": {
      "type": "object",
      "properties": {
        "summary": {
          "type": "string"
        },
        "command": {
          "type": "string"
        }
      },
      "required": [
        "summary",
        "command"
      ]
    }
  }
}
```

### `docs`

Search and read pkg's documentation.

#### `pkg docs search`

Search pkg's documentation: naming packages, removing them, catalogs, and the CLI reference.

Ranks every section of the Markdown compiled into this binary against the query and returns the best matches: the document `path`, its `title`, the `section` heading, an `excerpt` of the best-matching paragraph, and a `score`. Read a whole document with `pkg docs show <path>`. Runs offline and never changes anything.

- **Effect:** reads only
- **Requires:** nothing beyond `pkg`
- **Dry run:** no
- **Idempotent:** yes
- **Errors:** `INVALID_ARGUMENT`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `<query>` | string, repeatable | yes | — | The topic, in plain words; several words need no quotes |
| `--limit <integer>` | integer, 1–20 | no | `5` | The most matches to return |
| `--cursor <string>` | string | no | — | Resume after a previous page: the nextCursor it returned |

Examples:

```sh
# Learn why remove can refuse
pkg docs search 'undeclared packages prune'
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "items": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/SectionMatch"
      }
    },
    "hasMore": {
      "type": "boolean",
      "description": "Whether more items follow this page."
    },
    "nextCursor": {
      "type": [
        "string",
        "null"
      ],
      "description": "Pass as `--cursor` to get the next page; null when there is none."
    }
  },
  "required": [
    "items",
    "hasMore"
  ],
  "description": "One page of a collection.",
  "title": "Page",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "SectionMatch": {
      "type": "object",
      "properties": {
        "path": {
          "type": "string",
          "description": "The document, for `pkg docs show <path>`."
        },
        "title": {
          "type": "string",
          "description": "The document's title."
        },
        "section": {
          "type": "string",
          "description": "The heading of the matching section."
        },
        "excerpt": {
          "type": "string",
          "description": "The paragraph of the section that best matches the query, shortened to 400 characters."
        },
        "score": {
          "type": "number",
          "format": "double",
          "description": "Relevance from 0 to 1."
        }
      },
      "required": [
        "path",
        "title",
        "section",
        "excerpt",
        "score"
      ],
      "description": "One section that matches the query."
    }
  }
}
```

#### `pkg docs show`

Print one documentation file in full.

Returns the Markdown of one document compiled into this binary, by the `path` that `pkg docs search` reports, such as `docs/packages.md`. `--output text` prints the Markdown alone. Never changes anything.

- **Effect:** reads only
- **Requires:** nothing beyond `pkg`
- **Dry run:** no
- **Idempotent:** yes
- **Errors:** `NOT_FOUND`, `INTERNAL`

| Argument | Type | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `<path>` | string | yes | — | The document, as `docs search` names it: `docs/packages.md` |

Examples:

```sh
# Read how pkg names and records packages
pkg docs show docs/packages.md
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string"
    },
    "title": {
      "type": "string"
    },
    "markdown": {
      "type": "string",
      "description": "The document's Markdown."
    }
  },
  "required": [
    "path",
    "title",
    "markdown"
  ],
  "description": "A whole document.",
  "title": "DocumentContent",
  "$schema": "https://json-schema.org/draft/2020-12/schema"
}
```

### `context`

Show the mise config and catalogs pkg uses.

#### `pkg context show`

Show the mise config files, mise, and catalogs pkg uses.

Reports where pkg takes effect: the mise executable (`PKG_MISE`, else `mise` on PATH, else `~/.local/bin/mise`), its version, and whether `pkg update` updates it (not when a package manager installed it), the home directory pkg runs mise in, the file new `[bootstrap.packages]` entries go to (mise's `write_targets.packages` setting, else its global config) and the file new `[tools]` entries go to (`write_targets.tools`), each with symlinks resolved to the checkout it lands in, every global config file mise loads, the Homebrew and App Store catalog URLs, and the cached Homebrew search index. Without mise it says so and reports the global config. Never changes anything.

- **Effect:** reads only
- **Requires:** nothing beyond `pkg`
- **Dry run:** no
- **Idempotent:** yes
- **Errors:** `INVALID_ARGUMENT`, `PRECONDITION_FAILED`, `MISE_FAILED`, `INTERNAL`

Examples:

```sh
# Show where installs are recorded
pkg context show
```

Result schema:

```json
{
  "type": "object",
  "properties": {
    "version": {
      "type": "string",
      "description": "The pkg version answering."
    },
    "pkg": {
      "$ref": "#/$defs/PkgContext"
    },
    "mise": {
      "$ref": "#/$defs/MiseContext"
    },
    "home": {
      "type": "string",
      "description": "The home directory pkg runs mise in."
    },
    "packagesConfig": {
      "$ref": "#/$defs/ConfigContext",
      "description": "Where `pkg install` declares bootstrap packages."
    },
    "toolsConfig": {
      "$ref": "#/$defs/ConfigContext",
      "description": "Where `pkg install` declares tools."
    },
    "configFiles": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Every global config file mise loads, symlinks resolved."
    },
    "catalogs": {
      "$ref": "#/$defs/Catalogs"
    }
  },
  "required": [
    "version",
    "pkg",
    "mise",
    "home",
    "packagesConfig",
    "toolsConfig",
    "configFiles",
    "catalogs"
  ],
  "description": "Where pkg takes effect.",
  "title": "Context",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "PkgContext": {
      "type": "object",
      "properties": {
        "executable": {
          "type": "string",
          "description": "The executable answering."
        },
        "owner": {
          "$ref": "#/$defs/PkgOwner",
          "description": "Who put it there: `installer` (rerun the installer to update it), `mise` (`pkg update`\nupdates it), or `other`."
        },
        "tool": {
          "anyOf": [
            {
              "$ref": "#/$defs/PkgTool"
            },
            {
              "type": "null"
            }
          ],
          "description": "pkg as a mise tool, when a config declares `github:gotpkg/pkg`."
        }
      },
      "required": [
        "executable",
        "owner"
      ]
    },
    "PkgOwner": {
      "oneOf": [
        {
          "type": "string",
          "const": "installer",
          "description": "The installer's copy in `~/.local/bin`, which rerunning the installer updates."
        },
        {
          "type": "string",
          "const": "mise",
          "description": "mise, which `pkg update` has update it."
        },
        {
          "type": "string",
          "const": "other",
          "description": "Something else, such as Homebrew or a build from a checkout."
        }
      ],
      "description": "Who put a pkg executable where it is."
    },
    "PkgTool": {
      "type": "object",
      "properties": {
        "requested": {
          "type": "string",
          "description": "The version the config asks for, such as `latest`."
        },
        "installed": {
          "type": [
            "string",
            "null"
          ],
          "description": "The version mise has installed; null when none is."
        },
        "config": {
          "type": [
            "string",
            "null"
          ],
          "description": "The config file that declares it."
        }
      },
      "required": [
        "requested"
      ]
    },
    "MiseContext": {
      "type": "object",
      "properties": {
        "program": {
          "type": "string",
          "description": "The executable pkg runs."
        },
        "environmentVariable": {
          "type": "string",
          "description": "The environment variable that overrides it."
        },
        "found": {
          "type": "boolean",
          "description": "Whether that executable exists and runs."
        },
        "version": {
          "type": [
            "string",
            "null"
          ],
          "description": "What `mise version` prints; null when mise is not found."
        },
        "selfUpdates": {
          "type": "boolean",
          "description": "Whether `pkg update` updates mise with `mise self-update`; false for a mise a package\nmanager installed."
        }
      },
      "required": [
        "program",
        "environmentVariable",
        "found",
        "selfUpdates"
      ]
    },
    "ConfigContext": {
      "type": "object",
      "properties": {
        "setting": {
          "type": [
            "string",
            "null"
          ],
          "description": "mise's `write_targets` setting as written; null when unset, which means the global config."
        },
        "path": {
          "type": "string",
          "description": "The file as mise loads it."
        },
        "realPath": {
          "type": "string",
          "description": "The file with symlinks resolved: the checkout new declarations land in."
        }
      },
      "required": [
        "path",
        "realPath"
      ]
    },
    "Catalogs": {
      "type": "object",
      "properties": {
        "homebrewApi": {
          "type": "string"
        },
        "homebrewApiVariable": {
          "type": "string"
        },
        "appStoreApi": {
          "type": "string"
        },
        "appStoreApiVariable": {
          "type": "string"
        },
        "homebrewIndex": {
          "type": "string",
          "description": "The cached Homebrew search index."
        },
        "homebrewIndexFetchedAt": {
          "type": [
            "integer",
            "null"
          ],
          "format": "uint64",
          "minimum": 0,
          "description": "When the cached index was downloaded, in seconds since the Unix epoch; null when there is\nnone yet."
        }
      },
      "required": [
        "homebrewApi",
        "homebrewApiVariable",
        "appStoreApi",
        "appStoreApiVariable",
        "homebrewIndex"
      ]
    }
  }
}
```
