//! The installer, as `cargo xtask installer` renders it, run by `/bin/sh` against fake releases
//! served from `file://` URLs. `HOME`, `TMPDIR` and the release server live in a temporary
//! directory and `PATH` holds only system directories, so no test reaches the network or the
//! real home.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::LazyLock;

use flate2::Compression;
use flate2::write::GzEncoder;
use tempfile::TempDir;

/// Where curl, tar, and shasum or sha256sum live on macOS and Linux.
const SYSTEM_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// Every fake release ships all four, so the tests pass on any host the installer supports.
const INSTALLER_TARGETS: [&str; 4] = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-musl",
    "x86_64-unknown-linux-musl",
];

const POSIX_BLOCK: &str = "# >>> pkg >>>\nexport PATH=\"$HOME/.local/bin:$PATH\"\n# <<< pkg <<<\n";
const FISH_BLOCK: &str = "# >>> pkg >>>\nset -gx PATH \"$HOME/.local/bin\" $PATH\n# <<< pkg <<<\n";

struct Rendered {
    script: PathBuf,
    version: String,
}

/// `cargo xtask installer`'s script, rendered once per test run, and the version it embeds.
static RENDERED: LazyLock<Rendered> = LazyLock::new(|| {
    let script = Path::new(env!("CARGO_TARGET_TMPDIR")).join("install.sh");
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["installer", "--output"])
        .arg(&script)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = fs::read_to_string(&script).unwrap();
    let version = text
        .lines()
        .find_map(|line| line.strip_prefix("release_version=\"")?.strip_suffix('"'))
        .expect("the rendered installer names its release")
        .to_string();
    Rendered { script, version }
});

fn release() -> &'static str {
    &RENDERED.version
}

/// A released version other than the rendered script's own.
fn previous() -> String {
    format!("{}-previous", release())
}

fn fake_pkg(version: &str) -> String {
    format!("#!/bin/sh\necho 'pkg {version}'\n")
}

struct Fixture {
    root: TempDir,
}

