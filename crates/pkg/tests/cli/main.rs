//! Process-level tests: each one runs the real `pkg` binary.
//!
//! One test binary keeps link time down and lets every module share `support`. The fake mise is
//! a shell script, so tests that run it are Unix-only; on Windows the discovery and registry
//! tests that never reach mise still run.

#[cfg(unix)]
mod bootstrap;
mod discovery;
#[cfg(unix)]
mod install;
#[cfg(unix)]
mod queries;
mod reference;
mod registry;
#[cfg(unix)]
mod remove;
#[cfg(unix)]
mod setup;
#[cfg(unix)]
mod shims;
mod support;
#[cfg(unix)]
mod terminal;
#[cfg(unix)]
mod upkeep;
