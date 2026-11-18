//! Release helpers for pkg, run as `cargo xtask <operation>`:
//!
//! - `package [<target>]` builds the release binary for a Rust target triple (default: this
//!   machine's) and writes `artifacts/pkg-<target>.tar.gz` (`.zip` for Windows): one
//!   top-level `pkg-<target>/` directory holding the executable, the layout Mise's `github:`
//!   backend unpacks without options.
//! - `installer [--output <file>]` writes the release's installer: `installer/install.sh` with
//!   the version in `crates/pkg/Cargo.toml` written in, so it downloads that release's archives.
//!   Default output: `artifacts/install.sh`.
//! - `checksums [--artifacts <dir>]` writes `<dir>/SHA256SUMS` (default `artifacts/`): one
//!   `<sha256>  <archive>` line, as `sha256sum` prints it, per release archive in `<dir>`. The
//!   installer checks its download against it.

use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The shipped package and executable.
const PACKAGE: &str = "pkg";

/// Every target a release ships an archive for.
const RELEASE_TARGETS: &[&str] = &[
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-musl",
    "x86_64-unknown-linux-musl",
    "x86_64-pc-windows-msvc",
];

/// What `installer/install.sh` assigns to `release_version`; `installer` writes the version over
/// it.
const INSTALLER_VERSION_PLACEHOLDER: &str = "@PKG_RELEASE_VERSION@";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.first().map(String::as_str) {
        Some("package") => match &args[1..] {
            [] => host_target().and_then(|target| package(&target)),
            [target] => package(target),
            _ => Err(usage()),
        },
        Some("installer") => installer(&args[1..]),
        Some("checksums") => checksums(&args[1..]),
        _ => Err(usage()),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> String {
    "usage: cargo xtask package [<target>]\n       \
     cargo xtask installer [--output <file>]\n       \
     cargo xtask checksums [--artifacts <dir>]"
        .to_string()
}

/// Reads `--<name> <path>` pairs into the slot of each name in `names`.
fn path_options<const N: usize>(
    args: &[String],
    names: [&str; N],
) -> Result<[Option<PathBuf>; N], String> {
    let mut values = [const { None }; N];
    let mut rest = args.iter();
    while let Some(name) = rest.next() {
        let slot = names.iter().position(|known| known == name).ok_or_else(usage)?;
        values[slot] = Some(PathBuf::from(rest.next().ok_or_else(usage)?));
    }
    Ok(values)
}

/// The target triple `rustc` builds for by default on this machine.
fn host_target() -> Result<String, String> {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let output = Command::new(rustc)
        .arg("-vV")
        .output()
        .map_err(|error| format!("cannot run rustc -vV: {error}"))?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_string)
        .ok_or_else(|| "rustc -vV did not name the host target".to_string())
}

/// What `cargo metadata` says about the workspace and the shipped package.
struct Metadata {
    workspace_root: PathBuf,
    target_directory: PathBuf,
    version: String,
}

fn metadata() -> Result<Metadata, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|error| format!("cannot run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cargo metadata printed invalid JSON: {error}"))?;
    let package = metadata["packages"]
        .as_array()
        .and_then(|packages| packages.iter().find(|package| package["name"] == PACKAGE))
        .ok_or_else(|| format!("the workspace has no package `{PACKAGE}`"))?;
    let text = |value: &Value| value.as_str().unwrap_or_default().to_string();
    Ok(Metadata {
        workspace_root: PathBuf::from(text(&metadata["workspace_root"])),
        target_directory: PathBuf::from(text(&metadata["target_directory"])),
        version: text(&package["version"]),
    })
}

