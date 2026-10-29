//! Linkers may retain unused marker literals on debug and release targets.
//! Only complete declarations establish capability; malformed declaration-shaped
//! headers, duplicate, stale and mixed declarations always refuse.
use paintcraft_formats::codec_worker as wire;

fn contracts() -> Vec<(&'static [u8], Vec<u8>)> {
    contracts_for(env!("CARGO_PKG_VERSION"))
}
fn contracts_for(version: &str) -> Vec<(&'static [u8], Vec<u8>)> {
    vec![
        (
            wire::jxl_integer::contracts::base::BUILD_RECORD_MARKER,
            wire::jxl_integer::contracts::base::build_record(version).unwrap(),
        ),
        (
            wire::jxl_integer::contracts::writer::BUILD_RECORD_MARKER,
            wire::jxl_integer::contracts::writer::build_record(version).unwrap(),
        ),
        (
            wire::BUILD_RECORD_MARKER,
            wire::native_build_record(version).unwrap(),
        ),
        (
            wire::AVIF_BUILD_RECORD_MARKER,
            wire::native_avif_build_record(version).unwrap(),
        ),
        (
            wire::AVIF_SDR_BUILD_RECORD_MARKER,
            wire::native_avif_sdr_build_record(version).unwrap(),
        ),
        (
            wire::FLOAT_BUILD_RECORD_MARKER,
            wire::native_float_build_record(version).unwrap(),
        ),
        (
            wire::jxr_encode::candidate::BUILD_RECORD_MARKER,
            wire::jxr_encode::candidate::build_record(version).unwrap(),
        ),
        (
            wire::jxr_encode::qualified::BUILD_RECORD_MARKER,
            wire::jxr_encode::qualified::build_record(version).unwrap(),
        ),
    ]
}

