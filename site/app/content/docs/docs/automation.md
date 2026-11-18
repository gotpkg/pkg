---
title: Scripts and agents
description: JSON output, stable exit statuses, paging, the discovery commands built into pkg, and the pkg-core Rust library.
sidebar:
    order: 5
---

Scripts and agents run the same commands people do. They add `--output json`, read results from stdout and errors from stderr, and branch on stable error codes. No command prompts: missing input fails at once with `INVALID_ARGUMENT`.

## JSON output

`--output json` works on every command. A result is one JSON document on stdout, compact when piped and indented on a terminal. mise's progress and pkg's warnings go to stderr in both formats, so stdout stays parseable.

A failure writes nothing to stdout. It writes one JSON document to stderr, an `error` object with `code`, `message`, `retryable`, `details`, and `suggestions`, and exits with the status of its code:

| Code                  | Exit | Retryable | Meaning                                                                                                                      |
| --------------------- | ---- | --------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `INVALID_ARGUMENT`    | 2    | no        | Input is missing or malformed.                                                                                               |
| `NOT_FOUND`           | 3    | no        | A named package, tap, command, or document does not exist, or is not declared.                                               |
| `PRECONDITION_FAILED` | 3    | no        | mise is missing, the package is declared in a file pkg does not edit, or a removal would also uninstall undeclared packages. |
| `UNAVAILABLE`         | 5    | yes       | The Homebrew or App Store catalog could not be reached; retry the same command.                                              |
| `MISE_FAILED`         | 6    | no        | mise reported a failure; `details.output` holds the end of its output.                                                       |
| `INTERNAL`            | 70   | no        | A defect in pkg, or a stdout that cannot take the result.                                                                    |

Branch on the exit status or on `code`, never on `message`. `NOT_FOUND` and `PRECONDITION_FAILED` share exit status 3, and only `code` tells them apart. `suggestions` lists commands or changes that recover from the failure, most useful first. Warnings and mise's progress also go to stderr, so stderr can hold more than the error document.

```sh
pkg search ffmpeg --output json > matches.json
case $? in
  0) jq -r '.items[].package' matches.json ;;
  5) echo "a catalog was unreachable; run the same search again" >&2 ;;
esac
```

## Receipts and dry runs

Every command that changes something accepts `--dry-run`, which makes the same checks and decisions as the real run and changes nothing. Its receipt carries `status` set to `dry-run`; a real run's receipt says `completed`. Running the dry run first and reading its receipt is the safe way to let an agent change a machine.

## Paging collections

`list`, `search`, `cli search`, and `docs search` return collections with `items`, `hasMore`, and `nextCursor`. Pass `nextCursor` back as `--cursor` until `hasMore` is false:

```sh
pkg list --limit 20 --output json
pkg list --limit 20 --cursor <nextCursor> --output json
```

`pkg list` returns every entry when you omit `--limit`.

## Discovery built into the binary

pkg describes itself without a network connection or a checkout of the repository. `pkg --help` lists the commands for people; these commands are for scripts and agents:

```sh
pkg cli search 'install a cask'               # which command does this task
pkg schema                                    # every command, with its summary and effect
pkg schema tap add                            # one command's arguments, errors, examples, and result schema
pkg docs search 'undeclared packages prune'   # search the documentation compiled into pkg
pkg docs show docs/packages.md                # read one of those documents in full
pkg context show                              # the mise, config files, and catalogs in effect
```

The reliable loop is: find a command with `pkg cli search`, inspect it with `pkg schema`, preview it with `--dry-run`, run it, and verify the result with `pkg list` or `pkg info <package>`.

## Use pkg from Rust

The `pkg-core` crate runs the same package operations as typed, blocking calls, without starting the CLI or parsing its output. It is not published to crates.io; add it as a Git dependency:

```toml
[dependencies]
pkg-core = { git = "https://github.com/gotpkg/pkg.git" }
```

mise 2026.11.9 or newer still has to be installed separately. The [Rust library guide](https://github.com/gotpkg/pkg/blob/main/guides/rust-library.md) has a worked example and the limits, and the [API contract](https://github.com/gotpkg/pkg/blob/main/crates/pkg-core/src/lib.rs) defines isolation, progress, errors, and mutation safety.

The [generated CLI reference](https://github.com/gotpkg/pkg/blob/main/crates/pkg/docs/cli.md) owns the JSON contract on this page, including every command's result schema.
