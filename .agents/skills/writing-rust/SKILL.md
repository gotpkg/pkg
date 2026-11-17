---
name: writing-rust
description: Use when writing, reviewing, or restructuring Rust in this repository — adding a command or an argument, running mise or reading its output, editing mise config files, silencing a Clippy lint, choosing an error code, deciding what goes to stdout versus stderr, handling paths that differ on Windows, or choosing between a unit test and a test that runs the binary.
---

# Writing Rust

**Write Rust that makes the correct behavior easy to express, the incorrect behavior difficult to express, and the implemented behavior easy to verify.**

`references/rust-style-guide.md` is the full house guide: API design, ownership, errors and panics, traits, concurrency, unsafe, documentation, testing, performance and dependencies. This file is only what is specific to this repository. For the command-line contract itself, read the `designing-human-first-clis` skill.

## Layout

| Thing                                  | Where                                                                              |
| -------------------------------------- | ---------------------------------------------------------------------------------- |
| Parse, dispatch, report on the streams | `crates/pkg/src/app.rs`                                                            |
| Every command's declaration            | `COMMANDS` in `crates/pkg/src/commands/<command>.rs`, listed in `commands::GROUPS` |
| Registry types, parser, reference      | `registry.rs`, `parser.rs`, `reference.rs`                                         |
| Package names and routing              | `crates/pkg/src/package.rs`                                                        |
| Running mise and reading its answers   | `crates/pkg/src/mise/` (`queries.rs` for JSON, `plans.rs` for its text plans)      |
| Reading and editing mise config files  | `crates/pkg/src/declarations.rs`                                                   |
| Homebrew and App Store catalogs        | `crates/pkg/src/catalog/`                                                          |
| Where mise, catalogs and the cache are | `crates/pkg/src/environment.rs`                                                    |
| Shipped documentation                  | `crates/pkg/docs/*.md`, listed in `docs::DOCUMENTS`                                |
| Rules about values                     | `#[cfg(test)] mod tests` beside the code                                           |
| Rules about the process                | `crates/pkg/tests/cli/` (one test binary, a fake mise, fake catalogs)              |
| Release archives and Homebrew formula  | `xtask/` (`cargo xtask …`), never shipped                                          |

The binary is a shell around `pkg::app::run`; that is what lets the registry and parser be tested in-process.

## Adding or changing a command

1. Declare it in its module's `COMMANDS`: path, aliases, summary, description, arguments, effect, requirements, dry-run, idempotency, errors, examples, synonyms and output schema. A new top-level command or group also goes in `registry::RESOURCES` with its audience: `People` commands appear in `--help`, `Agents` commands are hidden there and named in its closing line.
2. Implement `run` with the shared pieces: `Invocation` for arguments, `Machine` (in `commands/common.rs`) for mise and the config, `Failure` for errors, `Response` for results, `RenderText` for the text people see.
3. Run `mise run cli:reference` to regenerate `crates/pkg/docs/cli.md`.
4. `mise run test`. The registry tests fail when a parser argument is undeclared, a declared default differs at run time, an example fails or its output does not match the schema, or a modifying command lacks `--dry-run` and a dry-run example.

Never hand-write Clap definitions: the parser is generated from the registry with Clap's builder API, so a flag cannot exist without its schema, help and search entry.

## Output and errors

- Results go to stdout as text by default and as JSON with `--output json`; both render the same value. Warnings go to stderr as `warning: …` lines, and so does mise's progress, which `Mise::run` passes through as it arrives.
- A failure returns `Failure` with a stable `ErrorCode`; `app::report` writes `error: …` and `hint: …` lines to stderr, or `{"error": {…}}` with `--output json`, and exits with the code's status. Add a code only when callers must branch on it, and update `ErrorCode::ALL`, its exit status and meaning.
- A closed stdout (`… | head`) is the reader's choice, not a failure; any other stdout write error is reported as `INTERNAL`.
- No `process::exit` below `main`; `main` returns `ExitCode`.

## mise and the config files

