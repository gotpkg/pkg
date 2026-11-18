# pkg

This project follows the Workbench process. Workbench loads from `.workbench`, a gitignored link that `mise install` restores: when `.workbench` is missing or dangling, the harness loads no Workbench content and tasks that call `workbench` fail, so run `mise install`.

This file holds only what is true of this project. The process itself comes from the Workbench package.

## Purpose and layout

`pkg` is a human-first command-line tool in Rust for installing and managing software with mise's bootstrap packages and tools. `pkg install brew:wget` installs a package and records it in the global mise config, so `mise bootstrap` restores it on a new machine. Mark is the main user; agents and scripts drive the same commands with `--output json` and the discovery commands (`cli search`, `schema`, `docs`, `context show`), which `--help` keeps out of the person's list.

- `crates/pkg/`: the shipped CLI. `src/registry.rs` declares the command contract; `src/commands/` holds each command's registry entry, typed library adapter, and text renderer; `src/environment.rs` captures process defaults; `docs/` is the Markdown compiled into the binary for `docs search` (`docs/cli.md` is generated); `tests/cli/` runs the real binary against a fake mise and fake catalogs.
- `crates/pkg-core/`: the reusable, unpublished Rust library. `src/lib.rs` owns the API contract; operation modules implement `Client` methods with typed requests/outcomes/errors; `src/mise/`, `src/declarations.rs`, and `src/catalog/` own process/config/network effects. `Settings` supplies all locations and the child environment. Core reports progress through the caller's callback and never writes to the terminal. `guides/rust-library.md` is the consumer guide.
- `xtask/`: the unpublished release helper, `cargo xtask package` and `cargo xtask formula`.
- `.agents/skills/`: `designing-human-first-clis` (the CLI contract: text for people by default, JSON and discovery for agents) and `writing-rust` (this repository's Rust conventions). Read both before changing commands.

## Commands

- `mise run dev -- <arguments>` runs the CLI from source, such as `mise run dev -- search ffmpeg`. Commands that change something run the real mise against your real config, so use `--dry-run` while developing.
- `mise run cli:reference` regenerates `crates/pkg/docs/cli.md` after a registry change; the tests fail until it matches.
- `mise run build`, `test`, `lint`, `doc`: Cargo over the workspace.
- `mise run package [-- <target>]` writes `artifacts/pkg-<target>.tar.gz` (`.zip` on Windows), this machine's target by default.
- `mise run release:formula` writes the Homebrew formula for the archives in `artifacts/`.
- `mise run release` publishes the GitHub release through Knope; only release.yml runs it.

## Platform rules and preview

- One registry entry per command generates the parser, `--help`, `schema`, `cli search` and the reference. Never add a Clap argument, a default or an output field outside it.
- Text for people is the default; `--output json` gives one JSON result on stdout and one structured error on stderr with a stable exit status. mise's progress and warnings go to stderr. No prompts; every modifying command has `--dry-run`, which installs, uninstalls and writes nothing.
- pkg runs every mise command as `mise -C <home>`, writes new declarations only through mise (`write_targets.packages` and `write_targets.tools` decide the file), and edits a config file itself only to remove an entry from mise's write target.
- A reviewer exercises a change with the built binary: first against the test fixtures (`mise run test`), then on the real machine with reads (`list`, `info`, `search`, `outdated`, `context show`) and `--dry-run` for every change, in both text and `--output json`, including the failure the change affects. Run a real install or removal only when the change needs it, and say so.
