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

#[test]
fn schema3_contract_enforces_host_selected_eight_imports_without_assuming_seven() {
    const EIGHT: &[&str] = &[
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "writer-read",
    ];
    let installation = Installation::new(Target::MacArm, false);
    let mut contract = encoder_contract();
    contract.decode.qualified_import_variants = EIGHT;
    contract.validate().unwrap();
    let mut value = manifest(&installation);
    set(&mut value, "qualified_import_variants", array(EIGHT));
    let selected = contract.select(&value).unwrap();
    assert_eq!(selected.qualified_import_variants, EIGHT);
    // Full import/profile/build verification still occurs in verify_selected;
    // this schema selector alone never creates either sealed capability.
    write(&installation, &value);
    assert!(verify_encoder(&installation.app, contract, installation.target, &|| Ok(())).is_err());
    contract.qualified_export_variants = &["same", "same"];
    assert!(contract.validate().is_err());
    contract.qualified_export_variants = &[];
    assert!(contract.validate().is_err());
}

#[test]
fn sealed_schema3_decoder_checks_full_installation_and_exact_eighth_variant() {
    use crate::codec_helper::decode::verify_decoder;
    const EIGHT: &[&str] = &[
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "writer-read",
    ];
    let installation = Installation::new(Target::MacArm, false);
    let mut contract = encoder_contract();
    contract.decode.qualified_import_variants = EIGHT;
    let mut helper = binary(installation.target);
    helper.truncate(helper.len() - RECORD.len());
    helper.extend_from_slice(ENCODE_RECORD);
    fs::write(&installation.helper, &helper).unwrap();
    let mut exact = manifest(&installation);
    set(&mut exact, "qualified_import_variants", array(EIGHT));
    write(&installation, &exact);
    let verified =
        verify_decoder(&installation.app, contract, installation.target, &|| Ok(())).unwrap();
    assert_eq!(
        verified.checked_executable(&|| Ok(())).unwrap(),
        installation.helper
    );
    assert_eq!(verified.sha256(), <[u8; 32]>::from(Sha256::digest(&helper)));
    assert!(
        installation.verify().is_err(),
        "schema2 discovery must refuse schema3"
    );
    for (key, changed) in [
        (
            "qualified_import_variants",
            array(&[
                "one", "two", "three", "four", "five", "six", "seven", "unknown",
            ]),
        ),
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
        ("profile", Value::str("another-private-profile")),
        ("qualified_export_variants", array(&["another-export"])),
        ("schema_version", Value::Int(2)),
    ] {
        let mut changed_manifest = exact.clone();
        set(&mut changed_manifest, key, changed);
        write(&installation, &changed_manifest);
        assert_eq!(
            verify_decoder(&installation.app, contract, installation.target, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported,
            "{key}"
        );
    }
    // Matching file SHA cannot substitute for the required exact mode record.
    *helper.last_mut().unwrap() ^= 1;
    fs::write(&installation.helper, &helper).unwrap();
    set(
        &mut exact,
        "sha256",
        Value::str(hex(&Sha256::digest(&helper))),
    );
    write(&installation, &exact);
    assert_eq!(
        verify_decoder(&installation.app, contract, installation.target, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let mut invalid = contract;
    invalid.decode.qualified_import_variants = &["same", "same"];
    assert_eq!(
        verify_decoder(&installation.app, invalid, installation.target, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
}
