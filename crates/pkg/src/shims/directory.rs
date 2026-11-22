//! The shim directory: where it is, and installing, removing, and listing what is in it.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};

use super::catalog::{self, System};
use super::launcher;
use super::path;

/// Where shims are written: `$XDG_DATA_HOME/pkg/shims`, else `~/.local/share/pkg/shims`, and on
/// Windows `%LOCALAPPDATA%\pkg\shims`. `None` without a home directory.
pub(crate) fn location(
    system: System,
    variable: impl Fn(&str) -> Option<OsString>,
) -> Option<PathBuf> {
    let absolute = |key: &str| variable(key).map(PathBuf::from).filter(|path| path.is_absolute());
    let data = if system == System::Windows {
        absolute("LOCALAPPDATA")
            .or_else(|| absolute("USERPROFILE").map(|home| home.join("AppData").join("Local")))?
    } else {
        absolute("XDG_DATA_HOME")
            .or_else(|| absolute("HOME").map(|home| home.join(".local/share")))?
    };
    Some(data.join("pkg").join("shims"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    Created,
    Updated,
    Unchanged,
    Removed,
}

/// One shim file and what happened, or would happen, to it.
#[derive(Debug)]
pub(crate) struct ShimFile {
    pub(crate) name: String,
    pub(crate) file: PathBuf,
    pub(crate) change: Change,
}

#[derive(Debug)]
pub(crate) enum InstallError {
    /// Files that are not shims have the names of shims; nothing was written.
    Conflict(Vec<PathBuf>),
    Io(io::Error),
}

impl From<io::Error> for InstallError {
    fn from(error: io::Error) -> InstallError {
        InstallError::Io(error)
    }
}

/// Writes the shims for `system` into `directory`; with `dry_run`, only works out what would
/// change. Replaces only files that carry the marker, and writes nothing when any other file has
/// a shim's name.
pub(crate) fn install(
    system: System,
    directory: &Path,
    dry_run: bool,
) -> Result<Vec<ShimFile>, InstallError> {
    let mut shims = Vec::new();
    let mut conflicts = Vec::new();
    let mut scripts = Vec::new();
    for tool in catalog::for_system(system) {
        let file = directory.join(launcher::file_name(tool.name, system));
        let script = launcher::script(tool.name, system, directory);
        let change = match std::fs::read(&file) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Change::Created,
            Err(_) if file.is_dir() => {
                conflicts.push(file);
                continue;
            }
            Err(error) => return Err(error.into()),
            Ok(existing) if !launcher::is_shim(&existing) => {
                conflicts.push(file);
                continue;
            }
            Ok(existing) if existing == script.as_bytes() => Change::Unchanged,
            Ok(_) => Change::Updated,
        };
        scripts.push(script);
        shims.push(ShimFile { name: tool.name.to_string(), file, change });
    }
    if !conflicts.is_empty() {
        return Err(InstallError::Conflict(conflicts));
    }
    if !dry_run {
        std::fs::create_dir_all(directory)?;
        for (shim, script) in shims.iter().zip(&scripts) {
            if shim.change != Change::Unchanged {
                write_executable(&shim.file, script)?;
            }
        }
    }
    Ok(shims)
}

/// Writes through a temporary file and renames it over `file`, so a shim that is running is never
/// rewritten under it.
fn write_executable(file: &Path, script: &str) -> io::Result<()> {
    let temporary = file.with_extension("pkg-tmp");
    std::fs::write(&temporary, script)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&temporary, file)
}

/// Deletes every file in `directory` that carries the marker, then the directory when nothing
/// else is in it. With `dry_run`, only lists what would go.
pub(crate) fn remove(directory: &Path, dry_run: bool) -> io::Result<Vec<ShimFile>> {
    let entries = match std::fs::read_dir(directory) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        other => other?,
    };
    let mut shims = Vec::new();
    let mut others = 0;
    for entry in entries {
        let file = entry?.path();
        match read_start(&file) {
            Some(start) if launcher::is_shim(&start) => {
                let name = file.file_name().unwrap_or_default().to_string_lossy();
                shims.push(ShimFile {
                    name: launcher::tool_name(&name).to_string(),
                    file,
                    change: Change::Removed,
                });
            }
            _ => others += 1,
        }
    }
    shims.sort_by(|first, second| first.name.cmp(&second.name));
    if !dry_run {
        for shim in &shims {
            std::fs::remove_file(&shim.file)?;
        }
        if others == 0 {
            std::fs::remove_dir(directory)?;
        }
    }
    Ok(shims)
}

