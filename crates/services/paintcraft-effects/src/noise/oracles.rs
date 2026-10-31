//! Independent mathematical corpus agreement is finite accuracy evidence.
//! Gaussian mathematical expectations are NOT normative libm intermediate bits
//! and do not prove ideal-real correct rounding outside these frozen cases.
use super::*;
use paintcraft_core::types::policy::hex;
use serde_json::Value;
use sha2::{Digest, Sha256};
fn number(v: &Value) -> u64 {
    v.as_u64().expect("unsigned fixture integer")
}
fn rgba(v: &Value) -> Rgba {
    let a = v.as_array().expect("fixture RGBA");
    assert_eq!(a.len(), 4);
    Rgba(std::array::from_fn(|i| {
        u8::try_from(number(&a[i])).unwrap()
    }))
}
#[test]
fn independent_finite_4380_candidates_and_26280_mask_controls_match() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../tests/noise-fixtures/oracle.json")).unwrap();
    let rows = fixture["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 4380);
    let coverages = fixture["mask_coverages"].as_array().unwrap();
    assert_eq!(coverages.len(), 6);
    let mut candidates = Sha256::new();
    let mut masked = Sha256::new();
    let mut masks = 0;
    for (index, row) in rows.iter().enumerate() {
        let d = match row["distribution"].as_str().unwrap() {
            "uniform" => NoiseDistribution::Uniform,
            "gaussian" => NoiseDistribution::Gaussian,
            _ => panic!("unknown fixture distribution"),
        };
        let p = AddNoiseRgbV1::new(
            i16::try_from(number(&row["amount_hundredths"])).unwrap(),
            u16::try_from(number(&row["seed"])).unwrap(),
            row["monochrome"].as_bool().unwrap(),
            d,
        )
        .unwrap();
        let point = SourcePixel::new(
            u32::try_from(number(&row["source_local"][0])).unwrap(),
            u32::try_from(number(&row["source_local"][1])).unwrap(),
        );
        let incoming = rgba(&row["rgba"]);
        let expected = rgba(&row["reference_rgba"]);
        let actual = evaluate_pixel(p, point, incoming).unwrap();
        assert_eq!(actual, expected, "finite mathematical case{index}");
        assert_eq!(actual.0[3], incoming.0[3]);
        candidates.update(actual.0);
        let mut row_result = [Rgba([7; 4])];
        let q = quote(p, 1).unwrap();
        let work =
            evaluate_row(p, point, &[incoming], &mut row_result, q.into(), &|| Ok(())).unwrap();
        assert_eq!(row_result, [actual]);
        assert_eq!(work.units, q.work);
        let expected_masks = row["six_mask_rgba"].as_array().unwrap();
        assert_eq!(expected_masks.len(), 6);
        for (coverage, expected) in coverages.iter().zip(expected_masks) {
            let mixed = crate::mix_point_coverage(
                incoming,
                actual,
                u8::try_from(number(coverage)).unwrap(),
            );
            assert_eq!(
                mixed,
                rgba(expected),
                "finite mathematical mask case{index}"
            );
            masked.update(mixed.0);
            masks += 1;
        }
    }
    assert_eq!(masks, 26280);
    assert_eq!(
        hex(&candidates.finalize()),
        fixture["rgba_stream_sha256"].as_str().unwrap()
    );
    assert_eq!(
        hex(&masked.finalize()),
        fixture["mask_stream_sha256"].as_str().unwrap()
    );
}