#[derive(Debug, PartialEq, Eq)]
enum Invalid {
    UnknownExpected,
    ExpectedCount(usize),
    OtherDeclaration(&'static [u8]),
    MalformedDeclaration(&'static [u8]),
}
fn check(bytes: &[u8], expected: &[u8], _shipping: bool) -> Result<(), Invalid> {
    let contracts = contracts();
    if !contracts.iter().any(|(_, record)| record == expected) {
        return Err(Invalid::UnknownExpected);
    }
    let count = bytes
        .windows(expected.len())
        .filter(|value| *value == expected)
        .count();
    if count != 1 {
        return Err(Invalid::ExpectedCount(count));
    }
    for (marker, other) in contracts {
        // Match a complete known contract tail at any bounded ASCII version,
        // including stale versions of the selected profile. A standalone
        // marker retained by a debug object has neither version nor tail.
        let suffix = &other[marker.len() + env!("CARGO_PKG_VERSION").len()..];
        for (start, _) in bytes
            .windows(marker.len())
            .enumerate()
            .filter(|(_, value)| *value == marker)
        {
            let rest = &bytes[start + marker.len()..];
            let Some(version_len) = rest.iter().take(65).position(|byte| *byte == 0) else {
                if rest.first().is_some_and(u8::is_ascii_digit)
                    && rest
                        .iter()
                        .take(65)
                        .all(|byte| byte.is_ascii_alphanumeric() || b".+-".contains(byte))
                {
                    return Err(Invalid::MalformedDeclaration(marker));
                }
                continue;
            };
            let version = &rest[..version_len];
            if rest[version_len..].starts_with(suffix)
                || (version.first().is_some_and(u8::is_ascii_digit)
                    && version
                        .iter()
                        .all(|byte| byte.is_ascii_alphanumeric() || b".+-".contains(byte)))
            {
                if !rest[version_len..].starts_with(suffix) {
                    return Err(Invalid::MalformedDeclaration(marker));
                }
                let record = &bytes[start..start + marker.len() + version_len + suffix.len()];
                if record != expected {
                    return Err(Invalid::OtherDeclaration(marker));
                }
            }
        }
    }
    Ok(())
}
pub fn assert_one(bytes: &[u8], expected: &[u8]) {
    assert_eq!(check(bytes, expected, !cfg!(debug_assertions)), Ok(()));
}

/// The same feature precedence as the real helper's embedded declaration.
pub fn selected() -> Vec<u8> {
    let version = env!("CARGO_PKG_VERSION");
    if cfg!(feature = "jxl-integer-codec") {
        if cfg!(feature = "jxr-codecs") {
            wire::jxl_integer::contracts::writer::build_record(version).unwrap()
        } else {
            wire::jxl_integer::contracts::base::build_record(version).unwrap()
        }
    } else if cfg!(feature = "jxr-codecs") {
        wire::jxr_encode::qualified::build_record(version).unwrap()
    } else if cfg!(feature = "jxr-encode") {
        wire::jxr_encode::candidate::build_record(version).unwrap()
    } else if cfg!(feature = "float-codecs") {
        wire::native_float_build_record(version).unwrap()
    } else if cfg!(feature = "avif-sdr-codec") {
        wire::native_avif_sdr_build_record(version).unwrap()
    } else if cfg!(feature = "avif-codec") {
        wire::native_avif_build_record(version).unwrap()
    } else {
        wire::native_build_record(version).unwrap()
    }
}

#[test]
fn release_arm_retained_v4_literal_is_not_a_second_declaration() {
    for (marker, expected) in contracts() {
        assert_one(&expected, &expected);
        assert_eq!(check(&expected, &expected, true), Ok(()));
        for (other_marker, _) in contracts() {
            if marker == other_marker {
                continue;
            }
            let mut debug_image = expected.clone();
            debug_image.extend_from_slice(other_marker);
            assert_eq!(check(&debug_image, &expected, false), Ok(()));
            assert_eq!(check(&debug_image, &expected, true), Ok(()));
        }
    }
}
#[test]
fn duplicate_selected_and_any_other_complete_contract_are_always_rejected() {
    for (_, expected) in contracts() {
        assert_eq!(check(&[], &expected, false), Err(Invalid::ExpectedCount(0)));
        let duplicate = [expected.as_slice(), expected.as_slice()].concat();
        assert_eq!(
            check(&duplicate, &expected, false),
            Err(Invalid::ExpectedCount(2))
        );
        for (marker, other) in contracts() {
            if expected == other {
                continue;
            }
            let mixed = [expected.as_slice(), other.as_slice()].concat();
            for shipping in [false, true] {
                assert_eq!(
                    check(&mixed, &expected, shipping),
                    Err(Invalid::OtherDeclaration(marker))
                );
            }
        }
    }
}
#[test]
fn unknown_or_stale_expected_declaration_is_not_an_alternate_policy() {
    let actual = selected();
    assert_eq!(
        check(&actual, b"not-a-worker-contract", false),
        Err(Invalid::UnknownExpected)
    );
    let stale = wire::native_float_build_record("0.0.0").unwrap();
    assert_eq!(check(&stale, &stale, false), Err(Invalid::UnknownExpected));
}

#[test]
fn stale_complete_records_cannot_hide_behind_debug_literal_policy() {
    let expected = selected();
    for (marker, stale) in contracts_for("0.0.0")
        .into_iter()
        .chain(contracts_for("v0.0.0"))
    {
        let mixed = [expected.as_slice(), stale.as_slice()].concat();
        for shipping in [false, true] {
            assert_eq!(
                check(&mixed, &expected, shipping),
                Err(Invalid::OtherDeclaration(marker))
            );
        }
    }
}

#[test]
fn malformed_versioned_declarations_cannot_hide_as_literals() {
    let expected = selected();
    for (marker, record) in contracts() {
        for tail in [
            b"0.1.1\0PCDEC003\0".as_slice(),
            b"0.1.1\0PCDEC002".as_slice(),
        ] {
            let mixed = [expected.as_slice(), marker, tail].concat();
            assert_eq!(
                check(&mixed, &expected, true),
                Err(Invalid::MalformedDeclaration(marker))
            );
        }
        let truncated = [expected.as_slice(), &record[..record.len() - 1]].concat();
        assert_eq!(
            check(&truncated, &expected, true),
            Err(Invalid::MalformedDeclaration(marker))
        );
    }
}

#[test]
fn malformed_complete_versions_refuse_but_literal_binary_neighbors_are_ignored() {
    let expected = selected();
    for (marker, record) in contracts() {
        let suffix = &record[marker.len() + env!("CARGO_PKG_VERSION").len()..];
        for invalid in [b"".as_slice(), b"\xff".as_slice()] {
            let mixed = [expected.as_slice(), marker, invalid, suffix].concat();
            assert_eq!(
                check(&mixed, &expected, true),
                Err(Invalid::OtherDeclaration(marker))
            );
        }
        let random = [expected.as_slice(), marker, b"1\xff\xfe"].concat();
        assert_eq!(check(&random, &expected, true), Ok(()));
    }
}
