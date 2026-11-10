//! Download, signature verification and the AppImage replace, against a local server.

use std::ops::ControlFlow;
use std::path::Path;

use paintcraft_update::{Available, Error, Progress, Updater};

mod support;
use support::server::{Reply, Server};
use support::{REPO, linux_platform, publish_appimage, sign_asset, tempdir, updater, write};

const PAYLOAD: &[u8] = b"fake appimage payload, not an executable format";

struct Fixture {
    server: Server,
    dir: tempfile::TempDir,
    image: std::path::PathBuf,
}

fn fixture() -> Fixture {
    let server = Server::start();
    let dir = tempdir();
    let image = write(dir.path().join("PaintCraft.AppImage"), b"old image");
    Fixture { server, dir, image }
}

impl Fixture {
    fn updater(&self) -> Updater {
        updater("0.1.0", linux_platform(&self.image), &self.server.base)
    }

    fn available(&self) -> Available {
        self.updater().check().unwrap().unwrap()
    }

    fn leftovers(&self) -> Vec<String> {
        std::fs::read_dir(self.dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n != "PaintCraft.AppImage")
            .collect()
    }
}

fn go(_: Progress) -> ControlFlow<()> {
    ControlFlow::Continue(())
}

fn asset_route(version: &str) -> String {
    format!("/{REPO}/releases/download/v{version}/paintcraft-{version}-linux-x64.AppImage")
}

#[test]
fn a_correctly_signed_download_verifies_and_reports_progress() {
    let f = fixture();
    publish_appimage(&f.server, "0.1.1", PAYLOAD);
    let available = f.available();
    let mut seen = Vec::new();
    let verified = f
        .updater()
        .download(&available, &mut |p| {
            seen.push(p);
            ControlFlow::Continue(())
        })
        .unwrap();
    assert_eq!(std::fs::read(verified.payload()).unwrap(), PAYLOAD);
    assert_eq!(
        seen.last().unwrap(),
        &Progress {
            received: PAYLOAD.len() as u64,
            total: PAYLOAD.len() as u64
        }
    );
    assert!(seen.windows(2).all(|w| w[0].received < w[1].received));
    // Staged beside the AppImage (same filesystem) in a private directory.
    assert_eq!(verified.staging_dir().parent().unwrap(), f.dir.path());
    assert!(
        verified
            .staging_dir()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".paintcraft-update-")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(verified.staging_dir())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }
    let requests: Vec<_> = f.server.requests().into_iter().map(|r| r.path).collect();
    assert_eq!(
        requests.len(),
        3,
        "check, signature, asset only: {requests:?}"
    );
    drop(verified);
    assert!(f.leftovers().is_empty(), "staging removed on drop");
}

#[test]
#[cfg(unix)]
fn apply_replaces_appimage_keeps_mode_and_retains_original_backup() {
    let f = fixture();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&f.image, std::fs::Permissions::from_mode(0o750)).unwrap();
    }
    publish_appimage(&f.server, "0.1.1", PAYLOAD);
    let updater = f.updater();
    let verified = updater.download(&f.available(), &mut go).unwrap();
    let locks = support::tempdir();
    let mut running =
        paintcraft_update::InstallLock::for_test(updater.platform(), locks.path()).unwrap();
    let relaunch = updater.apply(verified, &mut running).unwrap();
    assert!(running.requires_exit());
    assert_eq!(relaunch.version.to_string(), "0.1.1");
    assert_eq!(relaunch.launch, f.image);
    assert_eq!(std::fs::read(&f.image).unwrap(), PAYLOAD);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&f.image).unwrap().permissions().mode() & 0o777,
            0o750
        );
    }
    let leftovers = f.leftovers();
    assert_eq!(leftovers.len(), 1, "only original image backup remains");
    assert!(leftovers[0].starts_with("PaintCraft.AppImage.old-"));
    assert_eq!(
        std::fs::read(f.dir.path().join(&leftovers[0])).unwrap(),
        b"old image"
    );
}

#[test]
fn a_signature_by_another_key_is_rejected_and_nothing_is_left_behind() {
    let f = fixture();
    let name = publish_appimage(&f.server, "0.1.1", PAYLOAD);
    let other = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
    let forged = support::sign_with(
        &other,
        support::KEY_ID,
        PAYLOAD,
        &format!("paintcraft-update 0.1.1 {name}"),
    );
    f.server.route(
        &format!("{}.minisig", asset_route("0.1.1")),
        Reply::ok(forged),
    );
    let result = f.updater().download(&f.available(), &mut go);
    assert!(
        matches!(result, Err(Error::SignatureInvalid(_))),
        "{result:?}"
    );
    assert!(f.leftovers().is_empty());
    assert_eq!(std::fs::read(&f.image).unwrap(), b"old image");
}

#[test]
fn a_signature_naming_another_key_id_is_rejected() {
    let f = fixture();
    let name = publish_appimage(&f.server, "0.1.1", PAYLOAD);
    let forged = support::sign_with(
        &support::signing_key(),
        *b"OTHERKEY",
        PAYLOAD,
        &format!("paintcraft-update 0.1.1 {name}"),
    );
    f.server.route(
        &format!("{}.minisig", asset_route("0.1.1")),
        Reply::ok(forged),
    );
    let result = f.updater().download(&f.available(), &mut go);
    assert!(
        matches!(result, Err(Error::SignatureInvalid(_))),
        "{result:?}"
    );
}