fn package(target: &str) -> Result<(), String> {
    let metadata = metadata()?;
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(cargo)
        .args(["build", "--release", "--locked", "--package", PACKAGE, "--target", target])
        .current_dir(&metadata.workspace_root)
        .status()
        .map_err(|error| format!("cannot run cargo build: {error}"))?;
    if !status.success() {
        return Err(format!("cargo build for {target} failed"));
    }
    let executable = executable_name(target);
    let binary = metadata.target_directory.join(target).join("release").join(&executable);
    let artifacts = metadata.workspace_root.join("artifacts");
    fs::create_dir_all(&artifacts)
        .map_err(|error| format!("cannot create {}: {error}", artifacts.display()))?;
    let archive = artifacts.join(archive_name(target));
    write_archive(&binary, target, &archive)
        .map_err(|error| format!("cannot write {}: {error}", archive.display()))?;
    println!("{}", archive.display());
    Ok(())
}

fn executable_name(target: &str) -> String {
    if is_windows(target) { format!("{PACKAGE}.exe") } else { PACKAGE.to_string() }
}

fn is_windows(target: &str) -> bool {
    target.contains("-windows-")
}

/// `pkg-<target>.tar.gz`, or `.zip` for Windows: the Rust target triple in the name is what
/// Mise's `github:` backend matches against the installing machine's OS, CPU, and C library.
fn archive_name(target: &str) -> String {
    let extension = if is_windows(target) { "zip" } else { "tar.gz" };
    format!("{PACKAGE}-{target}.{extension}")
}

/// Writes `binary` into `archive` as `pkg-<target>/<executable>`, mode 0755.
fn write_archive(binary: &Path, target: &str, archive: &Path) -> io::Result<()> {
    let entry = format!("{PACKAGE}-{target}/{}", executable_name(target));
    let contents = fs::read(binary)?;
    if is_windows(target) {
        let mut zip = zip::ZipWriter::new(File::create(archive)?);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);
        zip.start_file(entry, options).map_err(io::Error::other)?;
        zip.write_all(&contents)?;
        zip.finish().map_err(io::Error::other)?;
    } else {
        let mut builder =
            tar::Builder::new(GzEncoder::new(File::create(archive)?, Compression::best()));
        let mut header = tar::Header::new_gnu();
        header.set_size(u64::try_from(contents.len()).map_err(io::Error::other)?);
        header.set_mode(0o755);
        header.set_mtime(
            fs::metadata(binary)?
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_secs()),
        );
        header.set_cksum();
        builder.append_data(&mut header, entry, contents.as_slice())?;
        builder.into_inner()?.finish()?;
    }
    Ok(())
}

fn installer(args: &[String]) -> Result<(), String> {
    let [output] = path_options(args, ["--output"])?;
    let metadata = metadata()?;
    let template = metadata.workspace_root.join("installer").join("install.sh");
    let script = fs::read_to_string(&template)
        .map_err(|error| format!("cannot read {}: {error}", template.display()))
        .and_then(|template| render_installer(&template, &metadata.version))?;
    let output =
        output.unwrap_or_else(|| metadata.workspace_root.join("artifacts").join("install.sh"));
    if let Some(directory) = output.parent() {
        fs::create_dir_all(directory)
            .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    }
    fs::write(&output, script)
        .map_err(|error| format!("cannot write {}: {error}", output.display()))?;
    println!("{}", output.display());
    Ok(())
}

/// The installer for `version`: the template with its one version placeholder replaced.
fn render_installer(template: &str, version: &str) -> Result<String, String> {
    match template.matches(INSTALLER_VERSION_PLACEHOLDER).count() {
        1 => Ok(template.replacen(INSTALLER_VERSION_PLACEHOLDER, version, 1)),
        count => Err(format!(
            "installer/install.sh must assign {INSTALLER_VERSION_PLACEHOLDER} once, not {count} \
             times"
        )),
    }
}

fn checksums(args: &[String]) -> Result<(), String> {
    let [artifacts] = path_options(args, ["--artifacts"])?;
    let artifacts = match artifacts {
        Some(artifacts) => artifacts,
        None => metadata()?.workspace_root.join("artifacts"),
    };
    let mut digests = Vec::new();
    for target in RELEASE_TARGETS {
        let archive = archive_name(target);
        let path = artifacts.join(&archive);
        match fs::read(&path) {
            Ok(contents) => digests.push((archive, sha256(&contents))),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
        }
    }
    if digests.is_empty() {
        return Err(format!("{} holds no release archives", artifacts.display()));
    }
    let output = artifacts.join("SHA256SUMS");
    fs::write(&output, render_checksums(digests))
        .map_err(|error| format!("cannot write {}: {error}", output.display()))?;
    println!("{}", output.display());
    Ok(())
}

