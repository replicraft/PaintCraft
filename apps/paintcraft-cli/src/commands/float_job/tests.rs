use super::*;
#[test]
fn exact_digests_loss_inventory_and_preparation_cancellation_fail_closed() {
    assert_eq!(digest(&"01".repeat(32)).unwrap(), [1; 32]);
    for value in [
        "a".repeat(63),
        "f".repeat(65),
        "AA".repeat(32),
        "é".repeat(32),
        "gg".repeat(32),
    ] {
        assert_eq!(digest(&value).unwrap_err().code, ErrorCode::InputSchema);
    }
    let known: Vec<_> = float_import::KNOWN_LOSSES
        .iter()
        .chain(precision::KNOWN_LOSSES)
        .map(|v| (*v).to_owned())
        .collect();
    check_losses(&known).unwrap();
    assert!(check_losses(&["invented".into()]).is_err());
    let selection:Selection=serde_json::from_value(serde_json::json!({"curve":"clip","exposure_milli_ev":0,"negative":"reject","alpha_output":"preserve"})).unwrap();
    let result = prepare(
        Path::new("not-opened.exr"),
        selection,
        Some("00".repeat(32)),
        super::super::codec_worker::owner(),
        SharedBudget::new(512 << 20),
        Arc::new(|| {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancelled before preparation",
            ))
        }),
        Frontend::Studio,
    );
    assert_eq!(result.err().unwrap().code, ErrorCode::OperationCancelled);
}
