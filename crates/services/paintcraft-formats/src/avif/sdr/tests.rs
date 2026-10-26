use super::*;
use crate::avif;
use paintcraft_core::types::{Error, ErrorCode};
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
fn actual_lossless_libaom_sources_have_explicit_candidate_profiles_without_widening_v1() {
    for (size, width, height) in [("even", 16, 16), ("odd", 17, 13)] {
        for (layout, siting, position) in [
            ("444", "fullchroma", 0),
            ("420", "vertical", 1),
            ("420", "colocated", 2),
        ] {
            for (range, full) in [("full", true), ("limited", false)] {
                for alpha in [false, true] {
                    let name = format!(
                        "{layout}-{siting}-{range}-{size}{}.avif",
                        if alpha { "-alpha" } else { "" }
                    );
                    let bytes = fixture(&name);
                    let captured = bytes.clone();
                    let value = admit_ycbcr8(&bytes, 4_194_304, &|| Ok(())).unwrap();
                    let sequence = &value.primary.sequence;
                    assert_eq!(
                        (value.primary.width, value.primary.height),
                        (width, height),
                        "{name}"
                    );
                    assert_eq!(
                        (sequence.primaries, sequence.transfer, sequence.matrix),
                        (1, 13, 1)
                    );
                    assert_eq!(sequence.bit_depth, 8);
                    assert_eq!(sequence.full_range, full);
                    assert_eq!(sequence.profile, u8::from(layout == "444"));
                    assert_eq!(
                        (sequence.subsampling_x, sequence.subsampling_y),
                        (layout == "420", layout == "420")
                    );
                    assert_eq!(sequence.chroma_position, position);
                    assert_eq!(value.alpha.is_some(), alpha);
                    assert_eq!(
                        avif::admit_srgb8(&bytes, 4_194_304, &|| Ok(()))
                            .unwrap_err()
                            .code,
                        ErrorCode::CapabilityUnsupported,
                        "existing identity-v1 route must refuse {name}"
                    );
                    assert_eq!(bytes, captured);
                }
            }
        }
    }
}
#[test]
fn unknown_positions_have_no_inferred_rgb_policy_and_identity_sources_are_not_sdr() {
    for size in ["even", "odd"] {
        for range in ["full", "limited"] {
            for alpha in ["", "-alpha"] {
                let bytes = fixture(&format!("420-unknown-{range}-{size}{alpha}.avif"));
                assert_eq!(
                    admit_ycbcr8(&bytes, 4_194_304, &|| Ok(()))
                        .unwrap_err()
                        .code,
                    ErrorCode::CapabilityUnsupported
                );
            }
        }
    }
    for source in [
        include_bytes!("../../../tests/avif-fixtures/opaque.avif").as_slice(),
        include_bytes!("../../../tests/avif-fixtures/alpha.avif").as_slice(),
    ] {
        assert_eq!(
            admit_ycbcr8(source, 4_194_304, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        avif::admit_srgb8(source, 4_194_304, &|| Ok(())).unwrap();
    }
}
#[test]
fn changed_container_color_config_and_every_truncation_budget_or_cancel_refuse() {
    let bytes = fixture("420-vertical-limited-odd-alpha.avif");
    let colr = bytes.windows(4).position(|p| p == b"colr").unwrap() + 4;
    let av1c = bytes.windows(4).position(|p| p == b"av1C").unwrap() + 4;
    for (offset, value) in [
        (colr + 5, 9),
        (colr + 7, 16),
        (colr + 9, 0),
        (colr + 10, 128),
        (av1c + 2, 14),
    ] {
        let mut changed = bytes.clone();
        changed[offset] = value;
        assert!(admit_ycbcr8(&changed, 4_194_304, &|| Ok(())).is_err());
    }
    for end in 0..bytes.len() {
        assert!(
            admit_ycbcr8(&bytes[..end], 4_194_304, &|| Ok(())).is_err(),
            "truncation{end}"
        );
    }
    assert_eq!(
        admit_ycbcr8(&bytes, 220, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    for cancel_at in [1, 2, 6, 12] {
        let calls = Cell::new(0);
        let error = admit_ycbcr8(&bytes, 4_194_304, &|| {
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
