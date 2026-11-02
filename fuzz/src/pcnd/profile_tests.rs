//! Consumer-profile closure, separate from general reader support and seed totals.
use super::*;
use crate::{Outcome, classify};
use paintcraft_core::types::json;
use std::cell::Cell;
fn literal(oracle: &[u8], name: &str) -> Vec<u8> {
    let value = json::parse(oracle).unwrap();
    let row = value
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row.get("name").and_then(json::Value::as_str) == Some(name))
        .unwrap();
    assert_eq!(
        row.get("accepted")
            .or_else(|| row.get("accepted_planned"))
            .and_then(json::Value::as_bool),
        Some(true)
    );
    let hex = row.get("hex").unwrap().as_str().unwrap();
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect()
}
#[test]
fn reader_known_pcnd5_through8_are_unsupported_not_fuzz_invariant_failures() {
    let sources = [
        (
            5,
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../crates/services/paintcraft-formats/tests/pcnd-spatial-fixtures/box-r0.pcnd"
            ))
            .to_vec(),
        ),
        (
            6,
            literal(
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../crates/services/paintcraft-formats/tests/pcnd-catalog-fixtures/oracle.json"
                )),
                "levels-default",
            ),
        ),
        (
            7,
            literal(
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../crates/services/paintcraft-formats/tests/pcnd-layer-mask-fixtures/oracle.json"
                )),
                "disabled",
            ),
        ),
        (
            8,
            literal(
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../crates/services/paintcraft-formats/tests/pcnd-blend-fixtures/oracle.json"
                )),
                "multiply",
            ),
        ),
    ];
    for (version, bytes) in sources {
        assert_eq!(bytes[4], version);
        let metadata = SharedBudget::new(METADATA_BYTES_MAX);
        let coverage = SharedBudget::new(2 * MASK_BYTES_MAX);
        let doc = pcnd::read_with_admission_checked(
            &bytes,
            PIXELS_MAX,
            STAGED_PIXELS_MAX,
            LAYERS_MAX,
            admission(&metadata, &coverage),
            &|| Ok(()),
        )
        .unwrap();
        assert!(
            !doc.roots.is_empty(),
            "actual general reader must accept literal profile{version}"
        );
        drop(doc);
        assert_eq!(metadata.usage().reserved, 0);
        assert_eq!(coverage.usage().reserved, 0);
        let calls = Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            Ok(())
        };
        let error = exercise(&bytes, &check).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert_eq!(
            error.message,
            "Historical PCND fuzz target admits only PCND1 through PCND4"
        );
        assert_eq!(
            calls.get(),
            1,
            "no extra header parse/hash/callback before closure"
        );
        assert_eq!(classify(Err(error)), Outcome::Rejected);
        // Both public routes reject this profile. The checksum-aware route keeps
        // its existing bounded repair copy/hash before reaching the target guard.
        for result in [
            crate::exercise(crate::Target::Pcnd, &bytes),
            crate::exercise_pcnd_checksum_aware(&bytes),
        ] {
            let error = result.unwrap_err();
            assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
            assert_eq!(classify(Err(error)), Outcome::Rejected);
        }
        let cancelled = exercise(&bytes, &|| {
            Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
        })
        .unwrap_err();
        assert_eq!(
            cancelled.code,
            ErrorCode::OperationCancelled,
            "original entry cancellation wins"
        );
    }
}
#[test]
fn supported_and_non_pcnd_malformed_headers_still_reach_the_old_parser() {
    for version in 1..=4 {
        let mut bytes = *b"PCND\x01\r\n\x1a";
        bytes[4] = version;
        assert_eq!(
            exercise(&bytes, &|| Ok(())).unwrap_err().code,
            ErrorCode::CodecFailed
        );
    }
    let mut malformed = *b"PCND\x08\r\n\x1a";
    malformed[0] = b'X';
    assert_eq!(
        exercise(&malformed, &|| Ok(())).unwrap_err().code,
        ErrorCode::CodecFailed
    );
}
