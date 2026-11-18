use crate::{Error, Settings};

/// A reusable execution context, not a cache of the machine's installed packages.
#[derive(Debug)]
pub struct Client {
    pub(crate) settings: Settings,
}

impl Client {
    /// Validates locations without running mise or reading package state.
    ///
    /// # Errors
    /// Returns `InvalidArgument` for relative paths, an empty executable, or non-HTTP catalogs.
    pub fn new(settings: Settings) -> Result<Self, Error> {
        settings.validate()?;
        Ok(Self { settings })
    }
}
