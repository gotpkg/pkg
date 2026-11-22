use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::Settings;
use crate::mise::Mise;

pub(crate) struct Environment<'a> {
    pub mise: Mise<'a>,
    /// The home directory, which mise runs in.
    pub home: &'a Path,
    pub global_config: PathBuf,
    pub homebrew_api: &'a str,
    pub app_store_api: &'a str,
    pub cache_dir: &'a Path,
    pub child_env: &'a BTreeMap<OsString, OsString>,
}

impl<'a> Environment<'a> {
    pub(crate) fn new(settings: &'a Settings) -> Self {
        Self {
            mise: Mise::new(settings),
            home: &settings.home,
            global_config: settings.global_config(),
            homebrew_api: &settings.homebrew_api,
            app_store_api: &settings.app_store_api,
            cache_dir: &settings.cache_dir,
            child_env: &settings.child_env,
        }
    }
}

pub(crate) fn real_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
