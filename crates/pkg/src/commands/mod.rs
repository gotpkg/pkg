//! Every resource's commands. Each module declares its registry entries in `COMMANDS`.

use crate::registry::CommandSpec;

pub mod bootstrap;
pub mod cli;
pub mod common;
pub mod context;
pub mod docs;
pub mod info;
pub mod install;
pub mod list;
pub mod locate;
pub mod outdated;
pub mod prune;
pub mod remove;
pub mod schema;
pub mod search;
pub mod setup;
pub mod shims;
pub mod sync;
pub mod tap;
pub mod update;

/// The registry, by resource, in the order discovery lists them.
pub static GROUPS: &[&[CommandSpec]] = &[
    install::COMMANDS,
    remove::COMMANDS,
    list::COMMANDS,
    search::COMMANDS,
    info::COMMANDS,
    outdated::COMMANDS,
    update::COMMANDS,
    sync::COMMANDS,
    prune::COMMANDS,
    locate::COMMANDS,
    tap::COMMANDS,
    setup::COMMANDS,
    bootstrap::COMMANDS,
    shims::COMMANDS,
    cli::COMMANDS,
    schema::COMMANDS,
    docs::COMMANDS,
    context::COMMANDS,
];
