//! pkg: install, remove, and find packages and tools with mise, recording each one in the mise
//! config so a new machine gets it back.
//!
//! The binary is a shell around [`app::run`]. Every command is declared once in the
//! [`registry`]; the parser, `--help`, `schema`, `cli search`, and the CLI reference are generated
//! from it.

pub mod app;
pub mod catalog;
pub mod commands;
pub mod declarations;
pub mod docs;
pub mod environment;
pub mod failure;
pub mod invocation;
pub mod mise;
pub mod package;
pub mod parser;
pub mod reference;
pub mod registry;
pub mod response;
pub mod search;

/// The executable's name, as commands and suggestions spell it.
pub const NAME: &str = env!("CARGO_PKG_NAME");
