//! Test-only finite manifest selection from the copied executable's real record.
//! Production discovery independently verifies architecture, SHA and contracts.
use super::*;
use paintcraft_core::types::json::Value;
use sha2::{Digest, Sha256};
type Row = (
    &'static [u8],
    Vec<u8>,
    &'static str,
    &'static [&'static str],
);
fn records(version: &str) -> Option<Vec<Row>> {
    Some(vec![
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
        (
            wire::FLOAT_BUILD_RECORD_MARKER,
            wire::native_float_build_record(version).ok()?,
            wire::FLOAT_NATIVE_PROFILE,
            wire::QUALIFIED_FLOAT_IMPORT_VARIANTS,
        ),
        (
            wire::jxr_encode::qualified::BUILD_RECORD_MARKER,
            wire::jxr_encode::qualified::build_record(version).ok()?,
            wire::jxr_encode::qualified::NATIVE_PROFILE,
            wire::jxr_encode::qualified::IMPORT_VARIANTS,
        ),
        (
            wire::jxl_integer::contracts::base::BUILD_RECORD_MARKER,
            wire::jxl_integer::contracts::base::build_record(version).ok()?,
            wire::jxl_integer::contracts::base::NATIVE_PROFILE,
            wire::jxl_integer::contracts::base::IMPORT_VARIANTS,
        ),
        (
            wire::jxl_integer::contracts::writer::BUILD_RECORD_MARKER,
            wire::jxl_integer::contracts::writer::build_record(version).ok()?,
            wire::jxl_integer::contracts::writer::NATIVE_PROFILE,
            wire::jxl_integer::contracts::writer::IMPORT_VARIANTS,
        ),
        (
            wire::jxr_encode::candidate::BUILD_RECORD_MARKER,
            wire::jxr_encode::candidate::build_record(version).ok()?,
            "private-refused",
            &[],
        ),
    ])
}
fn select(bytes: &[u8], version: &str) -> Option<(&'static str, &'static [&'static str])> {
    let mut result = None;
    for (marker, record, profile, variants) in records(version)? {
        let suffix = &record[marker.len() + version.len()..];
        let shaped = |value: &[u8]| {
            value.first().is_some_and(u8::is_ascii_digit)
                && value
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || b".+-".contains(byte))
        };
        for (at, _) in bytes
            .windows(marker.len())
            .enumerate()
            .filter(|(_, value)| *value == marker)
        {
            let rest = &bytes[at + marker.len()..];
            let Some(length) = rest.iter().take(65).position(|byte| *byte == 0) else {
                if shaped(&rest[..rest.len().min(65)]) {
                    return None;
                }
                continue;
            };
            if rest[length..].starts_with(suffix) || shaped(&rest[..length]) {
                if bytes.get(at..at + record.len()) != Some(record.as_slice()) || result.is_some() {
                    return None;
                }
                result = Some((profile, variants));
            }
        }
    }
    result.filter(|(profile, _)| *profile != "private-refused")
}
pub(super) fn manifest(installed: &Path, target: &str, required: Option<&str>) -> Value {
    let bytes = std::fs::read(installed).unwrap();
    let version = env!("CARGO_PKG_VERSION");
    let (profile, variants) = select(&bytes, version)
        .expect("copied helper must have exactly one complete current nonprivate V4-V10 contract");
    if let Some(required) = required {
        assert!(
            variants.contains(&required),
            "copied helper lacks the required fixture profile"
        );
    }
    let writer = matches!(
        profile,
        wire::jxr_encode::qualified::NATIVE_PROFILE
            | wire::jxl_integer::contracts::writer::NATIVE_PROFILE
    );
    let mut result = Value::object([
        ("schema_version", Value::Int(if writer { 3 } else { 2 })),
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
    ]);
    if writer {
        let Value::Object(fields) = &mut result else {
            unreachable!()
        };
        fields.insert(
            "encode_protocol_version".into(),
            Value::Int(wire::jxr_encode::VERSION.into()),
        );
        fields.insert(
            "qualified_export_variants".into(),
            Value::Array(
                wire::jxr_encode::qualified::EXPORT_VARIANTS
                    .iter()
                    .copied()
                    .map(Value::str)
                    .collect(),
            ),
        );
    }
    result
}
#[test]
fn finite_complete_declarations_are_distinct_from_literals_and_reject_ambiguous_records() {
    let version = env!("CARGO_PKG_VERSION");
    let rows = records(version).unwrap();
    for (_, record, expected, _) in &rows[..7] {
        assert_eq!(select(record, version).unwrap().0, *expected);
        assert!(select(&[record.as_slice(), record.as_slice()].concat(), version).is_none());
        assert!(select(&record[..record.len() - 1], version).is_none());
        assert!(select(record, "different-version").is_none());
        for (marker, other, _, _) in &rows {
            assert!(select(&[record.as_slice(), other.as_slice()].concat(), version).is_none());
            assert_eq!(
                select(&[record.as_slice(), marker].concat(), version)
                    .unwrap()
                    .0,
                *expected
            );
            let suffix = &other[marker.len() + version.len()..];
            for malformed in [b"".as_slice(), b"\xff", b"0.0.0", b"v0.0.0"] {
                assert!(
                    select(
                        &[record.as_slice(), marker, malformed, suffix].concat(),
                        version
                    )
                    .is_none()
                );
            }
            for malformed in [b"1.2.3\0PCDEC003".as_slice(), b"1.2.3"] {
                assert!(
                    select(&[record.as_slice(), marker, malformed].concat(), version).is_none()
                );
            }
            assert!(select(&[record.as_slice(), marker, b"1\xff\xfe"].concat(), version).is_some());
        }
    }
    assert!(select(rows[7].1.as_slice(), version).is_none());
    assert!(select(b"unrelated", version).is_none());
}
#[test]
fn schema_three_is_exactly_the_public_writer_profiles() {
    let root = std::env::temp_dir().join(format!("paintcraft-app-manifest-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let helper = root.join("paintcraft-codec-worker");
    for (_, record, profile, variants) in records(env!("CARGO_PKG_VERSION"))
        .unwrap()
        .into_iter()
        .take(7)
    {
        std::fs::write(&helper, record).unwrap();
        let value = manifest(&helper, "aarch64-apple-darwin", variants.last().copied());
        let writer = matches!(
            profile,
            wire::jxr_encode::qualified::NATIVE_PROFILE
                | wire::jxl_integer::contracts::writer::NATIVE_PROFILE
        );
        assert_eq!(
            value.get("schema_version").and_then(Value::as_i64),
            Some(if writer { 3 } else { 2 })
        );
        assert_eq!(
            value.as_object().unwrap().len(),
            if writer { 12 } else { 10 }
        );
        assert_eq!(
            value.get("encode_protocol_version").and_then(Value::as_i64),
            writer.then_some(wire::jxr_encode::VERSION.into())
        );
        assert_eq!(
            value.get("qualified_export_variants"),
            writer
                .then(|| Value::Array(
                    wire::jxr_encode::qualified::EXPORT_VARIANTS
                        .iter()
                        .copied()
                        .map(Value::str)
                        .collect()
                ))
                .as_ref()
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