impl Fixture {
    /// An empty home, and a release server holding the rendered script's release.
    fn new() -> Self {
        let fixture = Self { root: tempfile::tempdir().unwrap() };
        for directory in ["home", "releases", "tmp"] {
            fs::create_dir(fixture.root.path().join(directory)).unwrap();
        }
        fixture.publish(release());
        fixture
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn bin(&self) -> PathBuf {
        self.home().join(".local/bin")
    }

    fn installed(&self) -> PathBuf {
        self.bin().join("pkg")
    }

    fn tmp(&self) -> PathBuf {
        self.root.path().join("tmp")
    }

    fn release_directory(&self, version: &str) -> PathBuf {
        self.root.path().join("releases/download").join(format!("v{version}"))
    }

    /// Publishes `version`: per target, an archive holding a fake pkg that reports `version`,
    /// then the SHA256SUMS `cargo xtask checksums` writes for them.
    fn publish(&self, version: &str) {
        self.publish_holding(version, &[]);
    }

    /// Publishes `version` with `extra` files beside pkg in every archive.
    fn publish_holding(&self, version: &str, extra: &[&str]) {
        self.write_archives(version, &fake_pkg(version), extra);
        let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(["checksums", "--artifacts"])
            .arg(self.release_directory(version))
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }

    /// Replaces `version`'s archives with ones holding `pkg`, leaving its SHA256SUMS alone.
    fn tamper(&self, version: &str, pkg: &str) {
        self.write_archives(version, pkg, &[]);
    }

    fn write_archives(&self, version: &str, pkg: &str, extra: &[&str]) {
        let directory = self.release_directory(version);
        fs::create_dir_all(&directory).unwrap();
        for target in INSTALLER_TARGETS {
            let archive = File::create(directory.join(format!("pkg-{target}.tar.gz"))).unwrap();
            let mut builder = tar::Builder::new(GzEncoder::new(archive, Compression::fast()));
            let members = std::iter::once(("pkg", pkg)).chain(extra.iter().map(|name| (*name, "")));
            for (name, contents) in members {
                let mut header = tar::Header::new_gnu();
                header.set_size(u64::try_from(contents.len()).unwrap());
                header.set_mode(0o755);
                header.set_cksum();
                builder
                    .append_data(&mut header, format!("pkg-{target}/{name}"), contents.as_bytes())
                    .unwrap();
            }
            builder.into_inner().unwrap().finish().unwrap();
        }
    }

    /// The rendered installer, run for a person whose login shell is `shell`.
    fn installer(&self, shell: &str) -> Command {
        self.script(&RENDERED.script, shell)
    }

    fn script(&self, script: &Path, shell: &str) -> Command {
        let releases = self.root.path().join("releases");
        let mut command = Command::new("/bin/sh");
        command
            .arg(script)
            .env_clear()
            .env("HOME", self.home())
            .env("SHELL", format!("/bin/{shell}"))
            .env("PATH", SYSTEM_PATH)
            .env("TMPDIR", self.tmp())
            .env("PKG_RELEASES_URL", format!("file://{}", releases.display()));
        command
    }

    /// Puts a pkg that reports `version` at the destination, as an earlier install would.
    fn install_fake(&self, version: &str) {
        fs::create_dir_all(self.bin()).unwrap();
        write_executable(&self.installed(), &fake_pkg(version));
    }
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// Runs `command`, asserts it succeeded with nothing on stderr but progress, and returns stdout.
fn succeed(command: &mut Command) -> String {
    let Output { status, stdout, stderr } = command.output().unwrap();
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(status.success(), "the installer failed:\n{stderr}");
    assert!(!stderr.contains("error:"), "{stderr}");
    String::from_utf8(stdout).unwrap()
}

/// Runs `command`, asserts it failed with exit status 1 and nothing on stdout, and returns
/// stderr.
fn fail(command: &mut Command) -> String {
    let Output { status, stdout, stderr } = command.output().unwrap();
    let stderr = String::from_utf8(stderr).unwrap();
    assert_eq!(status.code(), Some(1), "{stderr}");
    assert_eq!(String::from_utf8_lossy(&stdout), "");
    stderr
}

/// Every file, directory and link under `root`, with contents, mode and link target.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, String> {
    let mut entries = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            let description = if metadata.is_symlink() {
                format!("link to {}", fs::read_link(&path).unwrap().display())
            } else if metadata.is_dir() {
                pending.push(path.clone());
                "directory".to_string()
            } else {
                let contents = fs::read(&path).unwrap();
                let mode = metadata.permissions().mode();
                format!("file {mode:o}: {}", String::from_utf8_lossy(&contents))
            };
            entries.insert(path.strip_prefix(root).unwrap().to_path_buf(), description);
        }
    }
    entries
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

#[test]
fn a_fresh_install_puts_pkg_on_path_for_new_zsh_shells() {
    let fixture = Fixture::new();

    let stdout = succeed(&mut fixture.installer("zsh"));

    assert_eq!(read(&fixture.installed()), fake_pkg(release()));
    let mode = fs::metadata(fixture.installed()).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755);
    assert_eq!(fs::read_dir(fixture.bin()).unwrap().count(), 1, "nothing staged is left behind");
    assert_eq!(read(&fixture.home().join(".zshenv")), POSIX_BLOCK);
    let installed = format!("Installed pkg {} (", release());
    assert!(stdout.contains(&installed), "{stdout}");
    assert!(stdout.contains(&fixture.installed().display().to_string()), "{stdout}");
    assert!(stdout.contains("    source ~/.zshenv\n"), "{stdout}");
    assert!(stdout.contains("pkg bootstrap --from <your dotfiles repository>"), "{stdout}");
    assert!(stdout.contains("    pkg setup\n"), "{stdout}");
    assert_eq!(fs::read_dir(fixture.tmp()).unwrap().count(), 0, "the download is cleaned up");
}

#[test]
fn a_rerun_adds_no_second_block() {
    let fixture = Fixture::new();
    succeed(&mut fixture.installer("zsh"));

    let stdout = succeed(&mut fixture.installer("zsh"));

    assert_eq!(read(&fixture.home().join(".zshenv")), POSIX_BLOCK);
    assert!(stdout.contains(&format!("pkg {} is already installed", release())), "{stdout}");
    assert!(stdout.contains("source ~/.zshenv"), "{stdout}");
}