- Run mise only through `Mise` (`read`/`query` for queries whose output nobody sees, `run` for changes, which streams). It always passes `-C <home>`, so a project's mise.toml never decides what pkg installs.
- Prefer mise's JSON (`--json`) to its text. Where mise has none (prune and upgrade plans), parse only the lines you need in `mise/plans.rs` and skip the rest, with a unit test over captured output.
- New declarations are mise's to write (`mise bootstrap packages use -g`, `mise use -g`). pkg edits a file itself only to take a `[bootstrap.packages]` entry out, through `declarations::remove_package`, which deletes the entry's own lines, keeps comments, and checks the result parses to the original minus that entry. Edit only the write target; refuse a declaration in any other file.
- Reads and dry runs never install, uninstall or write a config file. The only thing a read may write is the search index cache.

## Lints

`[workspace.lints]` in the root `Cargo.toml`:

| Lint                                                  | Table    | Why it is on                                                          |
| ----------------------------------------------------- | -------- | --------------------------------------------------------------------- |
| `unreachable_pub`                                     | `rust`   | a `pub` item no public path reaches is dead or a missing `pub(crate)` |
| `undocumented_unsafe_blocks`                          | `clippy` | every `unsafe` block carries its `SAFETY:` comment                    |
| `allow_attributes_without_reason`, `allow_attributes` | `clippy` | silencing a lint is a decision with a reason, made with `#[expect]`   |
| `cast_possible_truncation`, `cast_sign_loss`          | `clippy` | casts that silently lose data; use `try_from`                         |
| `missing_errors_doc`, `missing_panics_doc`            | `clippy` | a public fallible or panicking function says when it does either      |

Silence a lint on the item, with a reason, using `#[expect]`, never `#[allow]`. `#[expect]` fails the build once the underlying problem is gone.

## Formatting

`rustfmt.toml`: 100 columns, 4 spaces, Unix newlines, `use_small_heuristics = "Max"`. Write std, then external crates, then `crate` imports as separate groups; stable rustfmt sorts within groups but cannot create them.

## Cross-platform

CI builds and tests on Linux, macOS and Windows.

- Print paths with `Path::display()`; build them with `Path::join`.
- Compare paths in tests by canonical directory: macOS reports `/var` as `/private/var`, and Windows canonical paths carry a `\\?\` prefix.
- In tests use `env!("CARGO_BIN_EXE_pkg")`, which already has Windows' `.exe`.

## Testing

Unit tests cover rules: package routing, config edits, plan parsing, ranking, rendering, paging. Process tests in `tests/cli/` cover what needs a process: both renderings, nothing on stdout for failures, exit statuses, the exact mise commands pkg ran (`Fixture::calls`), and that dry runs and refusals leave the home directory byte-for-byte unchanged (`Fixture::snapshot`). Every fixture runs pkg with `HOME`, `PKG_MISE`, `PKG_HOMEBREW_API`, `PKG_APP_STORE_API` and `XDG_CACHE_HOME` pointed into a temporary directory, with a fake mise that answers from files and a local catalog server, so no test runs the real mise, reaches the network, or touches real config. The fake mise is a shell script, so those tests are Unix-only.

## Verification

```sh
mise run fmt        # dprint and rustfmt, in place
mise run lint       # cargo clippy --workspace --all-targets -- -D warnings
mise run test       # cargo test --workspace
mise run doc        # rustdoc with RUSTDOCFLAGS=-D warnings
mise run check      # the gate: test, doc, every hk step, Workbench validation
```

Never substitute a weaker command for one of these, and never report a check as passing unless it ran.

## Anti-patterns — stop if you are about to

- Add `.clone()`, `Arc`, `Mutex` or `Box` to make a borrow error go away instead of deciding the ownership.
- Add `#[allow(...)]`, or widen a lint, to make a warning go away.
- `unwrap()` or `expect()` on anything a user, mise, a catalog or the file system supplied.
- Swallow an error with `.ok()`, a default value or an ignored result.
- Install, uninstall or write a config file from a read or a `--dry-run` path, or run the real mise in a test.
- Document a flag, default or output field anywhere but the registry and the types it derives schemas from.
- Write a test that restates the implementation, or that asserts a constant against itself.
- Claim correctness because it compiled.
