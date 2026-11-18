---
id: guide.rust-library
title: Use pkg From Rust
---

# Use pkg From Rust

## What it does

`pkg-core` provides the package operations used by the `pkg` executable without parsing command lines or writing terminal output. Its [API contract](../crates/pkg-core/src/lib.rs) defines context isolation, blocking execution, progress, errors, and mutation safety. The CLI's [package reference](../crates/pkg/docs/packages.md) explains package names and mise configuration ownership.

## How to use it

1. Add `pkg-core` from this repository as a Git dependency, or use `path = "../pkg/crates/pkg-core"` from a neighboring checkout. The crate is not published to crates.io.
2. Install mise 2026.11.9 or newer separately, or let `Client::setup` install it with mise's own installer on macOS or Linux. Create `Settings` with an absolute home and the mise executable. Its configuration/cache defaults derive from that home, not the process environment. Set explicit paths and catalog endpoints when needed; pass only the child environment that the operation should receive.
3. Construct `Client`, pass a progress callback to each operation, and inspect the typed result or error. Run calls on your own worker thread when the host has a UI; calls are blocking.

## Worked example

This example reads the caller-selected home, previews installing wget, and distinguishes an unmet prerequisite without parsing a message. It does not install software or edit configuration. The process-environment snapshot is a frontend choice; a sandboxed consumer can instead supply a smaller map.

```rust,no_run
use std::path::PathBuf;

use pkg_core::{Client, ErrorKind, InstallRequest, ListRequest, Mode, Progress, Settings};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let home_variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let home = PathBuf::from(std::env::var_os(home_variable).ok_or("home is not set")?);
    let mut settings = Settings::new(home, "mise".into());
    settings.child_env = std::env::vars_os().collect();
    let mut client = Client::new(settings)?;
    let mut progress = |event: Progress<'_>| match event {
        Progress::Output { text, .. } => eprint!("{text}"),
        Progress::Warning(message) => eprintln!("warning: {message}"),
    };

    let listed = client.list(ListRequest::default(), &mut progress)?;
    for package in listed.result.items {
        println!("{}: {}", package.package, package.state);
    }

    let request = InstallRequest {
        packages: vec!["brew:wget".into()],
        mode: Mode::DryRun,
    };
    match client.install(request, &mut progress) {
        Ok(outcome) => {
            for package in outcome.result.packages {
                println!("{}: {:?}", package.package, package.action);
            }
        }
        Err(error) if error.kind == ErrorKind::PreconditionFailed => {
            eprintln!("Cannot preview: {error}");
            eprintln!("Recovery: {:?}", error.recovery);
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
```

The application owns every print in this example. A native frontend can instead update its model from the receipt and route progress to its own log. Passing `&mut |_| {}` ignores progress; final outcomes still retain warnings.

## Limits and edge cases

- A context is not an inventory snapshot: later calls reload the configuration and installed state. Two clients can use different homes without changing process-global settings.
- Set `config_dir` to select a different global mise directory with its host/environment overlays. Set `global_config_override` only when deliberately requesting mise's narrower `MISE_GLOBAL_CONFIG_FILE` behavior; it is not equivalent to selecting that file's directory. The default leaves the file override unset.
- Serialize mutations of the same configuration, including calls through separate clients or processes. This library does not add a lock around external mise users.
- Callbacks run serially on the calling thread and stop before the method returns. Return promptly, do not panic, and do not reenter the same client. Stdout and stderr retain their individual ordering, not a shared total order.
- Core errors expose typed details and recovery actions. `details.package`, `details.installed`, `details.removed`, `details.config`, and `details.editable_config` identify relevant partial work and declaration conflicts when present. `removed` lists declaration removals confirmed before a later failure; it is not a rollback guarantee. Do not assume an error means no earlier package changed.
- Reads and dry runs do not install or edit configuration. Catalog search can update its cache and can return warnings alongside matches.
- `setup` and `bootstrap` change the machine beyond packages, on macOS and Linux only: they can run mise's installer, edit pkg's `PATH` block in shell startup files, and replace `~/.local/bin/pkg` with a link to mise's shim. `bootstrap` applies whatever the configuration declares and is not a transaction; a failure's `details.phase`, `details.completed`, and `details.changes_may_remain` say how far it got.
- Core starts no prompt of its own, async runtime, signal handler, daemon, or background job that survives a call. Set `BootstrapRequest::terminal` only when your process owns an interactive terminal: mise and the programs it runs may then ask there for passwords and answers. Without it, mise runs with no controlling terminal, so a step that needs an answer fails. There is no cancellation or rollback-on-drop API.
- The previous unpublished `pkg` implementation modules are removed rather than retained as aliases. Rust consumers migrate to `pkg_core`; CLI users need no migration. Swift bindings and app packaging remain separate work.