/// What a shim's name runs from the current `PATH`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Resolution {
    /// The shim.
    Shim,
    /// Another executable, which comes first on `PATH`.
    Other(PathBuf),
    Nothing,
}

/// One shim of this system, as `pkg shims list` reports it.
pub(crate) struct Listed {
    pub(crate) name: String,
    pub(crate) file: PathBuf,
    pub(crate) installed: bool,
    pub(crate) resolves_to: Resolution,
}

/// The shims of `system`, whether each is installed, and what its name runs from `search_path`.
pub(crate) fn list(
    system: System,
    directory: &Path,
    search_path: Option<&OsStr>,
    pathext: Option<&OsStr>,
) -> Vec<Listed> {
    let directories = search_path.map(path::directories).unwrap_or_default();
    let extensions = path::executable_extensions(pathext);
    catalog::for_system(system)
        .map(|tool| {
            let file = directory.join(launcher::file_name(tool.name, system));
            let installed = read_start(&file).is_some_and(|start| launcher::is_shim(&start));
            let resolves_to =
                match path::find_executable(&directories, tool.name, &extensions, |_| false) {
                    None => Resolution::Nothing,
                    Some(found)
                        if found
                            .parent()
                            .is_some_and(|parent| path::same_directory(parent, directory)) =>
                    {
                        Resolution::Shim
                    }
                    Some(found) => Resolution::Other(found),
                };
            Listed { name: tool.name.to_string(), file, installed, resolves_to }
        })
        .collect()
}

/// The first bytes of a file, or `None` when it cannot be read.
pub(crate) fn read_start(file: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut start = Vec::new();
    std::fs::File::open(file).ok()?.take(512).read_to_end(&mut start).ok()?;
    Some(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variables(pairs: &'static [(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        move |key| pairs.iter().find(|(name, _)| *name == key).map(|(_, value)| (*value).into())
    }

    #[cfg(unix)]
    #[test]
    fn the_directory_follows_xdg_data_home_then_home() {
        let default = location(System::Linux, variables(&[("HOME", "/home/mark")]));
        assert_eq!(default.as_deref(), Some(Path::new("/home/mark/.local/share/pkg/shims")));
        let moved = location(
            System::Macos,
            variables(&[("HOME", "/home/mark"), ("XDG_DATA_HOME", "/data")]),
        );
        assert_eq!(moved.as_deref(), Some(Path::new("/data/pkg/shims")));
        let relative =
            location(System::Linux, variables(&[("HOME", "/h"), ("XDG_DATA_HOME", "data")]));
        assert_eq!(relative.as_deref(), Some(Path::new("/h/.local/share/pkg/shims")));
        assert_eq!(location(System::Linux, variables(&[])), None);
    }

    #[cfg(unix)]
    #[test]
    fn on_windows_the_directory_is_under_local_app_data() {
        let local = location(System::Windows, variables(&[("LOCALAPPDATA", "/c/local")]));
        assert_eq!(local.as_deref(), Some(Path::new("/c/local/pkg/shims")));
        let profile = location(System::Windows, variables(&[("USERPROFILE", "/c/users/mark")]));
        assert_eq!(profile.as_deref(), Some(Path::new("/c/users/mark/AppData/Local/pkg/shims")));
    }

    #[test]
    fn a_file_that_is_not_a_shim_stops_the_install_before_anything_is_written() {
        let temporary = tempfile::tempdir().expect("a directory");
        let directory = temporary.path().join("shims");
        std::fs::create_dir(&directory).expect("a directory");
        let system = System::current();
        let name = launcher::file_name("npm", system);
        std::fs::write(directory.join(&name), "mine").expect("a foreign file");

        let Err(InstallError::Conflict(files)) = install(system, &directory, false) else {
            panic!("the install went ahead");
        };
        assert_eq!(files, [directory.join(&name)]);
        assert_eq!(std::fs::read_dir(&directory).expect("a directory").count(), 1);
    }

    #[test]
    fn removing_leaves_a_directory_that_holds_anything_else() {
        let temporary = tempfile::tempdir().expect("a directory");
        let directory = temporary.path().join("shims");
        let system = System::current();
        install(system, &directory, false).expect("installs");
        std::fs::write(directory.join("notes.txt"), "mine").expect("a foreign file");

        let removed = remove(&directory, false).expect("removes");
        assert!(removed.iter().all(|shim| shim.change == Change::Removed));
        assert!(directory.join("notes.txt").exists());
        std::fs::remove_file(directory.join("notes.txt")).expect("a file");
        install(system, &directory, false).expect("installs");
        remove(&directory, false).expect("removes");
        assert!(!directory.exists());
    }
}