/// One `<sha256>  <archive>` line per `(archive, digest)` pair, sorted by archive name.
fn render_checksums(mut digests: Vec<(String, String)>) -> String {
    digests.sort();
    digests.iter().map(|(archive, digest)| format!("{digest}  {archive}\n")).collect()
}

fn sha256(contents: &[u8]) -> String {
    Sha256::digest(contents).iter().fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use flate2::read::GzDecoder;

    use super::*;

    #[test]
    fn archive_names_carry_the_target_triple() {
        assert_eq!(
            archive_name("aarch64-apple-darwin"),
            format!("{PACKAGE}-aarch64-apple-darwin.tar.gz")
        );
        assert_eq!(
            archive_name("x86_64-pc-windows-msvc"),
            format!("{PACKAGE}-x86_64-pc-windows-msvc.zip")
        );
    }

    #[test]
    fn archives_hold_one_executable_under_a_top_level_directory() {
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("binary");
        fs::write(&binary, b"executable").unwrap();

        let target = "aarch64-unknown-linux-musl";
        let tarball = directory.path().join(archive_name(target));
        write_archive(&binary, target, &tarball).unwrap();
        let mut archive = tar::Archive::new(GzDecoder::new(File::open(&tarball).unwrap()));
        let mut entries = archive.entries().unwrap();
        let mut entry = entries.next().unwrap().unwrap();
        assert_eq!(entry.path().unwrap(), Path::new(&format!("{PACKAGE}-{target}/{PACKAGE}")));
        assert_eq!(entry.header().mode().unwrap(), 0o755);
        let mut contents = String::new();
        entry.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "executable");
        assert!(entries.next().is_none());

        let target = "x86_64-pc-windows-msvc";
        let zipfile = directory.path().join(archive_name(target));
        write_archive(&binary, target, &zipfile).unwrap();
        let mut archive = zip::ZipArchive::new(File::open(&zipfile).unwrap()).unwrap();
        assert_eq!(archive.len(), 1);
        let mut entry = archive.by_index(0).unwrap();
        assert_eq!(entry.name().unwrap(), format!("{PACKAGE}-{target}/{PACKAGE}.exe"));
        let mut contents = String::new();
        entry.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "executable");
    }

    #[test]
    fn digests_are_lowercase_hex_sha256() {
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn the_installer_names_its_release_on_the_line_release_yml_reads() {
        let installer =
            render_installer(include_str!("../../installer/install.sh"), "1.2.3").unwrap();
        assert!(installer.lines().any(|line| line == r#"release_version="1.2.3""#));
        assert!(!installer.contains(INSTALLER_VERSION_PLACEHOLDER));
    }

    #[test]
    fn an_installer_template_must_hold_the_placeholder_once() {
        assert!(render_installer("release_version=\"\"\n", "1.2.3").is_err());
        let twice = INSTALLER_VERSION_PLACEHOLDER.repeat(2);
        assert!(render_installer(&twice, "1.2.3").is_err());
    }

    #[test]
    fn the_checksum_manifest_reads_like_sha256sum_output_sorted_by_archive() {
        let manifest = render_checksums(vec![
            ("pkg-x86_64-pc-windows-msvc.zip".into(), "1".repeat(64)),
            ("pkg-aarch64-apple-darwin.tar.gz".into(), "2".repeat(64)),
        ]);
        assert_eq!(
            manifest,
            format!(
                "{}  pkg-aarch64-apple-darwin.tar.gz\n{}  pkg-x86_64-pc-windows-msvc.zip\n",
                "2".repeat(64),
                "1".repeat(64)
            )
        );
    }
}
