//! Frozen independent Fraction hue_channel expectations, not product math.
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
fn params(v: &Value) -> [i16; 3] {
    assert_eq!(rows(v).len(), 3);
    std::array::from_fn(|i| i16::try_from(v[i].as_i64().unwrap()).unwrap())
}
fn policy(v: &Value) -> HueSaturationRgbV1 {
    let [h, s, l] = params(v);
    HueSaturationRgbV1::new(h, s, l).unwrap()
}
fn rgb(v: &Value, a: u8) -> Rgba {
    assert_eq!(rows(v).len(), 3);
    Rgba([byte(&v[0]), byte(&v[1]), byte(&v[2]), a])
}
fn rgba(v: &Value) -> Rgba {
    assert_eq!(rows(v).len(), 4);
    Rgba(std::array::from_fn(|i| byte(&v[i])))
}
fn oracle() -> Value {
    let raw = include_bytes!("../../tests/hue-saturation-fixtures/oracle.json");
    assert_eq!(
        hex(&Sha256::digest(raw)),
        "490cc0da56a18139abbed89b3866e449824a857ef816135e3d70c38244f25e72"
    );
    let o: Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(o["schema"], "paintcraft.proposed-hsl-independent/v1");
    o
}
#[test]
fn all_whole_rgb_records_exact_half_ties_and_partitioned_rows_match_independent_hsl() {
    let o = oracle();
    let scalar = rows(&o["scalar_cases"]);
    assert_eq!(scalar.len(), 2720);
    assert_eq!(rows(&o["parameter_order"]).len(), 40);
    let mut digest = Sha256::new();
    let mut groups: BTreeMap<[i16; 3], Vec<(Rgba, Rgba)>> = BTreeMap::new();
    for row in scalar {
        let input = rgb(&row["rgb"], 0);
        let expected = rgb(&row["expected"], 0);
        let actual = evaluate_pixel(policy(&row["parameters"]), input).unwrap();
        assert_eq!(actual, expected);
        digest.update(&actual.0[..3]);
        groups
            .entry(params(&row["parameters"]))
            .or_default()
            .push((input, expected));
    }
    assert_eq!(
        hex(&digest.finalize()),
        o["scalar_rgb_sha256"].as_str().unwrap()
    );
    assert_eq!(groups.len(), 40);
    for ([h, s, l], pairs) in groups {
        let p = HueSaturationRgbV1::new(h, s, l).unwrap();
        let input: Vec<_> = pairs.iter().map(|p| p.0).collect();
        let expected: Vec<_> = pairs.iter().map(|p| p.1).collect();
        let frozen = input.clone();
        for partition in [1, 37, 64, 65, input.len()] {
            let mut guarded = vec![Rgba([199; 4]); input.len() + 4];
            let mut work = 0;
            for (src, dst) in input
                .chunks(partition)
                .zip(guarded[2..2 + input.len()].chunks_mut(partition))
            {
                work += evaluate_row(p, src, dst, quote(src.len()).unwrap().into(), &|| Ok(()))
                    .unwrap()
                    .units;
            }
            assert_eq!(work, input.len() as u64 * 128);
            assert_eq!(guarded[2..2 + input.len()], expected);
            assert_eq!(input, frozen);
            assert!(
                guarded[..2]
                    .iter()
                    .chain(&guarded[2 + input.len()..])
                    .all(|v| *v == Rgba([199; 4]))
            );
        }
    }
    let ties = rows(&o["causal_half_ties"]);
    assert_eq!(ties.len(), 266);
    for tie in ties {
        let ci = usize::try_from(tie["color_index"].as_u64().unwrap()).unwrap();
        let pi = usize::try_from(tie["parameter_index"].as_u64().unwrap()).unwrap();
        let row = &scalar[ci * 40 + pi];
        assert_eq!(tie["exact"][1], 2);
        assert_eq!(tie["exact"][0].as_u64().unwrap() % 2, 1);
        let actual = evaluate_pixel(policy(&row["parameters"]), rgb(&row["rgb"], 0)).unwrap();
        assert_eq!(
            actual.0[usize::from(byte(&tie["channel"]))],
            byte(&tie["expected"])
        );
    }
    assert_eq!(rows(&o["literals"]).len(), 7);
    for row in rows(&o["literals"]) {
        assert_eq!(
            evaluate_pixel(policy(&row["parameters"]), rgb(&row["rgb"], 0)).unwrap(),
            rgb(&row["expected"], 0)
        );
    }
    let intent = &o["equal_effect_distinct_intent"];
    let a = policy(&intent["parameters"][0]);
    let b = policy(&intent["parameters"][1]);
    assert_ne!(a, b);
    assert_eq!(
        hex(&a.semantic_bytes()),
        intent["semantic_hex"][0].as_str().unwrap()
    );
    assert_eq!(
        hex(&b.semantic_bytes()),
        intent["semantic_hex"][1].as_str().unwrap()
    );
}
#[test]
fn all_literal_rgba_and_existing_once_only_masks_preserve_source_and_alpha() {
    let o = oracle();
    let records = rows(&o["rgba_controls"]);
    assert_eq!(records.len(), 3240);
    let mut digest = Sha256::new();
    let mut causal_hidden = 0;
    for row in records {
        let incoming = rgba(&row["input_rgba"]);
        let frozen = incoming;
        let p = policy(&row["parameters"]);
        let candidate = evaluate_pixel(p, incoming).unwrap();
        let m = byte(&row["coverage"]);
        let actual = crate::mix_point_coverage(incoming, candidate, m);
        assert_eq!(actual, rgba(&row["expected"]));
        assert_eq!(actual.0[3], incoming.0[3]);
        assert_eq!(incoming, frozen);
        if m == 0 {
            assert_eq!(actual, incoming);
        }
        if m == 255 {
            assert_eq!(actual, candidate);
        }
        if incoming.0[3] == 0 && incoming.0[..3] != actual.0[..3] {
            causal_hidden += 1;
        }
        digest.update(actual.0);
    }
    assert!(causal_hidden > 0);
    assert_eq!(hex(&digest.finalize()), o["rgba_sha256"].as_str().unwrap());
    assert_eq!(rows(&o["invalid_parameters"]).len(), 15); // lexical fixtures, not an activated parser
}
