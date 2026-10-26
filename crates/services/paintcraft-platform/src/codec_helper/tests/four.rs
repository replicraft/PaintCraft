use super::*;
fn contracts() -> [Contract<'static>; 4] {
    [
        contract(),
        alternate(),
        Contract {
            profile: "third-exact-test",
            build_marker: b"THIRD_RECORD\0",
            build_record: b"THIRD_RECORD\0exact-third-contract",
            ..contract()
        },
        Contract {
            profile: "fourth-exact-test",
            build_marker: b"FOURTH_RECORD\0",
            build_record: b"FOURTH_RECORD\0exact-fourth-contract",
            ..contract()
        },
    ]
}
fn install(value: &Installation, contract: Contract<'_>) {
    let mut bytes = binary(value.target);
    bytes.truncate(128);
    bytes.extend(contract.build_record);
    fs::write(&value.helper, bytes).unwrap();
    let Value::Object(mut metadata) = value.metadata() else {
        unreachable!()
    };
    metadata.insert("profile".into(), Value::str(contract.profile));
    metadata.insert(
        "qualified_import_variants".into(),
        Value::Array(
            contract
                .qualified_import_variants
                .iter()
                .map(|v| Value::str(*v))
                .collect(),
        ),
    );
    fs::write(&value.manifest, json::canonical(&Value::Object(metadata))).unwrap();
}
fn selected(value: &Installation) -> Result<VerifiedHelper> {
    verify_set(&value.app, &contracts(), value.target, &|| Ok(()))
}
#[test]
fn four_exact_profiles_keep_all_six_synthetic_targets_and_prior_choices() {
    for target in [
        Target::MacArm,
        Target::MacX64,
        Target::WinArm,
        Target::WinX64,
        Target::LinuxArm,
        Target::LinuxX64,
    ] {
        let value = Installation::new(target, target.linux());
        for profile in contracts() {
            install(&value, profile);
            assert_eq!(
                selected(&value)
                    .unwrap()
                    .checked_executable(&|| Ok(()))
                    .unwrap(),
                value.helper
            );
        }
    }
    let value = Installation::new(Target::current().unwrap(), false);
    let [a, b, c, d] = contracts();
    install(&value, d);
    for_application_compatible_four(&value.app, a, b, c, d, &|| Ok(())).unwrap();
    assert!(for_application_compatible_three(&value.app, a, b, c, &|| Ok(())).is_err());
}
#[test]
fn fourth_selection_refuses_unknown_forged_corrupt_and_mixed_contracts_without_retry() {
    let value = Installation::new(Target::current().unwrap(), false);
    let [a, b, c, d] = contracts();
    for candidates in [
        vec![],
        vec![a, b, c, d, a],
        vec![a, b, c, c],
        vec![
            a,
            b,
            c,
            Contract {
                application_version: "0.2.0",
                ..d
            },
        ],
        vec![
            a,
            b,
            c,
            Contract {
                protocol_version: 3,
                ..d
            },
        ],
        vec![
            a,
            b,
            c,
            Contract {
                requested_heap_ceiling_bytes: 128 << 20,
                ..d
            },
        ],
    ] {
        assert_eq!(
            verify_set(&value.app, &candidates, value.target, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    for (field, new_value) in [
        ("profile", Value::str("unknown-profile")),
        ("qualified_import_variants", Value::Array(vec![])),
        ("sha256", Value::str("0".repeat(64))),
    ] {
        install(&value, d);
        let Value::Object(mut metadata) = json::parse(&fs::read(&value.manifest).unwrap()).unwrap()
        else {
            unreachable!()
        };
        metadata.insert(field.into(), new_value);
        fs::write(&value.manifest, json::canonical(&Value::Object(metadata))).unwrap();
        assert!(selected(&value).is_err());
    }
    install(&value, d);
    fs::OpenOptions::new()
        .append(true)
        .open(&value.helper)
        .unwrap()
        .write_all(b"changed")
        .unwrap();
    assert!(selected(&value).unwrap_err().message.contains("SHA-256"));
    install(&value, c);
    let Value::Object(mut metadata) = json::parse(&fs::read(&value.manifest).unwrap()).unwrap()
    else {
        unreachable!()
    };
    metadata.insert("profile".into(), Value::str(d.profile));
    fs::write(&value.manifest, json::canonical(&Value::Object(metadata))).unwrap();
    assert!(
        selected(&value).is_err(),
        "fourth manifest cannot bless third build record"
    );
    let calls = std::cell::Cell::new(0);
    let error = verify_set(&value.app, &contracts(), value.target, &|| {
        calls.set(calls.get() + 1);
        Err(Error::new(ErrorCode::OperationCancelled, "stop"))
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(calls.get(), 1);
}
