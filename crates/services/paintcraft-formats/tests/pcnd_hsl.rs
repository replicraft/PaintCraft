//! Independent HSL transport controls, without a rendering oracle.
use paintcraft_core::types::{
    ErrorCode,
    filters::Descriptor,
    json::{self, Value},
};
use paintcraft_formats::pcnd;
use sha2::{Digest, Sha256};
fn rows() -> Vec<Value> {
    json::parse(include_bytes!("pcnd-hsl-fixtures/oracle.json"))
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
fn independent_hsl_preserves_raw_source_parameters_identity_and_minimal_version() {
    let mut accepted = 0;
    for row in rows()
        .iter()
        .filter(|r| r.get("accepted").unwrap().as_bool() == Some(true))
    {
        accepted += 1;
        let data = bytes(row);
        assert_eq!(
            paintcraft_document::hex(&Sha256::digest(&data)),
            row.get("sha256").unwrap().as_str().unwrap()
        );
        let doc = open(&data).unwrap();
        let layer = &doc.layers[&doc.roots[0]];
        let instance = &layer.filters.as_slice()[0];
        let Descriptor::HueSaturationRgbV1(params) = instance.descriptor() else {
            panic!("wrong profile")
        };
        let expected = row.get("parameters").unwrap().as_array().unwrap();
        assert_eq!(
            [
                i64::from(params.hue_degrees()),
                i64::from(params.saturation_hundredths()),
                i64::from(params.lightness_hundredths())
            ],
            [
                expected[0].as_i64().unwrap(),
                expected[1].as_i64().unwrap(),
                expected[2].as_i64().unwrap()
            ]
        );
        if let Some(enabled) = row.get("mask_enabled").unwrap().as_bool() {
            let mask = instance.mask().unwrap();
            assert_eq!(mask.enabled(), enabled);
            let d = mask.coverage().domain();
            assert_eq!([d.x0, d.y0, d.x1, d.y1], [-1, -2, 3, 2]);
            assert_eq!(mask.coverage().stats().bands, 0);
        } else {
            assert!(instance.mask().is_none());
        }
        assert_eq!(
            instance.enabled(),
            row.get("enabled").unwrap().as_bool().unwrap()
        );
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
        let restored = open(&compact).unwrap();
        assert!(doc.same_semantics(&restored));
        pcnd::check_metadata_admission(&doc, &|| Ok(())).unwrap();
        pcnd::check_compact_metadata_admission(&doc, &|| Ok(())).unwrap();
        let requirements = pcnd::catalog_manifest(&data).unwrap();
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements[0].profile_id, "hue-saturation-rgb-v1");
        assert_eq!(requirements[0].codec_id, "params-v1");
        let mut removed = doc;
        removed.layers.get_mut(&removed.roots[0]).unwrap().filters = Default::default();
        assert_eq!(pcnd::write(&removed).unwrap()[4], 1);
    }
    assert_eq!(accepted, 13);
}
#[test]
fn independent_hsl_rejects_noncanonical_lengths_out_of_range_and_unknown_codec() {
    let mut refused = 0;
    for row in rows()
        .iter()
        .filter(|r| r.get("accepted").unwrap().as_bool() == Some(false))
    {
        refused += 1;
        let data = bytes(row);
        assert_eq!(
            &Sha256::digest(&data[..data.len() - 32])[..],
            &data[data.len() - 32..]
        );
        let error = open(&data).unwrap_err();
        let expected = match row.get("error").unwrap().as_str().unwrap() {
            "CodecFailed" => ErrorCode::CodecFailed,
            "CapabilityUnsupported" => ErrorCode::CapabilityUnsupported,
            _ => unreachable!(),
        };
        assert_eq!(
            error.code,
            expected,
            "{}",
            row.get("name").unwrap().as_str().unwrap()
        );
    }
    assert_eq!(refused, 17);
}
