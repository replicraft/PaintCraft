//! Independent Curves transport controls, without a rendering oracle.
use paintcraft_core::types::{
    ErrorCode,
    filters::Descriptor,
    json::{self, Value},
};
use paintcraft_formats::pcnd;
use sha2::{Digest, Sha256};
fn rows() -> Vec<Value> {
    json::parse(include_bytes!("pcnd-curves-fixtures/oracle.json"))
        .unwrap()
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec()
}
fn bytes(row: &Value) -> Vec<u8> {
    let (pairs, tail) = row
        .get("hex")
        .unwrap()
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>();
    assert!(tail.is_empty());
    pairs
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn open(bytes: &[u8]) -> paintcraft_core::types::Result<paintcraft_document::Document> {
    let budget = paintcraft_core::types::resources::SharedBudget::new(4 << 20);
    pcnd::read_with_admission_checked(
        bytes,
        2,
        65_536,
        1,
        pcnd::ReadAdmission {
            metadata: pcnd::MetadataAdmission {
                bytes: 1 << 20,
                budget: &budget,
            },
            coverage: pcnd::CoverageAdmission {
                bytes: 1 << 20,
                budget: &budget,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
}
#[test]
fn independent_curves_codec_preserves_all_intent_masks_pixels_and_exact_wire() {
    let mut count = 0;
    for row in rows()
        .iter()
        .filter(|r| r.get("expected").unwrap().as_str() == Some("accept-after-codec-integration"))
    {
        count += 1;
        let data = bytes(row);
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&data)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        let doc = open(&data).unwrap();
        let layer = &doc.layers[&doc.roots[0]];
        let operations = row.get("operations").unwrap().as_array().unwrap();
        assert_eq!(layer.filters.len(), operations.len());
        for (instance, operation) in layer.filters.iter().zip(operations) {
            assert_eq!(
                instance.descriptor().id(),
                operation.get("profile").unwrap().as_str().unwrap()
            );
            assert_eq!(
                instance.enabled(),
                operation.get("enabled").unwrap().as_bool().unwrap()
            );
            let (parameters, len) = instance.descriptor().semantic_parameter_bytes();
            let expected: Vec<u8> = operation
                .get("parameters")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as u8)
                .collect();
            assert_eq!(&parameters[..len], expected);
            assert_eq!(
                instance.mask().is_some(),
                !matches!(operation.get("mask").unwrap(), Value::Null)
            );
            if let Descriptor::CurvesRgbV1(p) = instance.descriptor() {
                assert_eq!(p.outputs().as_slice(), expected);
            }
        }
        assert_eq!(
            paintcraft_document::hex(&layer.filters.digest()),
            row.get("stack_digest_v3").unwrap().as_str().unwrap()
        );
        let raw: Vec<u8> = (0..2).flat_map(|x| layer.raster.get(x, 0).0).collect();
        assert_eq!(raw, [10, 11, 128, 0, 20, 115, 200, 128]);
        assert!(doc.source_metadata.is_none());
        assert_eq!(pcnd::write(&doc).unwrap(), data);
        let compact = pcnd::write_compact(&doc).unwrap();
        assert_eq!(compact[4], 6);
        assert!(doc.same_semantics(&open(&compact).unwrap()));
        pcnd::check_metadata_admission(&doc, &|| Ok(())).unwrap();
        pcnd::check_compact_metadata_admission(&doc, &|| Ok(())).unwrap();
        let mut removed = doc;
        removed.layers.get_mut(&removed.roots[0]).unwrap().filters = Default::default();
        assert_eq!(pcnd::write(&removed).unwrap()[4], 1);
    }
    assert_eq!(count, 9);
}
#[test]
fn independent_curves_invalid_lengths_unknown_pairs_and_inner_digest_refuse() {
    let mut count = 0;
    for row in rows()
        .iter()
        .filter(|r| r.get("expected").unwrap().as_str() != Some("accept-after-codec-integration"))
    {
        count += 1;
        let expected = match row.get("expected").unwrap().as_str().unwrap() {
            "CodecFailed" => ErrorCode::CodecFailed,
            "CapabilityUnsupported" => ErrorCode::CapabilityUnsupported,
            _ => unreachable!(),
        };
        assert_eq!(
            open(&bytes(row)).unwrap_err().code,
            expected,
            "{}",
            row.get("name").unwrap().as_str().unwrap()
        );
    }
    assert_eq!(count, 5);
}
