//! Resolves process defaults once at the CLI edge.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use pkg_core::Settings;
use serde_json::json;

use crate::failure::Failure;

/// The mise executable pkg runs; unset means `mise` on PATH.
pub(crate) const MISE_VARIABLE: &str = "PKG_MISE";
/// The Homebrew API's base URL; unset means [`DEFAULT_HOMEBREW_API`].
pub(crate) const HOMEBREW_API_VARIABLE: &str = "PKG_HOMEBREW_API";
/// The App Store (iTunes Search API) base URL; unset means [`DEFAULT_APP_STORE_API`].
pub(crate) const APP_STORE_API_VARIABLE: &str = "PKG_APP_STORE_API";

pub(crate) const DEFAULT_HOMEBREW_API: &str = "https://formulae.brew.sh/api";
pub(crate) const DEFAULT_APP_STORE_API: &str = "https://itunes.apple.com";

pub(crate) fn from_process() -> Result<Settings, Failure> {
    let snapshot: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
    let mut settings = resolve(|key| {
        snapshot.get(std::ffi::OsStr::new(key)).filter(|value| !value.is_empty()).cloned()
    })?;
    let mise_chosen =
        snapshot.get(std::ffi::OsStr::new(MISE_VARIABLE)).is_some_and(|value| !value.is_empty());
    settings.child_env = snapshot;
    crate::shims::keep_pkg_away_from_shims(&mut settings, mise_chosen);
    Ok(settings)
}

fn resolve(variable: impl Fn(&str) -> Option<OsString>) -> Result<Settings, Failure> {
    let home_variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let home = variable(home_variable).map(PathBuf::from).ok_or_else(|| {
        Failure::precondition(format!(
            "{home_variable} is not set, so pkg cannot find mise's global config"
        ))
        .with_details(json!({ "variable": home_variable }))
    })?;
    let absolute = |key: &str| variable(key).map(PathBuf::from).filter(|path| path.is_absolute());
    let global_config_override = variable("MISE_GLOBAL_CONFIG_FILE").map(PathBuf::from);
    let config_dir = absolute("MISE_CONFIG_DIR")
        .or_else(|| absolute("XDG_CONFIG_HOME").map(|directory| directory.join("mise")))
        .unwrap_or_else(|| home.join(".config/mise"));
    let cache_dir = absolute("XDG_CACHE_HOME")
        .map(|directory| directory.join(crate::NAME))
        .or_else(|| {
            cfg!(windows)
                .then(|| absolute("LOCALAPPDATA"))
                .flatten()
                .map(|directory| directory.join(crate::NAME).join("cache"))
        })
        .unwrap_or_else(|| home.join(".cache").join(crate::NAME));
    let text = |key: &str, default: &str| {
        variable(key)
            .map(|value| value.to_string_lossy().trim_end_matches('/').to_string())
            .unwrap_or_else(|| default.to_string())
    };
    Ok(Settings {
        mise: variable(MISE_VARIABLE).unwrap_or_else(|| "mise".into()),
        child_env: BTreeMap::new(),
        homebrew_api: text(HOMEBREW_API_VARIABLE, DEFAULT_HOMEBREW_API),
        app_store_api: text(APP_STORE_API_VARIABLE, DEFAULT_APP_STORE_API),
        config_dir,
        global_config_override,
        cache_dir,
        home,
    })
}

/// `path` with every symlink resolved, so `~/.config/mise/config.toml` reads as the dotfiles
/// checkout it links into; `path` itself when it does not exist.
pub(crate) fn real_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(pairs: &[(&str, &str)]) -> Settings {
        super::resolve(|key| {
            pairs.iter().find(|(name, _)| *name == key).map(|(_, value)| OsString::from(value))
        })
        .unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn defaults_live_under_the_home_directory() {
        let environment = resolve(&[("HOME", "/home/mark")]);
        assert_eq!(environment.global_config(), Path::new("/home/mark/.config/mise/config.toml"));
        assert_eq!(environment.cache_dir, Path::new("/home/mark/.cache/pkg"));
        assert_eq!(environment.homebrew_api, DEFAULT_HOMEBREW_API);
    }

    #[cfg(unix)]
    #[test]
    fn mise_and_xdg_variables_move_the_global_config_and_the_cache() {
        let environment = resolve(&[
            ("HOME", "/home/mark"),
            ("MISE_CONFIG_DIR", "/etc/mise-config"),
            ("XDG_CACHE_HOME", "/tmp/cache"),
            ("PKG_HOMEBREW_API", "http://127.0.0.1:9/api/"),
        ]);
        assert_eq!(environment.global_config(), Path::new("/etc/mise-config/config.toml"));
        assert_eq!(environment.cache_dir, Path::new("/tmp/cache/pkg"));
        assert_eq!(environment.homebrew_api, "http://127.0.0.1:9/api");
    }

    #[test]
    fn no_home_is_a_precondition_failure() {
        let failure = super::resolve(|_| None).err().unwrap();
        assert_eq!(failure.code, crate::failure::ErrorCode::PreconditionFailed);
    }
}
