//! Debug linkers may retain unused marker literals from linked library objects.
//! They must never retain a second complete worker capability declaration.
//! Shipping release builds additionally retain the strict bare-marker check.
use paintcraft_formats::codec_worker as wire;

fn contracts() -> Vec<(&'static [u8], Vec<u8>)> {
    contracts_for(env!("CARGO_PKG_VERSION"))
}
fn contracts_for(version: &str) -> Vec<(&'static [u8], Vec<u8>)> {
    vec![
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
    ShippingMarker(&'static [u8]),
}
fn check(bytes: &[u8], expected: &[u8], shipping: bool) -> Result<(), Invalid> {
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
                continue;
            };
            if version_len > 0
                && rest[..version_len].is_ascii()
                && rest[version_len..].starts_with(suffix)
            {
                let record = &bytes[start..start + marker.len() + version_len + suffix.len()];
                if record != expected {
                    return Err(Invalid::OtherDeclaration(marker));
                }
            }
        }
        if shipping && other != expected && bytes.windows(marker.len()).any(|value| value == marker)
        {
            return Err(Invalid::ShippingMarker(marker));
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
    if cfg!(feature = "jxr-codecs") {
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
fn debug_literal_is_not_a_second_declaration_but_shipping_rejects_it() {
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
            assert_eq!(
                check(&debug_image, &expected, true),
                Err(Invalid::ShippingMarker(other_marker))
            );
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
    for (marker, stale) in contracts_for("0.0.0") {
        let mixed = [expected.as_slice(), stale.as_slice()].concat();
        for shipping in [false, true] {
            assert_eq!(
                check(&mixed, &expected, shipping),
                Err(Invalid::OtherDeclaration(marker))
            );
        }
    }
}
