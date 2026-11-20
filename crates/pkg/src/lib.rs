//! pkg: install, remove, and find packages and tools with mise, recording each one in the mise
//! config so a new machine gets it back.
//!
//! The binary is a shell around [`app::run`]. Every command is declared once in the
//! [`registry`]; the parser, `--help`, `schema`, `cli search`, and the CLI reference are generated
//! from it.

pub mod app;
pub mod commands;
mod core_failure;
pub mod docs;
mod environment;
mod error_details;
pub mod failure;
pub mod invocation;
pub mod parser;
mod progress;
pub mod reference;
pub mod registry;
pub mod response;

/// The executable's name, as commands and suggestions spell it.
pub const NAME: &str = env!("CARGO_PKG_NAME");
