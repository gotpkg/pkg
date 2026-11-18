//! Homebrew's formula and cask API (formulae.brew.sh), which `search` and `info` read because
//! Homebrew itself is not installed: mise pours its bottles directly.
//!
//! Search needs every name and description, which Homebrew publishes as two large documents
//! (about 7 MB compressed). pkg keeps the parts it searches in a cache file and refreshes it once
//! a day.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Client;
use crate::error::{Error as Failure, ErrorDetails};

/// How long a downloaded index serves searches before pkg downloads it again.
pub(crate) const INDEX_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const INDEX_FILE: &str = "homebrew-index.json";

/// Every formula and cask, reduced to what search reads.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Index {
    /// Seconds since the Unix epoch when pkg downloaded it.
    pub fetched_at: u64,
    pub formulae: Vec<IndexEntry>,
    pub casks: Vec<IndexEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct IndexEntry {
    /// The formula name or cask token: what goes after `brew:` or `brew-cask:`.
    pub name: String,
    /// A cask's display name, such as `Mozilla Firefox`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub description: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

/// One formula, as `api/formula/<name>.json` describes it.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Formula {
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    pub versions: FormulaVersions,
    #[serde(default)]
    pub revision: u32,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub caveats: Option<String>,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct FormulaVersions {
    #[serde(default)]
    pub stable: Option<String>,
}

/// One cask, as `api/cask/<token>.json` describes it.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Cask {
    #[serde(default)]
    pub name: Vec<String>,
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub caveats: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<Value>,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default)]
    pub disabled: bool,
}

impl Formula {
    /// The version as Homebrew installs it: `1.25.0_2` for revision 2.
    pub(crate) fn version(&self) -> Option<String> {
        self.versions.stable.as_ref().map(|stable| match self.revision {
            0 => stable.clone(),
            revision => format!("{stable}_{revision}"),
        })
    }
}

impl Cask {
    /// The apps the cask puts in /Applications.
    pub(crate) fn apps(&self) -> Vec<String> {
        self.artifacts
            .iter()
            .filter_map(|artifact| artifact.get("app").and_then(Value::as_array))
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    }
}

#[derive(Deserialize)]
struct FormulaSummary {
    name: String,
    #[serde(default)]
    desc: Option<String>,
    versions: FormulaVersions,
    #[serde(default)]
    revision: u32,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Deserialize)]
struct CaskSummary {
    token: String,
    #[serde(default)]
    name: Vec<String>,
    #[serde(default)]
    desc: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    disabled: bool,
}

/// The index and what search should say about its age.
pub(crate) struct LoadedIndex {
    pub index: Index,
    /// Set when the download failed and an older index answered instead.
    pub warning: Option<String>,
}

pub(crate) struct Homebrew<'a> {
    client: &'a Client,
    api: &'a str,
    cache_dir: &'a Path,
}

