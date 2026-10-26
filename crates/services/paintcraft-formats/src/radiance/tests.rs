use super::*;
use crate::precision::{ColorEvidence, ColorMetadata, LinearSpace};
use paintcraft_core::types::json;
use std::cell::Cell;
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/radiance-fixtures/");
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{ROOT}{name}")).unwrap()
}
fn image(metadata: &str, resolution: &str, body: &[u8]) -> Vec<u8> {
    let mut bytes =
        format!("#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n{metadata}\n{resolution}\n").into_bytes();
    bytes.extend_from_slice(body);
    bytes
}
#[test]
fn owned_packet_corpus_has_exact_admission_and_source_binding() {
    let manifest = json::parse(&fixture("manifest.json")).unwrap();
    for entry in manifest.as_array().unwrap() {
        let name = entry.get("file").unwrap().as_str().unwrap();
        let bytes = fixture(name);
        assert_eq!(
            bytes.len() as i64,
            entry.get("bytes").unwrap().as_i64().unwrap()
        );
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, entry.get("sha256").unwrap().as_str().unwrap());
        let result = inspect(&bytes, PIXELS_MAX, &|| Ok(()));
        if let Some(expected) = entry.get("expected_error").unwrap().as_str() {
            assert_eq!(
                format!("{:?}", result.unwrap_err().code),
                expected,
                "{name}"
            );
        } else {
            let result = result.unwrap();
            assert_eq!(result.sha256(), digest);
            assert_eq!(
                i64::from(result.header().width),
                entry.get("width").unwrap().as_i64().unwrap()
            );
            assert_eq!(
                i64::from(result.header().height),
                entry.get("height").unwrap().as_i64().unwrap()
            );
            assert_eq!(
                result.packets().new_rle_rows,
                if name == "modern-rle-8x2.hdr" { 2 } else { 0 }
            );
        }
    }
}
#[test]
fn calibration_is_ordered_provenance_not_another_exposure_operation() {
    let plain = fixture("literal-2x2.hdr");
    let calibrated = fixture("calibrated-2x2.hdr");
    let a = inspect(&plain, 4, &|| Ok(())).unwrap();
    let b = inspect(&calibrated, 4, &|| Ok(())).unwrap();
    assert_eq!(b.header().applied_exposure, 6.0);
    assert_eq!(b.header().applied_color_correction, [1.0, 2.0, 2.0]);
    assert_eq!(
        &plain[a.header().data_offset..],
        &calibrated[b.header().data_offset..]
    );
    for convention in ["lower-edge", "midpoint"] {
        assert_eq!(
            fixture(&format!("literal-2x2.{convention}.f32le")),
            fixture(&format!("calibrated-2x2.{convention}.f32le"))
        );
    }
    let exposure: Vec<_> = b
        .header()
        .records
        .iter()
        .filter(|r| r.key == Some("EXPOSURE"))
        .map(|r| r.value)
        .collect();
    assert_eq!(exposure, ["2", "3"]);
    assert!(b.header().has_noncolor_metadata());
    assert_eq!(a.header().color_metadata(), ColorMetadata::RadianceDefault);
}
#[test]
fn declared_color_does_not_snap_nearby_or_conflicting_coordinates() {
    let known = fixture("explicit-rec709.hdr");
    assert_eq!(
        inspect(&known, 4, &|| Ok(()))
            .unwrap()
            .header()
            .color_metadata(),
        ColorMetadata::Declared {
            space: LinearSpace::Rec709D65,
            evidence: ColorEvidence::Chromaticities
        }
    );
    for name in ["near-rec709-unresolved.hdr", "rounded-ee-unresolved.hdr"] {
        let source = fixture(name);
        let result = inspect(&source, 4, &|| Ok(())).unwrap();
        assert_eq!(result.header().color_metadata(), ColorMetadata::Unspecified);
        assert!(result.header().primaries.is_some());
    }
    let source = image(
        "PRIMARIES=.64 .33 .30 .60 .15 .06 .3127 .3290\nPRIMARIES=.64 .33 .29 .60 .15 .06 .3333 .3333\n",
        "-Y 1 +X 1",
        &[128, 64, 32, 129],
    );
    let result = inspect(&source, 1, &|| Ok(())).unwrap();
    assert!(result.header().primaries_conflict);
    assert_eq!(result.header().color_metadata(), ColorMetadata::Unspecified);
    assert_eq!(
        result
            .header()
            .records
            .iter()
            .filter(|r| r.key == Some("PRIMARIES"))
            .count(),
        2
    );
    let source = image(
        " EXPOSURE=99\n\tFORMAT=invalid\nEXPOSURE =88\n",
        "-Y 1 +X 1",
        &[128, 64, 32, 129],
    );
    assert_eq!(
        inspect(&source, 1, &|| Ok(()))
            .unwrap()
            .header()
            .applied_exposure,
        1.0
    );
}
#[test]
fn hostile_metadata_dimensions_truncations_and_cancel_are_bounded() {
    let body = [128, 64, 32, 129];
    for metadata in [
        "EXPOSURE=NaN\n",
        "EXPOSURE=0\n",
        "EXPOSURE=1e300\nEXPOSURE=1e300\n",
        "COLORCORR=1 -1 1\n",
        "GAMMA=2.2\n",
        "PRIMARIES=1 2\n",
        "FORMAT=32-bit_rle_rgbe\n",
    ] {
        assert!(
            inspect(&image(metadata, "-Y 1 +X 1", &body), 1, &|| Ok(())).is_err(),
            "{metadata}"
        );
    }
    for resolution in ["-Y 0 +X 1", "-Y 1 +X 65537", "-Y 2 +X 2"] {
        assert_eq!(
            inspect(&image("", resolution, &body), 1, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
    }
    for metadata in [
        format!("{}\n", "a".repeat(LINE_BYTES_MAX + 1)),
        "# comment\n".repeat(HEADER_LINES_MAX),
        format!("{}\n", "a".repeat(4000)).repeat(17),
    ] {
        assert_eq!(
            inspect(&image(&metadata, "-Y 1 +X 1", &body), 1, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
    }
    let valid = fixture("modern-rle-8x2.hdr");
    for length in 0..valid.len() {
        assert!(
            inspect(&valid[..length], 16, &|| Ok(())).is_err(),
            "prefix {length}"
        );
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    assert_eq!(
        inspect(&trailing, 16, &|| Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    let calls = Cell::new(0);
    let error = inspect(&valid, 16, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 10 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel packet validation",
            ))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(calls.get(), 10);
    let oversized = vec![0; INPUT_BYTES_MAX + 1];
    assert_eq!(
        inspect(&oversized, 16, &|| Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
}
