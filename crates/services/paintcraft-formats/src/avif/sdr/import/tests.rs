use super::*;
use std::{cell::Cell, path::PathBuf};
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/avif-sdr-fixtures")
            .join(name),
    )
    .unwrap()
}
#[test]
fn exact_sdr_source_range_siting_color_geometry_and_sha_are_sealed() {
    for (name, full, position, origin, primary, alpha) in [
        ("444-fullchroma-full-even-alpha.avif", true, 0, 0, "", ""),
        ("420-colocated-full-odd-alpha.avif", true, 2, 0, "", ""),
        ("420-vertical-limited-odd-alpha.avif", false, 1, 1, "", ""),
        ("named-primary.avif", false, 1, 1, "Color image", ""),
        ("named-alpha.avif", false, 1, 1, "", "Alpha mask"),
        ("named-both.avif", false, 1, 1, "Color image", "Alpha mask"),
    ] {
        let bytes = fixture(name);
        let captured = bytes.clone();
        let value = assess_import(&bytes, crate::avif::INSPECT_PIXELS_MAX, &|| Ok(())).unwrap();
        assert_eq!(value.sha256(), <[u8; 32]>::from(Sha256::digest(&bytes)));
        assert_eq!(value.source_len(), bytes.len() as u64);
        assert_eq!(value.color_policy(), COLOR_POLICY);
        assert_eq!(value.primary_id(), 1);
        assert_eq!(value.primary_name(), primary);
        assert_eq!(value.alpha_id(), Some(2));
        assert_eq!(value.alpha_name(), Some(alpha));
        assert!(value.alpha());
        let sequence = value.primary_sequence();
        assert_eq!(
            (sequence.primaries, sequence.transfer, sequence.matrix),
            (1, 13, 1)
        );
        assert_eq!(sequence.full_range, full);
        assert_eq!(sequence.chroma_position, position);
        assert_eq!(value.size(), (sequence.width, sequence.height));
        assert!(value.alpha_sequence().unwrap().monochrome);
        let json = value.to_value();
        assert_eq!(
            json.get("source_sha256").and_then(Value::as_str),
            Some(hex(&value.sha256()).as_str())
        );
        assert_eq!(
            json.get("chroma_origin_twice_luma"),
            Some(&Value::Array(vec![Value::Int(0), Value::Int(origin)]))
        );
        assert_eq!(
            json.get("preserving_save").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(bytes, captured);
    }
}
#[test]
fn actual_reconstruction_is_required_and_names_only_reported_when_present() {
    for name in [
        "420-vertical-limited-odd-alpha.avif",
        "named-primary.avif",
        "named-alpha.avif",
        "named-both.avif",
    ] {
        let value =
            assess_import(&fixture(name), crate::avif::INSPECT_PIXELS_MAX, &|| Ok(())).unwrap();
        let named = name.starts_with("named");
        assert_eq!(
            value.losses(),
            if named {
                KNOWN_IMPORT_LOSSES
            } else {
                &KNOWN_IMPORT_LOSSES[..1]
            }
        );
        assert_eq!(
            value.require_losses(&[]).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            value.require_losses(&["unknown".into()]).unwrap_err().code,
            ErrorCode::InputSchema
        );
        assert_eq!(
            value
                .require_losses(&["avif-item-names-not-retained".into()])
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        let color = vec!["avif-color-reconstruction".into()];
        if named {
            assert_eq!(
                value.require_losses(&color).unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
        } else {
            value.require_losses(&color).unwrap();
        }
        value
            .require_losses(
                &value
                    .losses()
                    .iter()
                    .map(|code| (*code).to_owned())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
    }
    let identity = crate::avif::assess_import(
        include_bytes!("../../../../tests/avif-fixtures/opaque.avif"),
        256,
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        identity
            .require_losses(&["avif-color-reconstruction".into()])
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
}
#[test]
fn unqualified_sources_budget_and_cancel_never_return_a_sealed_assessment() {
    for source in [
        fixture("420-unknown-full-odd-alpha.avif"),
        include_bytes!("../../../../tests/avif-fixtures/opaque.avif").to_vec(),
    ] {
        assert_eq!(
            assess_import(&source, crate::avif::INSPECT_PIXELS_MAX, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    let bytes = fixture("named-both.avif");
    assert_eq!(
        assess_import(&bytes, 220, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    for cancel_at in [1, 2, 8, 16] {
        let calls = Cell::new(0);
        let error = assess_import(&bytes, crate::avif::INSPECT_PIXELS_MAX, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == cancel_at {
                Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
}
