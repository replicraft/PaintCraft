//! Synthetic architecture/descriptor verification, not decoder execution proof.
use super::*;
use crate::codec_helper::mixed::verify_mixed;
const IMPORTS: &[&str] = &[
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "writer-read",
];
const CURRENT_MARKER: &[u8] = b"PAINTCRAFT_CODEC_WORKER_BUILD_TEST_SCHEMA3\0";
const CURRENT_RECORD: &[u8] = b"PAINTCRAFT_CODEC_WORKER_BUILD_TEST_SCHEMA3\0exact-mode-record\0";
fn current() -> EncoderContract<'static> {
    EncoderContract {
        decode: Contract {
            profile: "known-final-schema3",
            qualified_import_variants: IMPORTS,
            build_marker: CURRENT_MARKER,
            build_record: CURRENT_RECORD,
            ..contract()
        },
        encode_protocol_version: 1,
        qualified_export_variants: &["one-export"],
    }
}
fn set(value: &mut Value, key: &str, field: Value) {
    let Value::Object(fields) = value else {
        unreachable!()
    };
    fields.insert(key.into(), field);
}
fn array(values: &[&str]) -> Value {
    Value::Array(values.iter().map(|value| Value::str(*value)).collect())
}
fn current_manifest(f: &Installation) -> Value {
    let mut bytes = binary(f.target);
    bytes.truncate(bytes.len() - RECORD.len());
    bytes.extend_from_slice(CURRENT_RECORD);
    fs::write(&f.helper, bytes).unwrap();
    let mut value = f.metadata();
    set(&mut value, "schema_version", Value::Int(3));
    set(&mut value, "profile", Value::str(current().decode.profile));
    set(&mut value, "qualified_import_variants", array(IMPORTS));
    set(&mut value, "encode_protocol_version", Value::Int(1));
    set(
        &mut value,
        "qualified_export_variants",
        array(&["one-export"]),
    );
    value
}
fn write(f: &Installation, value: &Value) {
    fs::write(&f.manifest, json::canonical(value)).unwrap();
}
#[test]
fn exact_legacy_and_schema3_contracts_select_upfront_for_all_target_headers() {
    for target in [
        Target::MacArm,
        Target::MacX64,
        Target::LinuxArm,
        Target::LinuxX64,
        Target::WinArm,
        Target::WinX64,
    ] {
        let f = Installation::new(target, target.linux());
        let old = verify_mixed(&f.app, &[contract()], current(), target, &|| Ok(())).unwrap();
        assert_eq!(old.checked_executable(&|| Ok(())).unwrap(), f.helper);
        let value = current_manifest(&f);
        write(&f, &value);
        let new = verify_mixed(&f.app, &[contract()], current(), target, &|| Ok(())).unwrap();
        assert_eq!(new.checked_executable(&|| Ok(())).unwrap(), f.helper);
        assert_eq!(
            new.sha256(),
            <[u8; 32]>::from(Sha256::digest(fs::read(&f.helper).unwrap()))
        );
        assert!(
            f.verify().is_err(),
            "old schema2 discovery never accepts schema3"
        );
        assert!(
            old.checked_executable(&|| Ok(())).is_err(),
            "old retained pins detect replacement"
        );
    }
}
#[test]
fn unknown_private_mixed_order_integrity_and_invalid_host_contracts_never_retry() {
    let f = Installation::new(Target::MacArm, false);
    let exact = current_manifest(&f);
    for (key, changed) in [
        (
            "profile",
            Value::str("private-native-v8-jxr-rgb8-lossless-seekcap2-jxrread1-parent-lease1"),
        ),
        ("schema_version", Value::Int(2)),
        (
            "qualified_import_variants",
            array(&[
                "two",
                "one",
                "three",
                "four",
                "five",
                "six",
                "seven",
                "writer-read",
            ]),
        ),
        (
            "qualified_import_variants",
            array(&[
                "one", "two", "three", "four", "five", "six", "seven", "unknown",
            ]),
        ),
        ("qualified_export_variants", array(&["unknown-export"])),
        ("encode_protocol_version", Value::Int(2)),
    ] {
        let mut value = exact.clone();
        set(&mut value, key, changed);
        write(&f, &value);
        assert_eq!(
            verify_mixed(&f.app, &[contract()], current(), f.target, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported,
            "{key}"
        );
    }
    let mut extra = exact.clone();
    set(&mut extra, "unknown", Value::Bool(false));
    write(&f, &extra);
    assert!(verify_mixed(&f.app, &[contract()], current(), f.target, &|| Ok(())).is_err());
    let mut bytes = fs::read(&f.helper).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(&f.helper, &bytes).unwrap();
    let mut same_sha_forgery = exact;
    set(
        &mut same_sha_forgery,
        "sha256",
        Value::str(hex(&Sha256::digest(&bytes))),
    );
    write(&f, &same_sha_forgery);
    assert_eq!(
        verify_mixed(&f.app, &[contract()], current(), f.target, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    for legacy in [
        vec![contract(); 5],
        vec![contract(); 2],
        vec![current().decode],
    ] {
        assert_eq!(
            verify_mixed(&f.app, &legacy, current(), f.target, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    let foreign = Contract {
        application_version: "another-release",
        ..contract()
    };
    assert_eq!(
        verify_mixed(&f.app, &[foreign], current(), f.target, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert_eq!(
        verify_mixed(&f.app, &[contract()], current(), f.target, &|| Err(
            Error::new(ErrorCode::OperationCancelled, "cancel")
        ))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
}
