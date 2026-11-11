use super::*;
fn contract() -> EncoderContract<'static> {
    EncoderContract {
        decode: Contract {
            application_version: "test",
            protocol_version: 2,
            profile: "host-test",
            requested_heap_ceiling_bytes: 1024,
            qualified_import_variants: &["read-test"],
            build_marker: b"marker",
            build_record: b"marker-data",
        },
        encode_protocol_version: 1,
        qualified_export_variants: &["write-test"],
    }
}
#[test]
fn absent_installation_does_not_hide_invalid_duplicate_or_incompatible_contracts() {
    let root =
        std::env::temp_dir().join(format!("paintcraft-observation-{}", uuid::Uuid::new_v4()));
    let bin = root.join("prefix/bin");
    std::fs::create_dir_all(&bin).unwrap();
    let app = bin.join("paintcraft");
    let valid = contract();
    assert!(
        observe_for_application(&app, &[valid], Target::LinuxX64, &|| Ok(()))
            .unwrap()
            .is_none()
    );
    let mut invalid = valid;
    invalid.encode_protocol_version = 2;
    let mut incompatible = valid;
    incompatible.decode.profile = "second-host-test";
    incompatible.decode.application_version = "other";
    for contracts in [
        vec![],
        vec![invalid],
        vec![valid, valid],
        vec![valid, incompatible],
        vec![valid; 3],
    ] {
        let error =
            observe_for_application(&app, &contracts, Target::LinuxX64, &|| Ok(())).unwrap_err();
        assert_eq!(error.code, ErrorCode::InputSchema);
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn cancellation_precedes_contract_validation_and_missing_path_lookup() {
    let expected = Error::new(
        ErrorCode::OperationCancelled,
        "causal observation cancellation",
    );
    let error = observe_for_application(
        Path::new("never-inspect-this-application"),
        &[],
        Target::LinuxX64,
        &|| Err(expected.clone()),
    )
    .unwrap_err();
    assert_eq!(error.code, expected.code);
    assert_eq!(error.message, expected.message);
    let error = observe_encoder_set(&[], &|| Err(expected.clone())).unwrap_err();
    assert_eq!(error.code, expected.code);
    assert_eq!(error.message, expected.message);
}
