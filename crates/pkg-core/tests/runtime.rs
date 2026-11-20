#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;

use pkg_core::{
    Client, ErrorKind, InstallRequest, ListRequest, LocateRequest, Mode, OutputStream, Progress,
    PruneRequest, Settings,
};
use tempfile::TempDir;

fn settings(root: &TempDir, name: &str, script: &str) -> Settings {
    let home = root.path().join(name);
    fs::create_dir_all(&home).unwrap();
    let executable = home.join("mise");
    fs::write(&executable, format!("#!/bin/sh\nshift 2\n{script}")).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    Settings::new(home, executable.into_os_string())
}

#[test]
fn contexts_keep_their_child_environment_and_reread_current_state() {
    let root = TempDir::new().unwrap();
    let parent_cwd = std::env::current_dir().unwrap();
    let parent_home = std::env::var_os("HOME");
    for name in ["one", "space in home", "λ"] {
        let mut configuration = settings(
            &root,
            name,
            r#"
case "$*" in
    'backends ls') printf 'npm';;
    'config ls --json') printf '[]';;
    'where ripgrep') printf '%s/%s' "$HOME" "$TOKEN";;
    'bootstrap packages status --json')
        if [ -f "$HOME/changed" ]; then
            printf '{"brew":{"available":true,"packages":[{"package":"wget","state":"installed","installed_version":"2"}]}}'
        else
            printf '{}'
        fi;;
    'ls --global --json') printf '{}';;
esac
"#,
        );
        configuration.child_env.insert("TOKEN".into(), name.into());
        configuration.child_env.insert("HOME".into(), "/wrong-home".into());
        let home = configuration.home.clone();
        assert!(!format!("{configuration:?}").contains("wrong-home"));
        let mut client = Client::new(configuration).unwrap();
        let location =
            client.locate(LocateRequest { package: "ripgrep".into() }, &mut |_| {}).unwrap();
        assert_eq!(location.result.path, home.join(name));
        assert!(client.list(ListRequest::default(), &mut |_| {}).unwrap().result.items.is_empty());
        fs::write(home.join("changed"), "").unwrap();
        let listed = client.list(ListRequest::default(), &mut |_| {}).unwrap();
        assert_eq!(listed.result.items[0].package, "brew:wget");
        assert_eq!(listed.result.items[0].installed.as_deref(), Some("2"));
    }
    assert_eq!(std::env::current_dir().unwrap(), parent_cwd);
    assert_eq!(std::env::var_os("HOME"), parent_home);
}

#[test]
fn progress_drains_both_pipes_on_the_calling_thread_and_finishes_before_return() {
    let root = TempDir::new().unwrap();
    let configuration = settings(
        &root,
        "progress",
        r#"
case "$*" in
    'backends ls') printf 'npm';;
    'config ls --json') printf '[]';;
    'bootstrap packages prune --dry-run --manager brew') printf 'remove brew:unused@1\n';;
    'bootstrap packages prune --yes --manager brew')
        index=0
        while [ "$index" -lt 10000 ]; do
            printf 'out:λ\n'
            printf 'err:世\n' >&2
            index=$((index + 1))
        done
        printf done > "$HOME/finished";;
esac
"#,
    );
    let finished = configuration.home.join("finished");
    let mut client = Client::new(configuration).unwrap();
    let calling_thread = std::thread::current().id();
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut callback = |event: Progress<'_>| {
        assert_eq!(std::thread::current().id(), calling_thread);
        if let Progress::Output { stream, text } = event {
            match stream {
                OutputStream::Stdout => stdout.push_str(text),
                OutputStream::Stderr => stderr.push_str(text),
            }
        }
    };
    let outcome =
        client.prune(PruneRequest { mode: Mode::Apply, force: false }, &mut callback).unwrap();
    assert_eq!(stdout, "out:λ\n".repeat(10000));
    assert_eq!(stderr, "err:世\n".repeat(10000));
    assert_eq!(fs::read_to_string(finished).unwrap(), "done");
    assert_eq!(outcome.result.uninstalled, ["brew:unused@1"]);
}

#[test]
fn invalid_library_requests_fail_before_mise_or_files_are_touched() {
    let root = TempDir::new().unwrap();
    let configuration =
        Settings::new(root.path().to_path_buf(), root.path().join("absent-mise").into_os_string());
    let mut client = Client::new(configuration).unwrap();
    for packages in [vec![], vec!["brew:".into()], vec!["not a name".into()]] {
        let failure = client
            .install(
                InstallRequest { packages, mode: Mode::Apply, report_plans: false },
                &mut |_| panic!("invalid request emitted progress"),
            )
            .err()
            .unwrap();
        assert_eq!(failure.kind, ErrorKind::InvalidArgument);
    }
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}
