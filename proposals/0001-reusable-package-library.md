---
id: proposal.0001
title: Reusable Package Library
authors: [Mark]
status: implemented
pull-request: https://github.com/markmals/pkg/pull/1
issues: []
supersedes: []
---

# Reusable Package Library

## Summary

Extract pkg's package-management functionality into a typed Rust library, `pkg-core`, consumed by the existing `pkg` CLI. Separate operations from argument parsing, terminal output, and discovery. Make future Swift or GTK frontends possible without invoking the CLI, adding bindings, an app, or a new package-management engine now.

## Motivation

A future native package-management app needs the same operations, safety checks, receipts, and failures as the CLI. Calling the executable and decoding its output would tie the app to process orchestration and command syntax. Copying the operation logic would create two implementations of package routing, declaration ownership, failure cleanup, and mise integration.

The repository already contains a Rust library target, but it is a library of CLI implementation modules rather than a deliberate application API. Programmatic consumers should pass typed requests and receive typed results, without constructing command-line invocations, managing stdout, or changing process-global environment variables.

## Domain grounding

### Established context

- Rust distinguishes a Cargo package from its library and executable crates; [a package may contain both](https://doc.rust-lang.org/book/ch07-01-packages-and-crates.html). Having `lib.rs` alone does not establish a frontend-independent API.
- [UniFFI](https://mozilla.github.io/uniffi-rs/latest/) generates foreign-language bindings for Rust libraries and supports Swift. It does not ship or package the native application. A later adapter can translate a Rust API into its supported boundary types; today's core need not adopt UniFFI macros or foreign-language representations.
- Rust's [API guidelines](https://rust-lang.github.io/api-guidelines/future-proofing.html) distinguish a stable public contract from implementation details. Exposing internal process runners and mutable state creates commitments unrelated to the caller's actual package-management task.
- A GTK app written in Rust can consume the Rust library directly. A Swift app needs a foreign-function boundary and packaging; neither follows automatically from extracting a crate.

### Relevant constraints and principles

A package identifier, a desired declaration, an observed installation, an operation request, and its receipt are different concepts. Preserve the existing distinction between bootstrap packages and mise tools, including package pins and Homebrew versioned names. Do not add a new universal package identity or persistent database.

A CLI invocation is one representation of an operation request, not the operation itself. A progress message is an observation, not proof that a package was installed. A successful subprocess exit is not a substitute for the existing independently read installation state. Keep these distinctions at the public boundary.

### Quality bar

An external Rust consumer can perform every current package operation with typed requests and results, using the same implementation as the CLI. Calling it does not print, exit the process, install signal handlers, or mutate process-global settings. Existing CLI behavior remains compatible. No public internal machinery or speculative framework is introduced merely to prepare for a possible app.

### Remaining uncertainty

No design question blocks this draft's selected two-crate direction. Foreign-language bindings, app sandbox/distribution constraints, and app-specific cancellation or scheduling require their own future design. This proposal does not promise that the extracted core is already a drop-in Swift SDK.

## Existing baseline

[The workspace](../Cargo.toml) contains the shipped `crates/pkg` package and the unpublished `xtask` helper. [The pkg manifest](../crates/pkg/Cargo.toml) defines both `pkg` library and binary targets and marks the package unpublished. [The current library](../crates/pkg/src/lib.rs) exposes application, parser, registry, command, catalog, mise, and configuration modules.

[Machine loading](../crates/pkg/src/commands/common.rs) accepts a CLI `Invocation`; operations return [Response](../crates/pkg/src/response.rs), which eagerly holds JSON and rendered text. [Environment](../crates/pkg/src/environment.rs) resolves process settings, while [the mise runner at the baseline revision](https://github.com/markmals/pkg/blob/ce9e7dac8b8afffc6cc9480d8d41798b18d54d63/crates/pkg/src/mise/mod.rs) prints progress directly to stderr. [Failure](../crates/pkg/src/failure.rs) combines operation facts with CLI error serialization and exit statuses.

Preserve the existing operation behavior and the real-binary fixture tests. Those are valuable evidence of package semantics and CLI compatibility. The broad public module surface and presentation coupling are implementation compromises, not APIs to preserve indefinitely. There is no published Rust library release to migrate at this baseline; any external Git/path consumers of the old implementation surface would need a source migration.

## Proposed solution

Use two production packages, with `xtask` remaining separate:

```text
crates/pkg-core/  typed package operations, mise/config/catalog integration
crates/pkg/       executable, registry, parsing, discovery, text/JSON adapters
xtask/            release packaging

pkg CLI ──depends on──> pkg-core
future Rust GTK app ──> pkg-core
future Swift app ──> future UniFFI adapter ──> pkg-core
```

The human selected this split over a crate for every subsystem and over adding UniFFI now. Organize `pkg-core` internally by cohesive functionality; a future need, not a layering diagram, must justify another package.

## Detailed design

### One reusable operation boundary

Create a `pkg-core` package with a `pkg_core` library target. Keep the existing `pkg` package name, executable name, and directory so source installation, `mise run dev`, release archive names, and the Homebrew formula do not change unnecessarily. The CLI may retain an in-process test library for its registry/parser, but that is not the reusable package API and must not re-export the old core implementation paths.

Expose a small client/context API with explicit settings and typed operation methods. Cover every current package-facing command: install, remove, list, search, info, outdated, upgrade, sync, prune, where/location, tap list/add/remove, and machine context inspection. Preserve argument meaning, ordering, pagination, package routing, and operation effects from [the current registry](../crates/pkg/src/registry.rs) and [Packages](../crates/pkg/docs/packages.md). Use operation-specific request and result types, not a string command dispatcher or a generic JSON method.

Mutation requests express apply versus dry run explicitly. Preview and apply use the same operation decisions and existing safety checks; do not introduce a persisted plan, a plan token, or a stale plan that can later be executed without rereading state. Results retain the observed actions and partial-completion information that current CLI receipts express. Warnings remain accessible to consumers even when no terminal is attached.

Keep package routing, declaration ownership/refusal, rollback of newly added declarations, prune safeguards, query interpretation, catalog lookup/ranking, and receipt construction in core. None may remain duplicated in the CLI. CLI handlers translate registry-derived arguments into requests, call core, and adapt the result; they do not issue their own mise commands or edit config files.

### Explicit context, no ambient frontend dependency

Move the operational parts of [environment resolution](../crates/pkg/src/environment.rs), [Machine](../crates/pkg/src/commands/common.rs), [mise](https://github.com/markmals/pkg/blob/ce9e7dac8b8afffc6cc9480d8d41798b18d54d63/crates/pkg/src/mise/mod.rs), [declarations](https://github.com/markmals/pkg/blob/ce9e7dac8b8afffc6cc9480d8d41798b18d54d63/crates/pkg/src/declarations.rs), [package routing](https://github.com/markmals/pkg/blob/ce9e7dac8b8afffc6cc9480d8d41798b18d54d63/crates/pkg/src/package.rs), and [catalogs](https://github.com/markmals/pkg/tree/ce9e7dac8b8afffc6cc9480d8d41798b18d54d63/crates/pkg/src/catalog/) behind the core boundary. Keep implementation modules private unless a consumer genuinely needs their types.

The context receives home, selected mise executable, global-config location, catalog endpoints, cache location, and child-process environment as explicit settings. A child-environment snapshot can preserve real CLI behavior, including credentials and mise/XDG selection, without changing the host process environment. Keep sensitive environment values out of Debug output and error payloads. Read process defaults once at the CLI edge; do not make every operation reread them or require Rust clients to alter global variables.

Construction validates settings but does not install anything or take a durable snapshot of all installed packages. Load operation-relevant state for each call so a long-lived app context does not silently use declarations cached before another command changed them. Two contexts with different homes and settings must not share mutable global state or a cache root implicitly.

This is not an operating-system sandbox. Core still invokes mise and talks to catalogs, and it inherits only the child environment its caller supplies. Existing reads may write the documented search cache, but do not gain package/configuration mutation. Ordinary mise commands retain home-directory execution and current write-target rules.

### Typed outcomes and recoverable failures

Return ordinary Rust records/enums and `Result`, with owned data only where results must outlive the call. Keep native path types internally and at the Rust boundary where appropriate; JSON path formatting and home abbreviation belong to the CLI. Do not turn every result into JSON and then deserialize it for library callers. Do not allocate both text and JSON representations when only one is requested.

Separate domain failures from process presentation. Core errors implement `std::error::Error` and expose typed recovery-relevant facts: invalid input, missing entities, unmet preconditions, catalog availability failures, and mise failure with its exit status and known partial work. Keep diagnostic output as diagnostic text rather than pretending unstructured upstream output is a stable structured protocol. Preserve retryability where supported by the existing contract.

The CLI maps these failures to the existing [ErrorCode and ErrorDocument contract](../crates/pkg/src/failure.rs), including stable exit statuses and CLI command suggestions. An app must not parse English hints or CLI JSON to learn which package failed, which config is not editable, or which earlier operations completed. CLI parse errors and stdout failures remain CLI-owned.

Domain result types may derive serialization/schema traits where that keeps one source of truth. Otherwise use a narrow CLI adapter for genuine presentation differences; do not hand-maintain a second operational schema. Keep `Response`, `RenderText`, table layout, shell hints, JSON envelopes, and exit-code mapping in the CLI. The local CLI rendering trait can be implemented for core result types without making core depend on terminal presentation.

### Progress and execution lifetime

Keep the current blocking execution model: no mandatory async runtime, background daemon, worker pool, or service protocol. Library consumers choose their worker thread or async blocking adapter. A client is moveable to a worker thread; calls on one mutable client execute sequentially. Applications must serialize mutations of the same real configuration across clients; this extraction does not add cross-process locking or claim transactional safety against concurrent external mise use.

Provide an explicit per-operation progress sink that can receive warnings and raw upstream output with its source stream identified. Never print from core. Deliver callbacks serially on the operation's calling thread; pipe readers may use internal workers, but they must not invoke user callbacks concurrently or after the operation has returned. Drain stdout and stderr without deadlock and finish/reap child processes before returning. Preserve ordering within each stream without inventing a total ordering across independent streams.

Progress is not the receipt. Sink events must not claim an install completed before the independent state observation used by the existing implementation. A consumer can ignore progress without losing warnings, failures, or the final result. Bound the internal handoff buffer rather than retaining an unbounded duplicate event history, while keeping any output needed by current parsers and error diagnostics.

Core owns neither process termination nor signal policy. No `process::exit`, global tracing subscriber, signal handler, process-wide cwd/environment change, or implicit terminal access. The extracted API remains noninteractive by default. Cancellation, long-lived subscriptions, and an app job queue are not invented by this refactor; do not present a blocking call as cancellable or promise that dropping a handle rolls back an install.

### CLI contract and modular migration

[Registry](../crates/pkg/src/registry.rs), [parser](../crates/pkg/src/parser.rs), [application](../crates/pkg/src/app.rs), [reference generation](../crates/pkg/src/reference.rs), and the discovery/documentation commands stay in the CLI. The registry remains the single declaration of CLI arguments, defaults, requirements, effects, examples, and output schemas. Core documents operation semantics, not a competing command registry. Package catalog search belongs in core; searching CLI help and embedded docs does not.

Move typed operational data and logic out of each [command module](../crates/pkg/src/commands/) and migrate all handlers, tests, and release-helper references in the same change. Delete obsolete paths and re-exports rather than maintaining a compatibility shim over the old CLI-shaped library. Exported symbol changes require reference discovery before edits during implementation.

Preserve text semantics, JSON schemas, errors, stdout/stderr routing, default flags, generated reference content, and home/config safety. A necessary change to a public CLI contract is not incidental cleanup: raise it rather than silently include it in this extraction.

### Current sources and new contract homes

- [Cargo workspace](../Cargo.toml) and [pkg manifest](../crates/pkg/Cargo.toml): add `pkg-core`, depend on it from `pkg`, retain the CLI as the default member, and keep the shared toolchain/lints. Do not introduce one crate per adapter.
- [Current library exports](../crates/pkg/src/lib.rs), [commands](../crates/pkg/src/commands/), and operational modules named above: migrate to the new typed boundary and remove dead implementation paths.
- [Registry](../crates/pkg/src/registry.rs), [CLI reference](../crates/pkg/docs/cli.md), and [Packages](../crates/pkg/docs/packages.md): existing observable behavior remains authoritative; reference regeneration must not disguise behavior drift.
- [README layout and development](../README.md#layout): describe the two production packages and how Rust callers consume the library. [mise tasks](../mise.toml) must continue checking the entire workspace; the installed executable and existing archive contents remain the same.
- [Knope configuration](../knope.toml): include the library manifest in the repository's existing release versioning rather than create an independently released component. Keep the library unpublished initially, consistent with `publish = false`; it is consumable through Git/path dependencies. Public registry publication is a separate authorized release decision.
- New current API specifications belong in `pkg-core` rustdoc: settings, operation requests, results, errors, progress lifetime, blocking behavior, mutation serialization requirements, and dry-run semantics. Write them before implementation, not by extracting descriptions from completed code. Add a template-based `guides/` guide for Rust consumers that links these contracts and demonstrates a read, preview, progress handling, and a typed failure.
- Update the project's Rust guidance and layout facts to the new ownership boundaries. Do not add a new architectural policy merely to duplicate this change.

## Compatibility

No intended CLI behavior or configuration migration. Keep the executable, Cargo package, archive naming, five build targets, and global mise/write-target semantics intact. Package state remains owned by mise.

The old Rust implementation-module surface is deliberately replaced, not preserved as aliases. It is unpublished at this baseline; an external Git/path consumer must migrate to `pkg_core`. Document that source break explicitly. The new library starts with an intentional Rust API, not a promise of permanent ABI stability or an FFI-safe representation of every type.

The onboarding change in [PR #2](https://github.com/markmals/pkg/pull/2) is independent. If onboarding lands first, migrate its package/setup/bootstrap operations too. Its explicitly approved interaction policy must cross a typed execution boundary rather than reintroduce terminal access deep in core. If extraction lands first, onboarding builds on it. This proposal does not authorize full bootstrap by itself.

## Implications on adoption

Existing CLI users do nothing. Rust consumers add `pkg-core` as a Git or path dependency and provide an explicit context and progress sink. No new daemon or runtime is required, and no process spawning of `pkg` is needed. mise remains an external runtime dependency; this proposal does not turn it into an embedded Rust dependency or provide installation of mise.

A future Swift wrapper will translate types and own binding generation, packaging, app permissions, background execution, and cancellation behavior. A future Rust GTK app can call core directly. Neither app is shipped here.

## Scope

Extract every existing package operation, introduce the typed caller boundary, migrate all CLI consumers, preserve contracts, and provide rustdoc and a Rust consumer guide. Include isolated direct-library tests and CLI compatibility validation.

### Out of scope

- UniFFI, Swift/GTK app projects, FFI-compatible ABI design, app packaging, or app sandbox support.
- A crate for every internal module, a provider/plugin framework, dependency-injection container, or generic command bus.
- A new package database, alternative installer engine, daemon, async conversion, cancellation protocol, or cross-process mutation lock.
- New CLI behavior, including first-machine onboarding, except adapting already-landed behavior if this proposal is implemented later.
- crates.io publication or independent library release automation.

## Preview

- Repository shape: library plus its CLI consumer.
- Artifact: private local prerelease source archive for `pkg-core` and the installable pkg executable built from the same revision, with exact filesystem paths in the review report. The human selected local delivery after discovery that this repository is public; do not upload previews to its Actions artifacts or publish a registry release.
- Exercise: an external Rust consumer builds without depending on the CLI, performs a typed read and dry-run mutation against a fake mise/isolated home, collects progress without terminal output, and inspects a typed failure. Two distinct contexts prove isolation; a chatty fake child exercises both pipes and callback lifetime. The normal CLI fixture suite proves the same package semantics and JSON/text contracts.
- Real-machine validation: use the built CLI for reads and dry runs only, in both output formats, including a failure. Do not install/remove real software just to demonstrate extraction.
- Cleanup: remove throwaway consumers and fixture homes after evidence is recorded; keep the documented public usage example. A native app mockup would not exercise the library contract and is not a substitute.

## Policies and decisions checked

No policy or decision records exist beyond directory READMEs. [VISION.md](../VISION.md) keeps mise authoritative and requires predictable, human-first CLI behavior; the extraction preserves both. No vision expansion is needed merely to make the same operations reusable. The [Rust conventions](../.agents/skills/writing-rust/SKILL.md) and [CLI design conventions](../.agents/skills/designing-human-first-clis/SKILL.md) require one command registry, explicit failures, isolated tests, and read-only previews; update their module locations without relaxing those contracts.

## Future directions

A separate UniFFI adapter could export a deliberate app-facing subset without imposing binding constraints on the entire Rust API. A Rust GTK frontend could consume core directly. Actual second consumers may justify cancellation, async integration, or independently reusable adapter crates; none is necessary to establish this boundary now.

## Alternatives considered

- Expose the existing `pkg` library as-is: smallest diff, but callers still depend on CLI invocations, output rendering, ambient environment, and terminal progress.
- Spawn pkg and consume JSON: useful for scripts, but requires executable discovery and process orchestration, and cannot provide an in-process Rust API.
- Separate domain, mise, catalog, and operation crates now: finer package boundaries but larger public APIs and more cross-crate coupling before independent consumers exist. The human selected one cohesive core instead.
- Add UniFFI immediately: tests a specific foreign boundary earlier, but expands scope into API lowering and language packaging before a native app's requirements are known. The human selected extraction without bindings.

## Open questions

None. The human approved implementation and accepted the completed change on 2026-11-17, then requested renumbering this proposal from 0002 to 0001 before merging PR #1. The accepted implementation is recorded here before merge. Delivery is merge-only; no release is authorized. The human also authorized deleting PR #1's local/remote branch, worktree, and private preview artifacts after merge.
