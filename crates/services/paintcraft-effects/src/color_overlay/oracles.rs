//! Expected bytes frozen by independent Fraction generator before this kernel.
use super::*;
use paintcraft_core::types::policy::hex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
fn byte(v: &Value) -> u8 {
    u8::try_from(v.as_u64().unwrap()).unwrap()
}
fn rgba(v: &Value) -> Rgba {
    Rgba(std::array::from_fn(|i| byte(&v[i])))
}
fn oracle() -> Value {
    let raw = include_bytes!("../../tests/color-overlay-fixtures/oracle.json");
    assert_eq!(
        hex(&Sha256::digest(raw)),
        "11f56f8d2f130312bf6a3a360d07a0085d93e394a216ca0812bba872403276af"
    );
    serde_json::from_slice(raw).unwrap()
}
#[test]
fn all_frozen_rgba_vectors_scalar_partitioned_rows_and_both_packed_hashes() {
    let o = oracle();
    assert_eq!(o["schema"], "paintcraft.color-overlay-independent/v1");
    let records = o["records"].as_array().unwrap();
    assert_eq!(records.len(), 6912);
    let mut rgb = Sha256::new();
    let mut full = Sha256::new();
    let mut groups: BTreeMap<[u8; 5], Vec<(Rgba, Rgba)>> = BTreeMap::new();
    for r in records {
        let p = ColorOverlayRgbV1::new(
            std::array::from_fn(|i| byte(&r[3][i])),
            byte(&r[2]),
            EffectBlendMode::from_code(byte(&r[1])).unwrap(),
        );
        let input = rgba(&r[4]);
        let expected = rgba(&r[5]);
        let actual = evaluate_pixel(p, input).unwrap();
        assert_eq!(actual, expected, "record {r}");
        assert_eq!(actual.a(), input.a());
        rgb.update(&actual.0[..3]);
        full.update(actual.0);
        groups
            .entry(p.semantic_bytes())
            .or_default()
            .push((input, expected));
    }
    assert_eq!(hex(&rgb.finalize()), o["rgb_sha256"].as_str().unwrap());
    assert_eq!(hex(&full.finalize()), o["rgba_sha256"].as_str().unwrap());
    for (key, pairs) in groups {
        let p = ColorOverlayRgbV1::new(
            [key[0], key[1], key[2]],
            key[3],
            EffectBlendMode::from_code(key[4]).unwrap(),
        );
        let input: Vec<_> = pairs.iter().map(|p| p.0).collect();
        let expected: Vec<_> = pairs.iter().map(|p| p.1).collect();
        for partition in [1, 5, 64] {
            let mut guarded = vec![Rgba([199; 4]); input.len() + 4];
            for (src, dst) in input
                .chunks(partition)
                .zip(guarded[2..2 + input.len()].chunks_mut(partition))
            {
                evaluate_row(p, src, dst, &|| Ok(())).unwrap();
            }
            assert_eq!(guarded[2..2 + input.len()], expected);
            assert!(
                guarded[..2]
                    .iter()
                    .chain(&guarded[2 + input.len()..])
                    .all(|p| *p == Rgba([199; 4]))
            );
            assert!(input.iter().zip(&pairs).all(|(a, b)| *a == b.0));
        }
    }
}
#[test]
fn causal_single_round_controls_and_future_stage_inputs() {
    let o = oracle();
    let records = o["records"].as_array().unwrap();
    let controls = o["intermediate_rounding_counterexamples"]
        .as_array()
        .unwrap();
    assert_eq!(controls.len(), 155);
    for c in controls {
        let row = records
            .iter()
            .find(|r| r[0] == c[0] && r[1] == c[1] && r[2] == c[2] && r[4][3] == 255)
            .unwrap();
        assert_eq!(
            row[5].as_array().unwrap()[..3],
            c[3].as_array().unwrap()[..]
        );
        assert_ne!(c[3], c[4], "intermediate rounding must be causal");
    }
    let stages = o["stages"].as_array().unwrap();
    assert_eq!(stages.len(), 216);
    for s in stages {
        let p = ColorOverlayRgbV1::new(
            [220, 40, 0],
            128,
            EffectBlendMode::from_code(byte(&s[0])).unwrap(),
        );
        assert_eq!(
            evaluate_pixel(p, Rgba([20, 80, 200, byte(&s[1])])).unwrap(),
            rgba(&s[4])
        );
    }
    // The mask/opacity columns are future-stage expectations; no present Document
    // integration or execution of those later stages is claimed by this test.
}
