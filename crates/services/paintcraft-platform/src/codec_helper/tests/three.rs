use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
const THIRD_MARKER: &[u8] = b"PAINTCRAFT_CODEC_WORKER_BUILD_V6_TEST\0";
const THIRD_RECORD: &[u8] = b"PAINTCRAFT_CODEC_WORKER_BUILD_V6_TEST\0\x30.1.0\0PCDEC002\0HEAP268435456\0AVIF_SDR_WORK1\0END_CODEC_WORKER_BUILD\0";
fn third() -> Contract<'static> {
    Contract {
        profile: "charged-avif-sdr-test",
        qualified_import_variants: &[
            "qualified-test-reader",
            "qualified-avif-reader",
            "qualified-sdr-reader",
        ],
        build_marker: THIRD_MARKER,
        build_record: THIRD_RECORD,
        ..contract()
    }
}
fn install_contract(value: &Installation, contract: Contract<'_>) {
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
                .map(|name| Value::str(*name))
                .collect(),
        ),
    );
    fs::write(&value.manifest, json::canonical(&Value::Object(metadata))).unwrap();
}
fn selected(value: &Installation) -> Result<VerifiedHelper> {
    verify_set(
        &value.app,
        &[contract(), alternate(), third()],
        value.target,
        &|| Ok(()),
    )
}
#[test]
fn all_three_profiles_validate_exact_tokens_and_variants_for_six_synthetic_headers() {
    for target in [
        Target::MacArm,
        Target::MacX64,
        Target::WinArm,
        Target::WinX64,
        Target::LinuxArm,
        Target::LinuxX64,
    ] {
        let value = Installation::new(target, target.linux());
        for profile in [contract(), alternate(), third()] {
            install_contract(&value, profile);
            let helper = selected(&value).unwrap();
            assert_eq!(helper.checked_executable(&|| Ok(())).unwrap(), value.helper);
            assert_eq!(
                helper.sha256(),
                Sha256::digest(fs::read(&value.helper).unwrap()).as_slice()
            );
        }
    }
}
#[test]
fn third_profile_integrity_variants_and_cancel_fail_without_another_contract_attempt() {
    let value = Installation::new(Target::current().unwrap(), false);
    install_contract(&value, third());
    for_application_compatible_three(&value.app, contract(), alternate(), third(), &|| Ok(()))
        .unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&value.helper)
        .unwrap()
        .write_all(b"changed binary")
        .unwrap();
    let error = selected(&value).unwrap_err();
    assert!(
        error.message.contains("SHA-256"),
        "selected integrity error must survive: {}",
        error.message
    );
    install_contract(&value, third());
    let Value::Object(mut metadata) = json::parse(&fs::read(&value.manifest).unwrap()).unwrap()
    else {
        unreachable!()
    };
    metadata.insert(
        "qualified_import_variants".into(),
        Value::Array(vec![
            Value::str("qualified-test-reader"),
            Value::str("qualified-avif-reader"),
        ]),
    );
    fs::write(&value.manifest, json::canonical(&Value::Object(metadata))).unwrap();
    assert!(
        selected(&value).is_err(),
        "third profile requires its exact additional variant"
    );
    install_contract(&value, alternate());
    let Value::Object(mut metadata) = json::parse(&fs::read(&value.manifest).unwrap()).unwrap()
    else {
        unreachable!()
    };
    metadata.insert("profile".into(), Value::str(third().profile));
    metadata.insert(
        "qualified_import_variants".into(),
        Value::Array(
            third()
                .qualified_import_variants
                .iter()
                .map(|name| Value::str(*name))
                .collect(),
        ),
    );
    fs::write(&value.manifest, json::canonical(&Value::Object(metadata))).unwrap();
    assert!(
        selected(&value).is_err(),
        "a third-profile manifest cannot bless a second-profile marker"
    );
    for code in [ErrorCode::OperationCancelled, ErrorCode::ResourceLimit] {
        let calls = AtomicUsize::new(0);
        let error = verify_set(
            &value.app,
            &[contract(), alternate(), third()],
            value.target,
            &|| {
                assert_eq!(
                    calls.fetch_add(1, Ordering::SeqCst),
                    0,
                    "a rejected check must not retry"
                );
                Err(Error::new(code, "stop"))
            },
        )
        .unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn finite_set_rejects_unknown_duplicate_and_incompatible_caller_contracts() {
    let value = Installation::new(Target::current().unwrap(), false);
    for contracts in [
        vec![],
        vec![contract(); 4],
        vec![contract(), alternate(), alternate()],
        vec![
            contract(),
            alternate(),
            Contract {
                application_version: "0.2.0",
                ..third()
            },
        ],
        vec![
            contract(),
            alternate(),
            Contract {
                protocol_version: 3,
                ..third()
            },
        ],
        vec![
            contract(),
            alternate(),
            Contract {
                requested_heap_ceiling_bytes: 128 << 20,
                ..third()
            },
        ],
    ] {
        assert_eq!(
            verify_set(&value.app, &contracts, value.target, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    install_contract(&value, third());
    assert!(
        verify_known(
            &value.app,
            contract(),
            Some(alternate()),
            value.target,
            &|| Ok(())
        )
        .is_err(),
        "prior one/two-profile callers do not acquire an implicit third profile"
    );
    let Value::Object(mut metadata) = json::parse(&fs::read(&value.manifest).unwrap()).unwrap()
    else {
        unreachable!()
    };
    metadata.insert("profile".into(), Value::str("unrecognized-profile"));
    fs::write(&value.manifest, json::canonical(&Value::Object(metadata))).unwrap();
    assert!(selected(&value).is_err());
}
