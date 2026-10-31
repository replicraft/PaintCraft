//! Independent Fraction HSL expectations; no product/simplified-kernel generator.
use super::*;
use paintcraft_core::types::policy::hex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
fn rows(v: &Value) -> &[Value] {
    v.as_array().unwrap()
}
fn byte(v: &Value) -> u8 {
    u8::try_from(v.as_u64().unwrap()).unwrap()
}
fn amount(v: &Value) -> i16 {
    i16::try_from(v.as_i64().unwrap()).unwrap()
}
fn rgba(v: &Value) -> Rgba {
    assert_eq!(rows(v).len(), 4);
    Rgba(std::array::from_fn(|i| byte(&v[i])))
}
fn rgb(v: &Value, alpha: u8) -> Rgba {
    assert_eq!(rows(v).len(), 3);
    Rgba([byte(&v[0]), byte(&v[1]), byte(&v[2]), alpha])
}
fn oracle() -> Value {
    let raw = include_bytes!("../../tests/vibrance-fixtures/oracle.json");
    assert_eq!(
        hex(&Sha256::digest(raw)),
        "447b959c646862afa28bb1212d809794b5a65636777ac108863c7e04bdec8dec"
    );
    serde_json::from_slice(raw).unwrap()
}
#[test]
fn independent_hsl_rgb_grays_and_half_ties_match_scalar_and_partitioned_rows() {
    let o = oracle();
    assert_eq!(rows(&o["samples"]).len(), 5929);
    assert_eq!(rows(&o["grays"]).len(), 768);
    assert_eq!(rows(&o["half_ties"]).len(), 46);
    let mut groups: BTreeMap<i16, Vec<(Rgba, Rgba)>> = BTreeMap::new();
    for row in rows(&o["samples"]) {
        let a = amount(&row["amount_hundredths"]);
        let input = rgb(&row["rgb"], 0);
        let expected = rgb(&row["output"], 0);
        assert_eq!(
            evaluate_pixel(VibranceRgbV1::new(a).unwrap(), input).unwrap(),
            expected
        );
        groups.entry(a).or_default().push((input, expected));
    }
    assert_eq!(groups.len(), 11);
    for (a, pairs) in groups {
        let policy = VibranceRgbV1::new(a).unwrap();
        let input: Vec<_> = pairs.iter().map(|p| p.0).collect();
        let expected: Vec<_> = pairs.iter().map(|p| p.1).collect();
        let frozen = input.clone();
        for partition in [1, 37, 64, 65, input.len()] {
            let mut output = vec![Rgba([199; 4]); input.len()];
            let mut work = 0;
            for (src, dst) in input.chunks(partition).zip(output.chunks_mut(partition)) {
                work += evaluate_row(policy, src, dst, quote(src.len()).unwrap().into(), &|| {
                    Ok(())
                })
                .unwrap()
                .units;
            }
            assert_eq!(work, input.len() as u64 * 64);
            assert_eq!(output, expected);
            assert_eq!(input, frozen);
        }
    }
    for row in rows(&o["grays"]) {
        let v = byte(&row["gray"]);
        let p = VibranceRgbV1::new(amount(&row["amount_hundredths"])).unwrap();
        assert_eq!(
            evaluate_pixel(p, Rgba([v, v, v, 0])).unwrap(),
            rgb(&row["output"], 0)
        );
    }
    for row in rows(&o["half_ties"]) {
        assert_eq!(row["denominator"], 2);
        assert_eq!(row["numerator"].as_u64().unwrap() % 2, 1);
        let p = VibranceRgbV1::new(amount(&row["amount_hundredths"])).unwrap();
        let actual = evaluate_pixel(p, rgb(&row["rgb"], 0)).unwrap();
        assert_eq!(
            actual.0[usize::from(byte(&row["channel"]))],
            byte(&row["output"])
        );
    }
}
#[test]
fn independent_rgba_masks_preserve_source_alpha_and_mix_one_rounded_candidate() {
    let o = oracle();
    assert_eq!(rows(&o["rgba_mask"]).len(), 4860);
    let mut causal_hidden = 0;
    for row in rows(&o["rgba_mask"]) {
        let input = rgba(&row["input"]);
        let frozen = input;
        let p = VibranceRgbV1::new(amount(&row["amount_hundredths"])).unwrap();
        let candidate = evaluate_pixel(p, input).unwrap();
        assert_eq!(candidate, rgba(&row["candidate"]));
        let coverage = byte(&row["coverage"]);
        let actual = crate::mix_point_coverage(input, candidate, coverage);
        assert_eq!(actual, rgba(&row["output"]));
        assert_eq!(actual.0[3], input.0[3]);
        assert_eq!(input, frozen);
        if input.0[3] == 0 && actual.0[..3] != input.0[..3] {
            causal_hidden += 1;
        }
        if coverage == 0 {
            assert_eq!(actual, input);
        }
        if coverage == 255 {
            assert_eq!(actual, candidate);
        }
    }
    assert!(causal_hidden > 0);
}