#[test]
fn a_replayed_signature_for_another_version_or_asset_is_rejected() {
    let f = fixture();
    let name = publish_appimage(&f.server, "0.1.1", PAYLOAD);
    let sig_url = format!("{}.minisig", asset_route("0.1.1"));
    for comment in [
        format!("paintcraft-update 0.1.0 {name}"),
        format!("paintcraft-update 0.1.2 {name}"),
        "paintcraft-update 0.1.1 paintcraft-0.1.1-macos-arm64-update.zip".to_owned(),
        format!("paintcraft-update 0.1.1  {name}"),
        name.clone(),
    ] {
        // Properly signed, by the real test key, over the real payload, but for something else.
        let replay = support::sign(PAYLOAD, &comment);
        f.server.route(&sig_url, Reply::ok(replay.clone()));
        // Keep the release document in step with the new signature's length.
        let json = support::release_json(
            &f.server.base,
            "v0.1.1",
            false,
            &[
                (&name, PAYLOAD.len()),
                (&format!("{name}.minisig"), replay.len()),
            ],
        );
        f.server
            .route(&format!("/repos/{REPO}/releases/latest"), Reply::ok(json));
        let result = f.updater().download(&f.available(), &mut go);
        assert!(
            matches!(result, Err(Error::TrustedCommentMismatch { .. })),
            "{comment}: {result:?}"
        );
    }
    assert!(f.leftovers().is_empty());
}

#[test]
fn a_tampered_payload_fails_verification() {
    let f = fixture();
    publish_appimage(&f.server, "0.1.1", PAYLOAD);
    let mut evil = PAYLOAD.to_vec();
    evil[3] ^= 0x01;
    f.server.route(&asset_route("0.1.1"), Reply::ok(evil));
    let result = f.updater().download(&f.available(), &mut go);
    assert!(
        matches!(result, Err(Error::SignatureInvalid(_))),
        "{result:?}"
    );
    assert!(f.leftovers().is_empty());
}

#[test]
fn a_missing_signature_file_stops_the_update() {
    let f = fixture();
    publish_appimage(&f.server, "0.1.1", PAYLOAD);
    f.server.route(
        &format!("{}.minisig", asset_route("0.1.1")),
        Reply::status(404),
    );
    let result = f.updater().download(&f.available(), &mut go);
    assert!(matches!(result, Err(Error::HttpStatus(404))), "{result:?}");
    assert!(f.leftovers().is_empty());
    // A garbage signature is no better.
    f.server.route(
        &format!("{}.minisig", asset_route("0.1.1")),
        Reply::ok("x".repeat(300)),
    );
    assert!(f.updater().download(&f.available(), &mut go).is_err());
}

#[test]
fn the_size_must_match_the_release_listing_exactly() {
    let f = fixture();
    publish_appimage(&f.server, "0.1.1", PAYLOAD);
    let available = f.available();
    // Shorter than listed.
    f.server
        .route(&asset_route("0.1.1"), Reply::ok(&PAYLOAD[..10]));
    let result = f.updater().download(&available, &mut go);
    assert!(
        matches!(result, Err(Error::SizeMismatch { expected, received: 10 }) if expected == PAYLOAD.len() as u64),
        "{result:?}"
    );
    // Longer than listed: cut off at the listed size.
    let mut longer = PAYLOAD.to_vec();
    longer.extend_from_slice(b"extra");
    f.server.route(&asset_route("0.1.1"), Reply::ok(longer));
    assert!(matches!(
        f.updater().download(&available, &mut go),
        Err(Error::ResponseTooLarge { .. })
    ));
    // Announced longer than listed, even if the body is short.
    let mut announced = Reply::ok(PAYLOAD);
    announced.announce = Some(PAYLOAD.len() + 1);
    f.server.route(&asset_route("0.1.1"), announced);
    assert!(matches!(
        f.updater().download(&available, &mut go),
        Err(Error::ResponseTooLarge { .. })
    ));
    assert!(f.leftovers().is_empty());
}

#[test]
fn the_caller_can_cancel() {
    let f = fixture();
    publish_appimage(&f.server, "0.1.1", PAYLOAD);
    let result = f
        .updater()
        .download(&f.available(), &mut |_| ControlFlow::Break(()));
    assert!(matches!(result, Err(Error::Cancelled)), "{result:?}");
    assert!(f.leftovers().is_empty());
}

#[test]
fn a_download_for_a_version_that_is_not_newer_is_refused() {
    let f = fixture();
    publish_appimage(&f.server, "0.1.1", PAYLOAD);
    let available = f.available();
    let newer = updater("0.1.1", linux_platform(&f.image), &f.server.base);
    assert!(matches!(
        newer.download(&available, &mut go),
        Err(Error::NotNewer { .. })
    ));
    assert_eq!(f.server.requests().len(), 1);
}

#[test]
fn an_unwritable_install_directory_is_reported_before_downloading() {
    let f = fixture();
    publish_appimage(&f.server, "0.1.1", PAYLOAD);
    let missing = Path::new("/definitely/not/here/PaintCraft.AppImage");
    let u = updater("0.1.0", linux_platform(missing), &f.server.base);
    let available = u.check().unwrap().unwrap();
    assert!(matches!(
        u.download(&available, &mut go),
        Err(Error::ParentNotWritable(_))
    ));
}

#[test]
fn leftover_staging_directories_are_cleaned_when_stale_only() {
    let f = fixture();
    let stale = f.dir.path().join(".paintcraft-update-stale");
    let fresh = f.dir.path().join(".paintcraft-update-fresh");
    let unrelated = f.dir.path().join("keep-me");
    for dir in [&stale, &fresh, &unrelated] {
        std::fs::create_dir(dir).unwrap();
    }
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3 * 86_400);
    std::fs::File::open(&stale)
        .unwrap()
        .set_modified(old)
        .unwrap();
    paintcraft_update::cleanup_leftovers(&linux_platform(&f.image));
    assert!(!stale.exists() && fresh.exists() && unrelated.exists());
    let _ = sign_asset;
}
