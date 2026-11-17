//! Where pkg finds mise, the catalogs, and its cache, resolved once from the process environment.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::failure::Failure;
use crate::mise::Mise;

/// The mise executable pkg runs; unset means `mise` on PATH.
pub const MISE_VARIABLE: &str = "PKG_MISE";
/// The Homebrew API's base URL; unset means [`DEFAULT_HOMEBREW_API`].
pub const HOMEBREW_API_VARIABLE: &str = "PKG_HOMEBREW_API";
/// The App Store (iTunes Search API) base URL; unset means [`DEFAULT_APP_STORE_API`].
pub const APP_STORE_API_VARIABLE: &str = "PKG_APP_STORE_API";

pub const DEFAULT_HOMEBREW_API: &str = "https://formulae.brew.sh/api";
pub const DEFAULT_APP_STORE_API: &str = "https://itunes.apple.com";

pub struct Environment {
    /// Your home directory. pkg runs every mise command there, so a project's mise.toml in the
    /// current directory never changes what pkg installs or where it records it.
    pub home: PathBuf,
    pub mise: Mise,
    /// mise's global config file, where `-g` writes when no `write_targets` setting says otherwise.
    pub global_config: PathBuf,
    pub homebrew_api: String,
    pub app_store_api: String,
    /// Where pkg keeps the Homebrew search index.
    pub cache_dir: PathBuf,
}

impl Environment {
    /// # Errors
    ///
    /// `PRECONDITION_FAILED` when no home directory is set.
    pub fn from_process() -> Result<Environment, Failure> {
        Environment::resolve(|key| std::env::var_os(key).filter(|value| !value.is_empty()))
    }

    /// Resolves every location from `variable`, which reads one environment variable.
    ///
    /// # Errors
    ///
    /// `PRECONDITION_FAILED` when neither `HOME` nor, on Windows, `USERPROFILE` is set.
    pub fn resolve(variable: impl Fn(&str) -> Option<OsString>) -> Result<Environment, Failure> {
        let home_variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let home = variable(home_variable).map(PathBuf::from).ok_or_else(|| {
            Failure::precondition(format!(
                "{home_variable} is not set, so pkg cannot find mise's global config"
            ))
            .with_details(json!({ "variable": home_variable }))
        })?;
        let absolute =
            |key: &str| variable(key).map(PathBuf::from).filter(|path| path.is_absolute());
        let global_config = variable("MISE_GLOBAL_CONFIG_FILE")
            .map(PathBuf::from)
            .or_else(|| absolute("MISE_CONFIG_DIR").map(|directory| directory.join("config.toml")))
            .or_else(|| {
                absolute("XDG_CONFIG_HOME").map(|directory| directory.join("mise/config.toml"))
            })
            .unwrap_or_else(|| home.join(".config/mise/config.toml"));
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
        Ok(Environment {
            mise: Mise::new(variable(MISE_VARIABLE).unwrap_or_else(|| "mise".into()), home.clone()),
            homebrew_api: text(HOMEBREW_API_VARIABLE, DEFAULT_HOMEBREW_API),
            app_store_api: text(APP_STORE_API_VARIABLE, DEFAULT_APP_STORE_API),
            global_config,
            cache_dir,
            home,
        })
    }

    /// `path` for people: under the home directory it starts with `~`.
    pub fn tilde(&self, path: &Path) -> String {
        match path.strip_prefix(&self.home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        }
    }

    /// `path` with a leading `~` replaced by the home directory, as mise writes paths in settings.
    pub fn expand(&self, path: &str) -> PathBuf {
        match path.strip_prefix("~/") {
            Some(rest) => self.home.join(rest),
            None if path == "~" => self.home.clone(),
            None => PathBuf::from(path),
        }
    }
}

/// `path` with every symlink resolved, so `~/.config/mise/config.toml` reads as the dotfiles
/// checkout it links into; `path` itself when it does not exist.
pub fn real_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(pairs: &[(&str, &str)]) -> Environment {
        Environment::resolve(|key| {
            pairs.iter().find(|(name, _)| *name == key).map(|(_, value)| OsString::from(value))
        })
        .unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn defaults_live_under_the_home_directory() {
        let environment = resolve(&[("HOME", "/home/mark")]);
        assert_eq!(environment.global_config, Path::new("/home/mark/.config/mise/config.toml"));
        assert_eq!(environment.cache_dir, Path::new("/home/mark/.cache/pkg"));
        assert_eq!(environment.homebrew_api, DEFAULT_HOMEBREW_API);
        assert_eq!(
            environment.tilde(Path::new("/home/mark/.dotfiles/x.toml")),
            "~/.dotfiles/x.toml"
        );
        assert_eq!(
            environment.expand("~/.config/mise/a.toml"),
            Path::new("/home/mark/.config/mise/a.toml")
        );
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
        assert_eq!(environment.global_config, Path::new("/etc/mise-config/config.toml"));
        assert_eq!(environment.cache_dir, Path::new("/tmp/cache/pkg"));
        assert_eq!(environment.homebrew_api, "http://127.0.0.1:9/api");
    }

    #[test]
    fn no_home_is_a_precondition_failure() {
        let failure = Environment::resolve(|_| None).err().unwrap();
        assert_eq!(failure.code, crate::failure::ErrorCode::PreconditionFailed);
    }
}
