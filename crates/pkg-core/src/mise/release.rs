//! mise's own release: the version that runs, the newest it knows of, and whether it can replace
//! itself with `mise self-update`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::Mise;
use crate::error::Error;
use crate::settings::find_on_path;

/// What `mise version --json` reports.
#[derive(Clone, Debug)]
pub(crate) struct MiseRelease {
    /// The whole version line: `2026.11.9 macos-arm64 (2026-11-16)`.
    pub description: String,
    /// The version number alone: `2026.11.9`.
    pub version: String,
    /// The newest release mise knows of, without its minimum release age applied.
    pub latest: Option<String>,
}

impl MiseRelease {
    pub(crate) fn newer_available(&self) -> Option<&str> {
        self.latest.as_deref().filter(|latest| crate::outdated::is_newer(latest, &self.version))
    }
}

#[derive(Deserialize)]
struct VersionJson {
    version: String,
    #[serde(default)]
    latest: Option<String>,
}

/// The name mise itself goes by in `outdated` and `update` results.
pub(crate) const MISE: &str = "mise";

/// Why a package manager's mise is left to it.
pub(crate) const PACKAGE_MANAGER_REASON: &str =
    "installed by a package manager, which updates it; mise will not update itself";

/// Files whose presence under mise's install prefix turns `mise self-update` off, as mise's
/// packaging guide documents (https://mise.jdx.dev/packaging.html).
const SELF_UPDATE_MARKERS: &[&str] = &[
    "lib/.disable-self-update",
    "lib/mise/.disable-self-update",
    "lib64/mise/.disable-self-update",
    "lib/mise-self-update-instructions.toml",
    "lib/mise/mise-self-update-instructions.toml",
    "lib64/mise/mise-self-update-instructions.toml",
];

impl Mise<'_> {
    /// # Errors
    ///
    /// `PRECONDITION_FAILED` when mise cannot start, `MISE_FAILED` when it fails or its answer is
    /// not the JSON pkg expects.
    pub(crate) fn release(&self) -> Result<MiseRelease, Error> {
        let output = self.read(&["version", "--json"])?;
        let parsed: VersionJson = super::queries::parse(&output, "version --json")?;
        let version = parsed.version.split_whitespace().next().unwrap_or_default().to_string();
        Ok(MiseRelease { description: parsed.version, version, latest: parsed.latest })
    }

    /// Whether `mise self-update` may replace this executable. mise's packaging contract turns it
    /// off with a marker or instructions file under the install prefix (the executable's
    /// directory's parent, symlinks resolved), and `MISE_SELF_UPDATE_AVAILABLE=true` turns it
    /// back on.
    pub(crate) fn updates_itself(&self) -> bool {
        let forced = self.settings.child_env.get(OsStr::new("MISE_SELF_UPDATE_AVAILABLE"));
        if forced.is_some_and(|value| value == "true") {
            return true;
        }
        let Some(prefix) = self.executable().and_then(|path| install_prefix(&path)) else {
            return true;
        };
        !SELF_UPDATE_MARKERS.iter().any(|marker| prefix.join(marker).exists())
    }

    /// The executable as a path: the program itself when it names a path, else the first match
    /// on the child environment's `PATH`.
    pub(crate) fn executable(&self) -> Option<PathBuf> {
        let program = Path::new(&self.settings.mise);
        if program.components().count() > 1 {
            return Some(program.to_path_buf());
        }
        let path = self.settings.child_env.get(OsStr::new("PATH"))?;
        find_on_path(&self.settings.mise, path)
    }
}

fn install_prefix(executable: &Path) -> Option<PathBuf> {
    let resolved = std::fs::canonicalize(executable).ok()?;
    Some(resolved.parent()?.parent()?.to_path_buf())
}