impl<'a> Homebrew<'a> {
    pub(crate) fn new(client: &'a Client, api: &'a str, cache_dir: &'a Path) -> Homebrew<'a> {
        Homebrew { client, api, cache_dir }
    }

    pub(crate) fn index_path(&self) -> PathBuf {
        self.cache_dir.join(INDEX_FILE)
    }

    /// The cached index without downloading anything, when there is one.
    pub(crate) fn cached_index(&self) -> Option<Index> {
        let text = std::fs::read_to_string(self.index_path()).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// The search index: the cached one while it is younger than [`INDEX_MAX_AGE`], else a fresh
    /// download, else the stale cache with a warning.
    ///
    /// # Errors
    ///
    /// `UNAVAILABLE` when there is no cached index and Homebrew cannot be reached.
    pub(crate) fn index(&self) -> Result<LoadedIndex, Failure> {
        let cached = self.cached_index();
        let now = now();
        if let Some(index) = cached
            .as_ref()
            .filter(|index| now.saturating_sub(index.fetched_at) < INDEX_MAX_AGE.as_secs())
        {
            let index = Index {
                fetched_at: index.fetched_at,
                formulae: index.formulae.clone(),
                casks: index.casks.clone(),
            };
            return Ok(LoadedIndex { index, warning: None });
        }
        match self.download_index(now) {
            Ok(index) => {
                // A cache that cannot be written only costs the next search a download.
                let _ = self.write_index(&index);
                Ok(LoadedIndex { index, warning: None })
            }
            Err(failure) => match cached {
                Some(index) => Ok(LoadedIndex {
                    index,
                    warning: Some(format!(
                        "searched an older copy of Homebrew's catalog: {}",
                        failure.message
                    )),
                }),
                None => Err(failure),
            },
        }
    }

    fn download_index(&self, now: u64) -> Result<Index, Failure> {
        let formulae: Vec<FormulaSummary> = self.document("formula.json")?;
        let casks: Vec<CaskSummary> = self.document("cask.json")?;
        Ok(Index {
            fetched_at: now,
            formulae: formulae
                .into_iter()
                .filter(|formula| !formula.disabled)
                .map(|formula| IndexEntry {
                    version: match (formula.versions.stable, formula.revision) {
                        (Some(stable), 0) => stable,
                        (Some(stable), revision) => format!("{stable}_{revision}"),
                        (None, _) => String::new(),
                    },
                    name: formula.name,
                    title: None,
                    description: formula.desc.unwrap_or_default(),
                    aliases: formula.aliases,
                })
                .collect(),
            casks: casks
                .into_iter()
                .filter(|cask| !cask.disabled)
                .map(|cask| IndexEntry {
                    title: cask.name.into_iter().next(),
                    name: cask.token,
                    description: cask.desc.unwrap_or_default(),
                    version: cask.version.unwrap_or_default(),
                    aliases: Vec::new(),
                })
                .collect(),
        })
    }

    fn document<T: for<'de> Deserialize<'de>>(&self, name: &str) -> Result<T, Failure> {
        let url = format!("{}/{name}", self.api);
        self.client.get_json(&url, &[])?.ok_or_else(|| {
            Failure::unavailable(format!("Homebrew's catalog has no {name}"))
                .with_details(ErrorDetails { url: Some(url), ..Default::default() })
        })
    }

    fn write_index(&self, index: &Index) -> std::io::Result<()> {
        std::fs::create_dir_all(self.cache_dir)?;
        let path = self.index_path();
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, serde_json::to_vec(index)?)?;
        std::fs::rename(partial, path)
    }

    /// The formula `name`, or `None` when Homebrew's core tap has none (tap formulae included).
    ///
    /// # Errors
    ///
    /// `UNAVAILABLE` when Homebrew cannot be reached.
    pub(crate) fn formula(&self, name: &str) -> Result<Option<Formula>, Failure> {
        self.client.get_json(&format!("{}/formula/{name}.json", self.api), &[])
    }

    /// The cask `token`, or `None` when Homebrew's cask tap has none.
    ///
    /// # Errors
    ///
    /// `UNAVAILABLE` when Homebrew cannot be reached.
    pub(crate) fn cask(&self, token: &str) -> Result<Option<Cask>, Failure> {
        self.client.get_json(&format!("{}/cask/{token}.json", self.api), &[])
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_formula_version_carries_its_revision() {
        let formula: Formula = serde_json::from_value(json!({
            "name": "wget", "versions": { "stable": "1.25.0" }, "revision": 2
        }))
        .unwrap();
        assert_eq!(formula.version().as_deref(), Some("1.25.0_2"));
    }

    #[test]
    fn a_cask_lists_its_apps() {
        let cask: Cask = serde_json::from_value(json!({
            "token": "firefox",
            "artifacts": [{ "uninstall": [] }, { "app": ["Firefox.app"] }, { "zap": [] }]
        }))
        .unwrap();
        assert_eq!(cask.apps(), ["Firefox.app"]);
    }
}
