use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::Error;

pub const DEFAULT_HOMEBREW_API: &str = "https://formulae.brew.sh/api";
pub const DEFAULT_APP_STORE_API: &str = "https://itunes.apple.com";

/// Locations and child environment supplied by the host, never reread from global state.
#[derive(Clone)]
pub struct Settings {
    pub home: PathBuf,
    pub mise: OsString,
    /// Selects mise's global directory, preserving its environment overlays and global settings.
    pub config_dir: PathBuf,
    /// An explicit MISE_GLOBAL_CONFIG_FILE override; None keeps ordinary directory loading.
    pub global_config_override: Option<PathBuf>,
    pub cache_dir: PathBuf,
    pub homebrew_api: String,
    pub app_store_api: String,
    pub child_env: BTreeMap<OsString, OsString>,
}

impl Settings {
    /// Derives config/cache paths from `home`; the child environment starts empty.
    pub fn new(home: PathBuf, mise: OsString) -> Self {
        Self {
            config_dir: home.join(".config/mise"),
            global_config_override: None,
            cache_dir: home.join(".cache/pkg"),
            home,
            mise,
            homebrew_api: DEFAULT_HOMEBREW_API.into(),
            app_store_api: DEFAULT_APP_STORE_API.into(),
            child_env: BTreeMap::new(),
        }
    }

    /// The default write target before mise's write_targets settings are applied.
    pub fn global_config(&self) -> PathBuf {
        self.global_config_override.clone().unwrap_or_else(|| self.config_dir.join("config.toml"))
    }

    pub(crate) fn validate(&self) -> Result<(), Error> {
        for (name, path) in [
            ("home", &self.home),
            ("configuration directory", &self.config_dir),
            ("cache", &self.cache_dir),
        ] {
            if !path.is_absolute() {
                return Err(Error::invalid_argument(format!("{name} path must be absolute")));
            }
        }
        if self.global_config_override.as_ref().is_some_and(|path| !path.is_absolute()) {
            return Err(Error::invalid_argument("global configuration override must be absolute"));
        }
        if self.mise.is_empty() {
            return Err(Error::invalid_argument("mise executable must not be empty"));
        }
        for url in [&self.homebrew_api, &self.app_store_api] {
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return Err(Error::invalid_argument("catalog URL must use HTTP or HTTPS"));
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Settings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Settings")
            .field("home", &self.home)
            .field("mise", &self.mise)
            .field("config_dir", &self.config_dir)
            .field("global_config_override", &self.global_config_override)
            .field("cache_dir", &self.cache_dir)
            .finish_non_exhaustive()
    }
}

/// The first executable file named `name` in the directories of a `PATH`-style list.
pub fn find_on_path(name: &OsStr, path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(name))
        .find(|candidate| is_executable(candidate))
}

pub(crate) fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}
