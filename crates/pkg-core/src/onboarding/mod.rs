//! Getting a new machine ready: mise, `PATH`, and pkg itself.

pub(crate) mod bootstrap;
pub(crate) mod handoff;
pub(crate) mod mise;
pub(crate) mod preview;
pub(crate) mod setup;
pub(crate) mod shims;
pub(crate) mod startup_files;

use crate::Error;

/// `setup` and `bootstrap` support macOS and Linux only.
fn supported_platform(command: &str) -> Result<(), Error> {
    if cfg!(windows) {
        return Err(Error::precondition(format!("pkg {command} supports macOS and Linux only")));
    }
    Ok(())
}
