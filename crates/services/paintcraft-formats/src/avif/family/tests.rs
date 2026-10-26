use super::*;
use std::{cell::Cell, fs, path::PathBuf};
fn fixture(directory: &str, name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(directory)
            .join(name),
    )
    .unwrap()
}
#[test]
fn family_selection_preserves_exact_sealed_identity_and_sdr_assessments() {
    for name in [
        "opaque.avif",
        "alpha.avif",
        "named-opaque.avif",
        "named-alpha.avif",
    ] {
        let bytes = fixture("avif-fixtures", name);
        assert_eq!(
            assess_family(&bytes, INSPECT_PIXELS_MAX, &|| Ok(())).unwrap(),
            FamilyAssessment::Identity(
                assess_import(&bytes, INSPECT_PIXELS_MAX, &|| Ok(())).unwrap()
            )
        );
        assert!(sdr::assess_import(&bytes, INSPECT_PIXELS_MAX, &|| Ok(())).is_err());
    }
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/avif-sdr-fixtures");
    let mut accepted = 0;
    let mut refused = 0;
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "avif") {
            continue;
        }
        let bytes = fs::read(path).unwrap();
        assert!(assess_import(&bytes, INSPECT_PIXELS_MAX, &|| Ok(())).is_err());
        match sdr::assess_import(&bytes, INSPECT_PIXELS_MAX, &|| Ok(())) {
            Ok(expected) => {
                assert_eq!(
                    assess_family(&bytes, INSPECT_PIXELS_MAX, &|| Ok(())).unwrap(),
                    FamilyAssessment::Sdr(expected)
                );
                accepted += 1;
            }
            Err(_) => {
                assert!(assess_family(&bytes, INSPECT_PIXELS_MAX, &|| Ok(())).is_err());
                refused += 1;
            }
        }
    }
    assert_eq!((accepted, refused), (27, 8));
}
#[test]
fn family_failures_propagate_without_retry_or_native_decoding() {
    let bytes = fixture("avif-sdr-fixtures", "named-both.avif");
    let calls = Cell::new(0);
    let error = assess_family(&bytes, INSPECT_PIXELS_MAX, &|| {
        calls.set(calls.get() + 1);
        Err(Error::new(ErrorCode::OperationCancelled, "stop once"))
    })
    .unwrap_err();
    assert_eq!(calls.get(), 1);
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(
        assess_family(&bytes, 1, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(assess_family(b"invalid", INSPECT_PIXELS_MAX, &|| Ok(())).is_err());
}
