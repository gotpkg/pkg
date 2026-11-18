---
name: designing-human-first-clis
description: Use when designing, implementing, or reviewing pkg's commands, help, output, or errors, or any command-line tool whose main user is a person at a terminal but which agents and scripts must also drive reliably.
---

# Designing Human-First CLIs

A person typing at a terminal is the main user. They should get from intent to result with one memorable command, read the outcome at a glance, and recover from a mistake from the error alone. Agents and scripts use the same commands with `--output json` and a discovery surface that stays out of the person's way.

## One contract, two renderings

Declare every command once, in the registry: arguments and defaults, constraints, effects, requirements, dry-run and idempotency, errors, examples, synonyms, and the result schema. The parser, `--help`, `schema`, `cli search` and the reference are generated from it, so the text a person reads and the contract an agent reads cannot drift. Never hand-write a Clap argument.

Every result is one value with two renderings through the CLI's `RenderOutput`: text for people (the default) and the JSON its schema describes (`--output json`). Both come from the same value, so they never disagree about what happened. Core keeps native paths; the CLI formats them for text or JSON.

## For people (the default)

- **Commands are verbs a person already knows.** `install`, `remove`, `list`, `search`, `info`, `outdated`, `upgrade`. Give familiar aliases (`rm`, `uninstall`, `ls`, `i`) rather than inventing new words. Keep arguments positional when there is one obvious thing to act on.
- **Accept the names people see elsewhere.** A package is written the way mise and Homebrew documentation write it (`brew:wget`, `brew-cask:iina`); never invent an identifier format.
- **`--help` is a short orientation:** what the tool does, how to name things, the everyday commands with one-line summaries, and a few examples. It is not the catalogue: agent commands are hidden from the list and named in one closing line.
- **Text says what happened in a sentence,** names the file or package it touched, and abbreviates the home directory as `~`. Tables align their columns. Say "Dry run: nothing was ..." at the end of every dry run.
- **Errors are a sentence and hints:** `error: <what went wrong>` then `hint: <command that fixes it>` lines. When a tool pkg runs failed, its own output has already gone to stderr; do not print it twice.
- **Progress from long operations goes to stderr as it happens,** so a person sees mise working and stdout keeps only the result.
- **No prompts.** Every command does exactly what was typed; a person previews with `--dry-run`. A command that would do more than was asked (a prune that would also take undeclared packages) refuses and says how to proceed instead of asking.

## For scripts and agents (opt-in)

`--output json` switches both streams: one JSON result on stdout, and on failure one `{"error": {code, message, retryable, details, suggestions}}` document as the last line of stderr, with stable exit statuses. Branch on `code`, never the message.

The discovery surface exists in full, but outside the person's `--help` list:

| Command                     | Result                                                   |
| --------------------------- | -------------------------------------------------------- |
| `pkg cli search "<intent>"` | Ranked commands with their exact command line and effect |
| `pkg schema [<command>]`    | Every command, or one command's full contract            |
| `pkg docs search "<topic>"` | Sections of the documentation compiled into the binary   |
| `pkg docs show <path>`      | One document in full                                     |
| `pkg context show`          | Where commands take effect: mise, config files, catalogs |
| `pkg --version`             | The installed version                                    |

Keep all six, even when people never use them. Discovery reads only local state and never needs credentials. Collections expose `items`, `hasMore` and `nextCursor`; never present a bounded page as the whole collection.

## Mutation safety

- Every modifying command has `--dry-run`, which makes the same checks and decisions as the real run and changes nothing: no install, no uninstall, no config write. It states what it cannot predict.
- Receipts say what actually happened (`added`, `installed`, `unchanged`, `declared`; `uninstalled`, `leftInstalled`), not what was requested. Verify with an independent read (`list`, `info`), never with the exit status alone.
- Undo partial work you started: a failed install takes its own new declaration back out of the config, and the error lists what completed before it.
- Edit only what you own: pkg writes to mise's write target and refuses to touch a declaration in another file, such as a config shared with other machines.

## Verification

Exercise each change three ways: the text a person sees, the JSON an agent parses (it must match the schema), and the failure the change affects, in both renderings. Every registered example runs against the fake mise in `tests/cli/`. Test dry runs by snapshotting the home directory before and after.

Common failures: JSON as the default; a `--help` that dumps every command and flag; errors that are only JSON; a prompt standing in for `--dry-run`; a receipt that echoes the request instead of the outcome; a mutation that silently does more than was asked.
