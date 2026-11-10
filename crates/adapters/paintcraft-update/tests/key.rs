//! Release key provisioning fails closed; known fixture keys never become trust roots.
use paintcraft_update::verify::{PUBLIC_KEY, TEST_ONLY_PUBLIC_KEY, public_key, verify_bytes};
use paintcraft_update::{Arch, Error, InstallKind, ManualReason, Os, Platform, Updater};
mod support;
#[test]
fn unconfigured_release_trust_cannot_start_an_updater() {
    if !PUBLIC_KEY.trim().is_empty() {
        return;
    }
    assert!(matches!(public_key(), Err(Error::SigningKeyUnconfigured)));
    let p = Platform {
        os: Os::Linux,
        arch: Arch::X64,
        install: InstallKind::Manual(ManualReason::Packaged),
    };
    assert!(matches!(
        Updater::new("0.1.0", p),
        Err(Error::SigningKeyUnconfigured)
    ));
}
#[test]
fn fixture_signature_binds_bytes_version_and_filename() {
    let key = minisign_verify::PublicKey::decode(TEST_ONLY_PUBLIC_KEY).unwrap();
    let v = semver::Version::parse("0.1.1").unwrap();
    let name = "paintcraft-0.1.1-linux-x64.AppImage";
    let signature = support::sign_asset(b"payload", "0.1.1", name);
    verify_bytes(&key, b"payload", signature.as_bytes(), &v, name).unwrap();
    assert!(verify_bytes(&key, b"payloaD", signature.as_bytes(), &v, name).is_err());
    assert!(
        verify_bytes(
            &key,
            b"payload",
            signature.as_bytes(),
            &v,
            "paintcraft-0.1.1-linux-arm64.AppImage"
        )
        .is_err()
    );
    assert!(
        verify_bytes(
            &key,
            b"payload",
            signature.as_bytes(),
            &semver::Version::parse("0.1.2").unwrap(),
            name
        )
        .is_err()
    );
}
#[test]
fn configured_release_key_is_not_the_known_test_key() {
    if PUBLIC_KEY.trim().is_empty() {
        return;
    }
    let release = public_key().unwrap();
    let test = minisign_verify::PublicKey::decode(TEST_ONLY_PUBLIC_KEY).unwrap();
    let v = semver::Version::parse("0.1.1").unwrap();
    let name = "paintcraft-0.1.1-linux-x64.AppImage";
    let sig = support::sign_asset(b"payload", "0.1.1", name);
    verify_bytes(&test, b"payload", sig.as_bytes(), &v, name).unwrap();
    assert!(verify_bytes(&release, b"payload", sig.as_bytes(), &v, name).is_err());
}
