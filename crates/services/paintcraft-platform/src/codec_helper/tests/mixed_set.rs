//! Synthetic seven-profile and six-target proof, not target codec execution.
use super::*;
use crate::codec_helper::mixed::verify_set;
const INTEGER: &str = "integer-import";
const OLD: &[&str] = &["qualified-test-reader"];
const NEW: &[&str] = &["qualified-test-reader", INTEGER];
const NINE: &[&str] = &[
    "qualified-test-reader",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    INTEGER,
];
fn legacy() -> [Contract<'static>; 5] {
    [
        (
            "v4",
            b"P4\0".as_slice(),
            b"P4\0\x30.1.0\0tail-four".as_slice(),
            OLD,
        ),
        ("v5", b"P5\0", b"P5\0\x30.1.0\0tail-five", OLD),
        ("v6", b"P6\0", b"P6\0\x30.1.0\0tail-six", OLD),
        ("v7", b"P7\0", b"P7\0\x30.1.0\0tail-seven", OLD),
        ("v9", b"P9\0", b"P9\0\x30.1.0\0tail-nine", NEW),
    ]
    .map(
        |(profile, build_marker, build_record, qualified_import_variants)| Contract {
            profile,
            build_marker,
            build_record,
            qualified_import_variants,
            ..contract()
        },
    )
}
fn current() -> [EncoderContract<'static>; 2] {
    [
        (
            "v8",
            b"P8\0".as_slice(),
            b"P8\0\x30.1.0\0tail-eight".as_slice(),
            OLD,
        ),
        ("v10", b"P10\0", b"P10\0\x30.1.0\0tail-ten", NINE),
    ]
    .map(
        |(profile, build_marker, build_record, qualified_import_variants)| EncoderContract {
            decode: Contract {
                profile,
                build_marker,
                build_record,
                qualified_import_variants,
                ..contract()
            },
            encode_protocol_version: 1,
            qualified_export_variants: &["one-export"],
        },
    )
}
fn install(f: &Installation, selected: Contract<'_>, schema3: bool) -> Value {
    let mut bytes = binary(f.target);
    bytes.truncate(128);
    bytes.extend_from_slice(selected.build_record);
    fs::write(&f.helper, &bytes).unwrap();
    let mut value = f.metadata();
    let Value::Object(fields) = &mut value else {
        unreachable!()
    };
    fields.insert("profile".into(), Value::str(selected.profile));
    fields.insert(
        "qualified_import_variants".into(),
        Value::Array(
            selected
                .qualified_import_variants
                .iter()
                .map(|name| Value::str(*name))
                .collect(),
        ),
    );
    if schema3 {
        fields.insert("schema_version".into(), Value::Int(3));
        fields.insert("encode_protocol_version".into(), Value::Int(1));
        fields.insert(
            "qualified_export_variants".into(),
            Value::Array(vec![Value::str("one-export")]),
        );
    }
    fs::write(&f.manifest, json::canonical(&value)).unwrap();
    value
}
#[test]
fn every_exact_profile_seals_only_selected_host_inventory_for_all_headers() {
    let old = legacy();
    let new = current();
    for target in [
        Target::MacArm,
        Target::MacX64,
        Target::LinuxArm,
        Target::LinuxX64,
        Target::WinArm,
        Target::WinX64,
    ] {
        let f = Installation::new(target, target.linux());
        for (selected, schema3) in old
            .iter()
            .map(|c| (*c, false))
            .chain(new.iter().map(|c| (c.decode, true)))
        {
            install(&f, selected, schema3);
            let verified = verify_set(&f.app, &old, &new, target, &|| Ok(())).unwrap();
            assert!(verified.supports_import_variant("qualified-test-reader"));
            assert_eq!(
                verified.supports_import_variant(INTEGER),
                selected.qualified_import_variants.contains(&INTEGER)
            );
            assert!(!verified.supports_import_variant("not-in-selected-host-contract"));
            assert_eq!(verified.checked_executable(&|| Ok(())).unwrap(), f.helper);
            // The returned inventory is owned and immutable, not caller slices.
            fs::write(&f.manifest, b"changed").unwrap();
            assert!(verified.checked_executable(&|| Ok(())).is_err());
        }
    }
}
#[test]
fn forged_inventory_unknown_schema_and_integrity_failure_never_fall_back() {
    let f = Installation::new(Target::MacArm, false);
    let old = legacy();
    let new = current();
    for selected in [old[3], old[4], new[1].decode] {
        let original = install(&f, selected, selected.profile == "v10");
        for (key, invalid) in [
            ("profile", Value::str("unknown")),
            (
                "qualified_import_variants",
                Value::Array(vec![Value::str(INTEGER)]),
            ),
            ("schema_version", Value::Int(99)),
            ("sha256", Value::str("00".repeat(32))),
        ] {
            let mut forged = original.clone();
            let Value::Object(fields) = &mut forged else {
                unreachable!()
            };
            fields.insert(key.into(), invalid);
            fs::write(&f.manifest, json::canonical(&forged)).unwrap();
            assert!(
                verify_set(&f.app, &old, &new, f.target, &|| Ok(())).is_err(),
                "{key}"
            );
        }
        fs::write(&f.manifest, json::canonical(&original)).unwrap();
        let verified = verify_set(&f.app, &old, &new, f.target, &|| Ok(())).unwrap();
        let mut bytes = fs::read(&f.helper).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        fs::write(&f.helper, &bytes).unwrap();
        assert!(verified.checked_executable(&|| Ok(())).is_err());
        let mut forged = original;
        let Value::Object(fields) = &mut forged else {
            unreachable!()
        };
        fields.insert("sha256".into(), Value::str(hex(&Sha256::digest(&bytes))));
        fs::write(&f.manifest, json::canonical(&forged)).unwrap();
        assert!(verify_set(&f.app, &old, &new, f.target, &|| Ok(())).is_err());
    }
    fs::remove_file(&f.manifest).unwrap();
    assert!(verify_set(&f.app, &old, &new, f.target, &|| Ok(())).is_err());
}
#[test]
fn host_sets_are_bounded_unique_and_cancel_before_acquisition() {
    let f = Installation::new(Target::LinuxX64, false);
    let old = legacy();
    let new = current();
    for (a, b) in [
        (vec![], vec![]),
        (vec![old[0]; 6], new.to_vec()),
        (old.to_vec(), vec![new[0]; 3]),
        (vec![old[0]; 2], new.to_vec()),
        (old.to_vec(), vec![new[0]; 2]),
        (vec![new[0].decode], new.to_vec()),
    ] {
        assert_eq!(
            verify_set(&f.app, &a, &b, f.target, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    install(&f, old[4], false);
    assert_eq!(
        verify_set(&f.app, &old, &new, f.target, &|| Err(Error::new(
            ErrorCode::OperationCancelled,
            "cancel"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
}

#[test]
fn selected_names_outlive_caller_storage_without_becoming_manifest_authority() {
    let f = Installation::new(Target::LinuxX64, false);
    let verified = {
        let owned = "x".repeat(128);
        let names = [owned.as_str()];
        let selected = Contract {
            qualified_import_variants: &names,
            ..legacy()[4]
        };
        install(&f, selected, false);
        verify_set(&f.app, &[selected], &[], f.target, &|| Ok(())).unwrap()
    };
    assert!(verified.supports_import_variant(&"x".repeat(128)));
    assert!(!verified.supports_import_variant(INTEGER));
    let mut forged = f.metadata();
    let Value::Object(fields) = &mut forged else {
        unreachable!()
    };
    fields.insert(
        "qualified_import_variants".into(),
        Value::Array(vec![Value::str(INTEGER)]),
    );
    fs::write(&f.manifest, json::canonical(&forged)).unwrap();
    assert!(!verified.supports_import_variant(INTEGER));
    assert!(verified.checked_executable(&|| Ok(())).is_err());
}

#[test]
fn two_writer_roles_select_exact_schema3_and_decoder_only_profile_is_refused() {
    use crate::codec_helper::encoder_set::verify_encoder_set;
    let f = Installation::new(Target::MacArm, false);
    let contracts = current();
    for selected in contracts {
        let original = install(&f, selected.decode, true);
        let encoder = verify_encoder_set(&f.app, &contracts, f.target, &|| Ok(())).unwrap();
        let decoder = verify_set(&f.app, &[], &contracts, f.target, &|| Ok(())).unwrap();
        assert_eq!(encoder.checked_executable(&|| Ok(())).unwrap(), f.helper);
        assert_eq!(decoder.checked_executable(&|| Ok(())).unwrap(), f.helper);
        for (key, invalid) in [
            ("encode_protocol_version", Value::Int(2)),
            (
                "qualified_export_variants",
                Value::Array(vec![Value::str("unknown-export")]),
            ),
            (
                "qualified_import_variants",
                Value::Array(vec![Value::str("unknown-import")]),
            ),
            ("profile", Value::str("unknown-profile")),
            ("schema_version", Value::Int(2)),
        ] {
            let mut forged = original.clone();
            let Value::Object(fields) = &mut forged else {
                unreachable!()
            };
            fields.insert(key.into(), invalid);
            fs::write(&f.manifest, json::canonical(&forged)).unwrap();
            assert!(
                verify_encoder_set(&f.app, &contracts, f.target, &|| Ok(())).is_err(),
                "{key}"
            );
            assert!(
                verify_set(&f.app, &[], &contracts, f.target, &|| Ok(())).is_err(),
                "{key}"
            );
        }
        fs::write(&f.manifest, json::canonical(&original)).unwrap();
        fs::write(&f.helper, b"changed").unwrap();
        assert!(encoder.checked_executable(&|| Ok(())).is_err());
        assert!(decoder.checked_executable(&|| Ok(())).is_err());
        assert!(verify_encoder_set(&f.app, &contracts, f.target, &|| Ok(())).is_err());
    }
    install(&f, legacy()[4], false);
    assert!(verify_encoder_set(&f.app, &contracts, f.target, &|| Ok(())).is_err());
    assert!(verify_set(&f.app, &[], &contracts, f.target, &|| Ok(())).is_err());
    for invalid in [vec![], vec![contracts[0]; 2], vec![contracts[0]; 3]] {
        assert_eq!(
            verify_encoder_set(&f.app, &invalid, f.target, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    fs::remove_file(&f.manifest).unwrap();
    assert!(verify_encoder_set(&f.app, &contracts, f.target, &|| Ok(())).is_err());
}

#[test]
fn finite_record_allowance_accepts_576_and_refuses_577_without_widening_other_fields() {
    let mut record = MARKER.to_vec();
    record.resize(576, b'x');
    let accepted = Contract {
        build_record: &record,
        ..contract()
    };
    accepted.validate().unwrap();
    record.push(b'x');
    assert_eq!(
        Contract {
            build_record: &record,
            ..contract()
        }
        .validate()
        .unwrap_err()
        .code,
        ErrorCode::InputSchema
    );
}
