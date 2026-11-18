use pkg_core::{Client, ErrorKind, InfoRequest, Mode, Settings, TapAddRequest, TapRemoveRequest};

#[test]
fn request_values_cannot_become_mise_options() {
    let root = tempfile::tempdir().unwrap();
    let settings =
        Settings::new(root.path().into(), root.path().join("missing-mise").into_os_string());
    let mut client = Client::new(settings).unwrap();
    let failure = client.info(InfoRequest { package: "--help".into() }, &mut |_| {}).err().unwrap();
    assert_eq!(failure.kind, ErrorKind::InvalidArgument);
    for (tap, url) in [("--help", "https://github.com/example/tap"), ("example/tap", "--dry-run")] {
        let failure = client
            .tap_add(
                TapAddRequest { tap: tap.into(), url: url.into(), mode: Mode::Apply },
                &mut |_| {},
            )
            .err()
            .unwrap();
        assert_eq!(failure.kind, ErrorKind::InvalidArgument);
    }
    let failure = client
        .tap_remove(TapRemoveRequest { tap: "--force".into(), mode: Mode::Apply }, &mut |_| {})
        .err()
        .unwrap();
    assert_eq!(failure.kind, ErrorKind::InvalidArgument);
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
