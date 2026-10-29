//! Exact complete copied-helper declarations; retained literals carry no capability.
use paintcraft_formats::codec_worker as wire;
pub fn helper_contract(
    bytes: &[u8],
    version: &str,
) -> Option<(&'static str, &'static [&'static str])> {
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
            wire::jxr_encode::candidate::BUILD_RECORD_MARKER,
            wire::jxr_encode::candidate::build_record(version).ok()?,
            "private-refused",
            &[],
        ),
    ];
    let mut result = None;
    for (marker, record, profile, variants) in records {
        let suffix = &record[marker.len() + version.len()..];
        for (at, _) in bytes
            .windows(marker.len())
            .enumerate()
            .filter(|(_, value)| *value == marker)
        {
            let rest = &bytes[at + marker.len()..];
            let Some(length) = rest.iter().take(65).position(|byte| *byte == 0) else {
                if rest.first().is_some_and(u8::is_ascii_digit)
                    && rest
                        .iter()
                        .take(65)
                        .all(|byte| byte.is_ascii_alphanumeric() || b".+-".contains(byte))
                {
                    return None;
                }
                continue;
            };
            let candidate = &rest[..length];
            let shaped = candidate.first().is_some_and(u8::is_ascii_digit)
                && candidate
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || b".+-".contains(byte));
            let complete = rest[length..].starts_with(suffix);
            if shaped || complete {
                if bytes.get(at..at + record.len()) != Some(record.as_slice()) || result.is_some() {
                    return None;
                }
                result = Some((profile, variants));
            }
        }
    }
    result.filter(|(profile, _)| *profile != "private-refused")
}

#[test]
fn complete_declarations_are_distinct_from_retained_literals() {
    let version = env!("CARGO_PKG_VERSION");
    let expected = wire::native_float_build_record(version).unwrap();
    let with_literal = [expected.as_slice(), wire::BUILD_RECORD_MARKER].concat();
    assert_eq!(
        helper_contract(&with_literal, version).unwrap().0,
        wire::FLOAT_NATIVE_PROFILE
    );
    for other in [
        wire::native_build_record("v0.0.0").unwrap(),
        wire::native_build_record("0.0.0").unwrap(),
        wire::jxr_encode::candidate::build_record(version).unwrap(),
        expected.clone(),
    ] {
        assert!(helper_contract(&[expected.as_slice(), &other].concat(), version).is_none());
    }
    for malformed in [
        b"0.1.1\0PCDEC003\0".as_slice(),
        b"0.1.1\0PCDEC002".as_slice(),
    ] {
        assert!(
            helper_contract(
                &[expected.as_slice(), wire::BUILD_RECORD_MARKER, malformed].concat(),
                version
            )
            .is_none()
        );
    }
}

#[test]
fn malformed_complete_versions_refuse_but_random_literal_neighbors_do_not() {
    let version = env!("CARGO_PKG_VERSION");
    let expected = wire::native_float_build_record(version).unwrap();
    let base = wire::native_build_record(version).unwrap();
    let suffix = &base[wire::BUILD_RECORD_MARKER.len() + version.len()..];
    for invalid in [b"".as_slice(), b"\xff".as_slice()] {
        assert!(
            helper_contract(
                &[
                    expected.as_slice(),
                    wire::BUILD_RECORD_MARKER,
                    invalid,
                    suffix
                ]
                .concat(),
                version
            )
            .is_none()
        );
    }
    let random = [expected.as_slice(), wire::BUILD_RECORD_MARKER, b"1\xff\xfe"].concat();
    assert!(helper_contract(&random, version).is_some());
}
