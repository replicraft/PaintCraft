//! Test-only finite manifest selection from the copied executable's real record.
//! Production discovery still independently verifies architecture/SHA/contract.
use super::*;
use paintcraft_core::types::json::Value;
use sha2::{Digest, Sha256};
fn select(bytes: &[u8], version: &str) -> Option<(&'static str, &'static [&'static str])> {
    let records = [
        (
            wire::BUILD_RECORD_MARKER,
            wire::native_build_record(version).ok()?,
            wire::NATIVE_PROFILE,
            wire::QUALIFIED_NATIVE_IMPORT_VARIANTS,
        ),
        (
            wire::AVIF_BUILD_RECORD_MARKER,
            wire::native_avif_build_record(version).ok()?,
            wire::AVIF_NATIVE_PROFILE,
            wire::QUALIFIED_AVIF_IMPORT_VARIANTS,
        ),
        (
            wire::AVIF_SDR_BUILD_RECORD_MARKER,
            wire::native_avif_sdr_build_record(version).ok()?,
            wire::AVIF_SDR_NATIVE_PROFILE,
            wire::QUALIFIED_AVIF_SDR_IMPORT_VARIANTS,
        ),
    ];
    let mut result = None;
    for (marker, record, profile, variants) in records {
        let positions = bytes
            .windows(marker.len())
            .enumerate()
            .filter_map(|(at, value)| (value == marker).then_some(at))
            .collect::<Vec<_>>();
        if positions.is_empty() {
            continue;
        }
        if result.is_some()
            || positions
                .iter()
                .any(|at| bytes.get(*at..*at + record.len()) != Some(record.as_slice()))
        {
            return None;
        }
        result = Some((profile, variants));
    }
    result
}
pub(super) fn manifest(installed: &Path, target: &str, required: Option<&str>) -> Value {
    let bytes = std::fs::read(installed).unwrap();
    let version = env!("CARGO_PKG_VERSION");
    let (profile, variants) = select(&bytes, version).expect(
        "copied helper must have exactly one supported current-version V4/V5/V6 build contract",
    );
    if let Some(required) = required {
        assert!(
            variants.contains(&required),
            "copied helper does not implement the required fixture profile"
        );
    }
    Value::object([
        ("schema_version", Value::Int(2)),
        ("embedded_build_record_verified", Value::Bool(true)),
        ("application_version", Value::str(version)),
        ("target", Value::str(target)),
        (
            "executable",
            Value::str(installed.file_name().unwrap().to_str().unwrap()),
        ),
        ("sha256", Value::str(hex(&Sha256::digest(bytes)))),
        (
            "protocol_version",
            Value::Int(wire::PROTOCOL_VERSION.into()),
        ),
        ("profile", Value::str(profile)),
        (
            "requested_heap_ceiling_bytes",
            Value::Int(wire::HEAP_BYTES as i64),
        ),
        (
            "qualified_import_variants",
            Value::Array(variants.iter().copied().map(Value::str).collect()),
        ),
    ])
}
#[test]
fn manifests_follow_actual_finite_contract_and_refuse_stale_truncated_mixed_records() {
    let version = env!("CARGO_PKG_VERSION");
    for (record, expected) in [
        (
            wire::native_build_record(version).unwrap(),
            wire::NATIVE_PROFILE,
        ),
        (
            wire::native_avif_build_record(version).unwrap(),
            wire::AVIF_NATIVE_PROFILE,
        ),
        (
            wire::native_avif_sdr_build_record(version).unwrap(),
            wire::AVIF_SDR_NATIVE_PROFILE,
        ),
    ] {
        assert_eq!(select(&record, version).unwrap().0, expected);
        assert!(select(&record[..record.len() - 1], version).is_none());
        assert!(select(&record, "different-version").is_none());
    }
    let mut mixed = wire::native_build_record(version).unwrap();
    mixed.extend(wire::native_avif_sdr_build_record(version).unwrap());
    assert!(select(&mixed, version).is_none());
    assert!(select(b"unrelated", version).is_none());
}
