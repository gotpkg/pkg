//! Getting a new machine ready: mise, `PATH`, and pkg itself.

pub(crate) mod handoff;
pub(crate) mod mise;
pub(crate) mod setup;
pub(crate) mod shims;

use crate::Error;

/// `setup` and `bootstrap` support macOS and Linux only.
fn supported_platform(command: &str) -> Result<(), Error> {
    if cfg!(windows) {
        return Err(Error::precondition(format!("pkg {command} supports macOS and Linux only")));
    }
    Ok(())
}