#[test]
fn bash_gets_the_block_in_bashrc_and_profile() {
    let fixture = Fixture::new();
    fs::write(fixture.home().join(".bashrc"), "alias ll='ls -l'").unwrap();

    let stdout = succeed(&mut fixture.installer("bash"));

    assert_eq!(read(&fixture.home().join(".bashrc")), format!("alias ll='ls -l'\n{POSIX_BLOCK}"));
    assert_eq!(read(&fixture.home().join(".profile")), POSIX_BLOCK);
    assert!(!fixture.home().join(".bash_profile").exists());
    assert!(stdout.contains("    source ~/.bashrc\n"), "{stdout}");
}

#[test]
fn bash_gets_the_block_in_an_existing_bash_profile_instead_of_profile() {
    let fixture = Fixture::new();
    fs::write(fixture.home().join(".bash_profile"), "# mine\n").unwrap();

    succeed(&mut fixture.installer("bash"));

    assert_eq!(read(&fixture.home().join(".bashrc")), POSIX_BLOCK);
    assert_eq!(read(&fixture.home().join(".bash_profile")), format!("# mine\n{POSIX_BLOCK}"));
    assert!(!fixture.home().join(".profile").exists());
}

#[test]
fn fish_gets_its_own_file_in_conf_d() {
    let fixture = Fixture::new();

    let stdout = succeed(&mut fixture.installer("fish"));

    assert_eq!(read(&fixture.home().join(".config/fish/conf.d/pkg.fish")), FISH_BLOCK);
    assert!(stdout.contains("    source ~/.config/fish/conf.d/pkg.fish\n"), "{stdout}");
}

#[test]
fn no_modify_path_writes_no_startup_file_and_prints_the_line() {
    let fixture = Fixture::new();

    let stdout = succeed(fixture.installer("zsh").arg("--no-modify-path"));

    let paths: Vec<PathBuf> = snapshot(&fixture.home()).into_keys().collect();
    assert_eq!(paths, [".local", ".local/bin", ".local/bin/pkg"].map(PathBuf::from));
    assert!(stdout.contains("    export PATH=\"$HOME/.local/bin:$PATH\"\n"), "{stdout}");
    assert!(!stdout.contains("source"), "{stdout}");
}

#[test]
fn another_shell_gets_the_line_to_add() {
    let fixture = Fixture::new();

    let stdout = succeed(&mut fixture.installer("ksh"));

    assert_eq!(snapshot(&fixture.home()).len(), 3, "only ~/.local/bin/pkg is new");
    assert!(stdout.contains("    export PATH=\"$HOME/.local/bin:$PATH\"\n"), "{stdout}");
}

#[test]
fn a_path_that_has_local_bin_already_gets_no_block_and_no_source_step() {
    let fixture = Fixture::new();
    let path = format!("{}:{SYSTEM_PATH}", fixture.bin().display());

    let stdout = succeed(fixture.installer("zsh").env("PATH", path));

    assert!(!fixture.home().join(".zshenv").exists());
    assert!(!stdout.contains("source"), "{stdout}");
    assert!(!stdout.contains("export PATH"), "{stdout}");
    assert!(stdout.contains("pkg bootstrap --from"), "{stdout}");
}

#[test]
fn a_dry_run_reports_the_plan_and_changes_nothing() {
    let fixture = Fixture::new();
    fixture.install_fake(&previous());
    fs::write(fixture.home().join(".zshenv"), "export EDITOR=vi\n").unwrap();
    let before = snapshot(&fixture.home());

    let stdout = succeed(fixture.installer("zsh").arg("--dry-run"));

    assert_eq!(snapshot(&fixture.home()), before);
    assert_eq!(fs::read_dir(fixture.tmp()).unwrap().count(), 0, "nothing was downloaded");
    let plan = |target| format!("Would install pkg {} ({target}) at ", release());
    assert!(INSTALLER_TARGETS.into_iter().any(|target| stdout.contains(&plan(target))), "{stdout}");
    let destination = format!("{}, replacing pkg {}", fixture.installed().display(), previous());
    assert!(stdout.contains(&destination), "{stdout}");
    let startup =
        format!("Would add ~/.local/bin to PATH in {}", fixture.home().join(".zshenv").display());
    assert!(stdout.contains(&startup), "{stdout}");
    assert!(stdout.ends_with("Dry run: nothing was downloaded or changed.\n"), "{stdout}");
}

