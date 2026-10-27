//! Installation layout and sealed-pin controls, not codec execution/signing proof.
use super::*;
use crate::codec_helper::{decode::verify_decoder, encode::verify_encoder, mixed::verify_mixed};
const ENCODER_MARKER: &[u8] = b"TEST_BUNDLE_ENCODER\0";
const ENCODER_RECORD: &[u8] = b"TEST_BUNDLE_ENCODER\0exact-profile\0";
fn bundle(target: Target) -> Installation {
    let mut installation = Installation::new(target, false);
    let contents = installation.root.join("PaintCraft Editor.app/Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    fs::create_dir_all(&macos).unwrap();
    fs::create_dir_all(&resources).unwrap();
    let app = macos.join("paintcraft-engine");
    let helper = macos.join(target.executable());
    let manifest = resources.join(MANIFEST_NAME);
    fs::rename(&installation.app, &app).unwrap();
    fs::rename(&installation.helper, &helper).unwrap();
    fs::rename(&installation.manifest, &manifest).unwrap();
    installation.app = app;
    installation.helper = helper;
    installation.manifest = manifest;
    installation
}
fn encoder(installation: &Installation) -> EncoderContract<'static> {
    let contract = EncoderContract {
        decode: Contract {
            profile: "test-bundle-schema3",
            build_marker: ENCODER_MARKER,
            build_record: ENCODER_RECORD,
            ..contract()
        },
        encode_protocol_version: 1,
        qualified_export_variants: &["test-export"],
    };
    let mut bytes = binary(installation.target);
    bytes.truncate(bytes.len() - RECORD.len());
    bytes.extend_from_slice(ENCODER_RECORD);
    fs::write(&installation.helper, bytes).unwrap();
    let mut value = installation.metadata();
    let Value::Object(fields) = &mut value else {
        unreachable!()
    };
    fields.insert("schema_version".into(), Value::Int(3));
    fields.insert("profile".into(), Value::str(contract.decode.profile));
    fields.insert("encode_protocol_version".into(), Value::Int(1));
    fields.insert(
        "qualified_export_variants".into(),
        Value::Array(vec![Value::str("test-export")]),
    );
    fs::write(&installation.manifest, json::canonical(&value)).unwrap();
    contract
}
#[test]
fn mac_bundle_schema2_uses_resources_and_preserves_retained_identity() {
    for target in [Target::MacArm, Target::MacX64] {
        let installation = bundle(target);
        let verified = installation.verify().unwrap();
        assert_eq!(verified.manifest, installation.manifest);
        assert_eq!(
            verified.checked_executable(&|| Ok(())).unwrap(),
            installation.helper
        );
        assert_eq!(
            verified.sha256(),
            <[u8; 32]>::from(Sha256::digest(fs::read(&installation.helper).unwrap()))
        );
        let original = fs::read(&installation.manifest).unwrap();
        // An identical replacement still violates the acquired FileIdentity.
        let renamed = installation.root.join("original.json");
        fs::rename(&installation.manifest, renamed).unwrap();
        fs::write(&installation.manifest, original).unwrap();
        assert_eq!(
            verified.checked_executable(&|| Ok(())).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
    }
}
#[test]
fn bundled_schema3_decode_encode_and_mixed_selection_share_the_same_resolver() {
    for target in [Target::MacArm, Target::MacX64] {
        let installation = bundle(target);
        let current = encoder(&installation);
        let decode = verify_decoder(&installation.app, current, target, &|| Ok(())).unwrap();
        let encode = verify_encoder(&installation.app, current, target, &|| Ok(())).unwrap();
        let mixed = verify_mixed(
            &installation.app,
            &[contract()],
            current,
            target,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(
            decode.checked_executable(&|| Ok(())).unwrap(),
            installation.helper
        );
        assert_eq!(
            encode.checked_executable(&|| Ok(())).unwrap(),
            installation.helper
        );
        assert_eq!(
            mixed.checked_executable(&|| Ok(())).unwrap(),
            installation.helper
        );
        let mut changed = fs::read(&installation.manifest).unwrap();
        let at = changed.windows(5).position(|v| v == b"0.1.0").unwrap();
        changed[at + 4] = b'1';
        fs::write(&installation.manifest, changed).unwrap();
        assert_eq!(
            decode.checked_executable(&|| Ok(())).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        assert_eq!(
            encode.checked_executable(&|| Ok(())).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
        assert_eq!(
            mixed.checked_executable(&|| Ok(())).unwrap_err().code,
            ErrorCode::ConflictStaleBase
        );
    }
}
#[test]
fn bundle_missing_corrupt_stale_and_mixed_manifests_never_fall_back() {
    let installation = bundle(Target::MacArm);
    let adjacent = installation.app.parent().unwrap().join(MANIFEST_NAME);
    let original = fs::read(&installation.manifest).unwrap();
    fs::remove_file(&installation.manifest).unwrap();
    assert!(installation.verify().is_err());
    fs::write(&adjacent, &original).unwrap();
    assert!(
        installation
            .verify()
            .unwrap_err()
            .message
            .contains("Resources is required")
    );
    fs::write(&installation.manifest, b"not JSON").unwrap();
    assert!(
        installation
            .verify()
            .unwrap_err()
            .message
            .contains("Resources is required")
    );
    fs::remove_file(&adjacent).unwrap();
    assert!(
        installation
            .verify()
            .unwrap_err()
            .message
            .contains("strict JSON")
    );
    let mut stale = installation.metadata();
    let Value::Object(fields) = &mut stale else {
        unreachable!()
    };
    fields.insert("application_version".into(), Value::str("0.0.0"));
    fs::write(&installation.manifest, json::canonical(&stale)).unwrap();
    assert!(
        installation
            .verify()
            .unwrap_err()
            .message
            .contains("host contract")
    );
    fs::write(&installation.manifest, &original).unwrap();
    let verified = installation.verify().unwrap();
    fs::write(&adjacent, &original).unwrap();
    assert_eq!(
        verified
            .checked_executable(&|| Err(Error::new(ErrorCode::OperationCancelled, "cancel")))
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled,
    );
    assert_eq!(
        verified.checked_executable(&|| Ok(())).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
    assert!(installation.verify().is_err());
    fs::remove_file(adjacent).unwrap();
    fs::write(&installation.manifest, vec![b' '; 8193]).unwrap();
    assert_eq!(
        installation.verify().unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}
#[cfg(unix)]
#[test]
fn bundle_resources_manifest_and_adjacent_symlinks_are_refused() {
    use std::os::unix::fs::symlink;
    let installation = bundle(Target::MacArm);
    let external = installation.root.join("external.json");
    fs::rename(&installation.manifest, &external).unwrap();
    symlink(&external, &installation.manifest).unwrap();
    assert!(
        installation
            .verify()
            .unwrap_err()
            .message
            .contains("Symlinked")
    );
    fs::remove_file(&installation.manifest).unwrap();
    fs::rename(&external, &installation.manifest).unwrap();
    let resources = installation.manifest.parent().unwrap();
    let external_directory = installation.root.join("external-resources");
    fs::rename(resources, &external_directory).unwrap();
    symlink(&external_directory, resources).unwrap();
    assert!(
        installation
            .verify()
            .unwrap_err()
            .message
            .contains("Symlinked")
    );
    fs::remove_file(resources).unwrap();
    fs::rename(&external_directory, resources).unwrap();
    let adjacent = installation.app.parent().unwrap().join(MANIFEST_NAME);
    symlink(installation.root.join("missing"), adjacent).unwrap();
    assert!(
        installation
            .verify()
            .unwrap_err()
            .message
            .contains("Resources is required")
    );
}
#[test]
fn bundle_selection_is_mac_only_exact_shape_and_flat_installs_stay_unchanged() {
    for target in [
        Target::MacArm,
        Target::MacX64,
        Target::WinArm,
        Target::WinX64,
        Target::LinuxArm,
        Target::LinuxX64,
    ] {
        let installation = Installation::new(target, false);
        assert_eq!(
            installation.verify().unwrap().manifest,
            installation.manifest
        );
    }
    for target in [Target::LinuxArm, Target::LinuxX64] {
        let installation = Installation::new(target, true);
        assert_eq!(
            installation.verify().unwrap().manifest,
            installation.manifest
        );
        // Retain Linux adjacent selection precedence, without retry on corrupt bytes.
        let adjacent = installation.app.parent().unwrap().join(MANIFEST_NAME);
        fs::write(adjacent, b"not JSON").unwrap();
        assert!(
            installation
                .verify()
                .unwrap_err()
                .message
                .contains("strict JSON")
        );
    }
    for target in [Target::WinX64, Target::LinuxX64] {
        let installation = bundle(target);
        // A non-Mac executable with Mac-shaped parents keeps flat lookup.
        assert!(installation.verify().is_err());
        let adjacent = installation.app.parent().unwrap().join(MANIFEST_NAME);
        fs::copy(&installation.manifest, adjacent).unwrap();
        installation.verify().unwrap();
    }
    let installation = bundle(Target::MacArm);
    let old = installation.root.join("PaintCraft Editor.app");
    let renamed = installation.root.join("PaintCraft Editor.appx");
    fs::rename(old, &renamed).unwrap();
    let app = renamed.join("Contents/MacOS/paintcraft-engine");
    assert!(verify(&app, contract(), Target::MacArm, &|| Ok(())).is_err());
    fs::copy(
        renamed.join("Contents/Resources").join(MANIFEST_NAME),
        app.parent().unwrap().join(MANIFEST_NAME),
    )
    .unwrap();
    verify(&app, contract(), Target::MacArm, &|| Ok(())).unwrap();
}
