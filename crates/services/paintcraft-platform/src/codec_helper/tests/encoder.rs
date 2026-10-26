//! Schema/negative installation controls, not execution/qualification of an encoder.
use super::*;
use crate::codec_helper::encode::{EncoderContract, verify_encoder};
const IMPORTS: &[&str] = &["one", "two", "three", "four", "five", "six", "seven"];
const ENCODE_MARKER: &[u8] = b"PAINTCRAFT_CODEC_WORKER_BUILD_TEST_ENCODER\0";
const ENCODE_RECORD: &[u8] =
    b"PAINTCRAFT_CODEC_WORKER_BUILD_TEST_ENCODER\0exact-test-encoder-contract\0";
fn encoder_contract() -> EncoderContract<'static> {
    EncoderContract {
        decode: Contract {
            qualified_import_variants: IMPORTS,
            build_marker: ENCODE_MARKER,
            build_record: ENCODE_RECORD,
            ..contract()
        },
        encode_protocol_version: 1,
        qualified_export_variants: &["test-export"],
    }
}
fn set(value: &mut Value, key: &str, field: Value) {
    let Value::Object(fields) = value else {
        unreachable!()
    };
    fields.insert(key.into(), field);
}
fn array(values: &[&str]) -> Value {
    Value::Array(values.iter().map(|s| Value::str(*s)).collect())
}
fn manifest(installation: &Installation) -> Value {
    let mut value = installation.metadata();
    set(&mut value, "schema_version", Value::Int(3));
    set(&mut value, "qualified_import_variants", array(IMPORTS));
    set(&mut value, "encode_protocol_version", Value::Int(1));
    set(
        &mut value,
        "qualified_export_variants",
        array(&["test-export"]),
    );
    value
}
fn write(installation: &Installation, value: &Value) {
    fs::write(&installation.manifest, json::canonical(value)).unwrap();
}
#[test]
fn exact_schema3_and_separate_capability_refuse_legacy_forgery_and_changed_fields() {
    let installation = Installation::new(Target::MacArm, false);
    let contract = encoder_contract();
    // Existing schema2 decoder metadata cannot produce an encoder capability.
    assert!(verify_encoder(&installation.app, contract, installation.target, &|| Ok(())).is_err());
    let valid_shape = manifest(&installation);
    for (field, changed) in [
        ("schema_version", Value::Int(2)),
        ("encode_protocol_version", Value::Int(2)),
        ("qualified_export_variants", array(&[])),
        ("qualified_export_variants", array(&["unknown-export"])),
        (
            "qualified_export_variants",
            array(&["test-export", "test-export"]),
        ),
        ("qualified_import_variants", array(&["one"])),
        ("profile", Value::str("unknown-private-profile")),
    ] {
        let mut value = valid_shape.clone();
        set(&mut value, field, changed);
        write(&installation, &value);
        let error = verify_encoder(&installation.app, contract, installation.target, &|| Ok(()))
            .unwrap_err();
        assert!(
            error.message.contains(
                if matches!(field, "qualified_import_variants" | "profile") {
                    "host contract"
                } else {
                    "schema3"
                }
            ),
            "{field}: {}",
            error.message
        );
    }
    let mut extra = valid_shape.clone();
    set(&mut extra, "unknown", Value::Bool(true));
    write(&installation, &extra);
    assert!(verify_encoder(&installation.app, contract, installation.target, &|| Ok(())).is_err());
    // A schema3 label and a matching file SHA cannot turn a decoder into an encoder.
    write(&installation, &valid_shape);
    let error =
        verify_encoder(&installation.app, contract, installation.target, &|| Ok(())).unwrap_err();
    assert!(error.message.contains("build record"), "{}", error.message);
    assert!(
        installation.verify().is_err(),
        "schema3 remains refused by schema2 discovery"
    );
    fs::write(&installation.manifest, [b' '; 8193]).unwrap();
    assert!(verify_encoder(&installation.app, contract, installation.target, &|| Ok(())).is_err());
}