#[test]
fn a_checksum_mismatch_leaves_the_installed_pkg_in_place() {
    let fixture = Fixture::new();
    fixture.install_fake(&previous());
    fixture.tamper(release(), &fake_pkg("tampered"));
    let before = snapshot(&fixture.home());

    let stderr = fail(&mut fixture.installer("zsh"));

    assert!(stderr.contains("does not match its checksum in SHA256SUMS"), "{stderr}");
    assert_eq!(snapshot(&fixture.home()), before);
    assert_eq!(fs::read_dir(fixture.tmp()).unwrap().count(), 0, "the download is cleaned up");
}

#[test]
fn an_archive_holding_unexpected_files_is_not_installed() {
    let fixture = Fixture::new();
    fixture.publish_holding(release(), &["README"]);

    let stderr = fail(&mut fixture.installer("zsh"));

    assert!(stderr.contains("holds files other than pkg-"), "{stderr}");
    assert!(!fixture.home().join(".local").exists());
}

#[test]
fn a_link_mise_manages_is_refused_and_left_alone() {
    let fixture = Fixture::new();
    let shim = fixture.home().join(".local/share/mise/shims/pkg");
    fs::create_dir_all(shim.parent().unwrap()).unwrap();
    write_executable(&shim, &fake_pkg(&previous()));
    fs::create_dir_all(fixture.bin()).unwrap();
    symlink(&shim, fixture.installed()).unwrap();
    let before = snapshot(&fixture.home());

    let stderr = fail(&mut fixture.installer("zsh"));

    assert!(stderr.contains("mise manages pkg"), "{stderr}");
    assert!(stderr.contains("pkg update github:gotpkg/pkg"), "{stderr}");
    assert_eq!(snapshot(&fixture.home()), before);
}

#[test]
fn a_broken_link_is_replaced() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.bin()).unwrap();
    symlink(fixture.home().join("gone"), fixture.installed()).unwrap();

    let stdout = succeed(&mut fixture.installer("zsh"));

    assert!(!fs::symlink_metadata(fixture.installed()).unwrap().is_symlink());
    assert_eq!(read(&fixture.installed()), fake_pkg(release()));
    assert!(stdout.contains("replacing a broken link"), "{stdout}");
}

#[test]
fn something_other_than_pkg_at_the_destination_is_refused() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.bin()).unwrap();
    write_executable(&fixture.installed(), "#!/bin/sh\necho 'hello'\n");
    let before = snapshot(&fixture.home());

    let stderr = fail(&mut fixture.installer("zsh"));

    assert!(stderr.contains("is not pkg"), "{stderr}");
    assert_eq!(snapshot(&fixture.home()), before);
}

#[test]
fn the_version_option_installs_another_release() {
    let fixture = Fixture::new();
    fixture.publish(&previous());

    let stdout = succeed(fixture.installer("zsh").args(["--version", &format!("v{}", previous())]));

    assert_eq!(read(&fixture.installed()), fake_pkg(&previous()));
    assert!(stdout.contains(&format!("Installed pkg {} (", previous())), "{stdout}");
}

#[test]
fn a_copy_from_a_checkout_installs_only_a_named_version() {
    let fixture = Fixture::new();
    let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("../installer/install.sh");

    let stderr = fail(&mut fixture.script(&template, "zsh"));
    assert!(stderr.contains("--version <version>"), "{stderr}");
    assert!(!fixture.home().join(".local").exists());

    succeed(fixture.script(&template, "zsh").args(["--version", release()]));
    assert_eq!(read(&fixture.installed()), fake_pkg(release()));
}

#[test]
fn an_unknown_option_prints_the_usage() {
    let fixture = Fixture::new();

    let output = fixture.installer("zsh").arg("--force").output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with("error: unknown option: --force\n"), "{stderr}");
    assert!(stderr.contains("usage: curl -fsSL https://gotpkg.dev | sh -s -- [options]"));
    assert!(!fixture.home().join(".local").exists());
}
