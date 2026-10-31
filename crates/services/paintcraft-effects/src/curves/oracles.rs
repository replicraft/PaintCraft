//! Frozen independent Fraction data, not compiler-generated expected pixels.
use crate::curves::{Scratch, build_lut, evaluate_pixel, quote};
use paintcraft_core::types::{Rgba, filters::CurvesRgbV1, policy::hex};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn oracle() -> Value {
    let bytes = include_bytes!("../../tests/curves-fixtures/oracle.json");
    assert_eq!(
        hex(&Sha256::digest(bytes)),
        "a4641d09a7e5e037ef496cde6f54db82c99b13e2247fa79bffc2e2d8c5f45e40"
    );
    let value: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(
        value["schema"],
        "paintcraft.curves-proposed-independent-oracle/v1"
    );
    assert_eq!(value["knots"], serde_json::json!([0, 64, 128, 192, 255]));
    value
}
fn rows(value: &Value) -> &[Value] {
    value.as_array().unwrap()
}
fn byte(value: &Value) -> u8 {
    u8::try_from(value.as_u64().unwrap()).unwrap()
}
fn rgba(value: &Value) -> Rgba {
    assert_eq!(rows(value).len(), 4);
    Rgba(std::array::from_fn(|i| byte(&value[i])))
}
fn points(value: &Value) -> [u8; 5] {
    assert_eq!(rows(value).len(), 5);
    std::array::from_fn(|i| byte(&value[i]))
}

#[test]
fn all_64_luts_half_ties_literal_masks_and_all_alpha_match_fraction_oracle() {
    let o = oracle();
    assert_eq!(o["profile_count"], 64);
    assert_eq!(o["full_lut_bytes"], 64 * 256);
    assert_eq!(rows(&o["profiles"]).len(), 64);
    let mut scratch = Scratch::default();
    let mut tables = BTreeMap::new();
    let mut aggregate = Sha256::new();
    let mut actual_work = 0u64;
    for row in rows(&o["profiles"]) {
        let name = row["name"].as_str().unwrap();
        let p = points(&row["points"]);
        let policy = CurvesRgbV1::new(p);
        let q = quote(policy);
        let mut output = [199; 256];
        let used = build_lut(policy, &mut scratch, &mut output, q.into(), &|| Ok(())).unwrap();
        assert!(used.units <= q.work, "{name}");
        actual_work = actual_work.checked_add(used.units).unwrap();
        assert_eq!(
            hex(&Sha256::digest(output)),
            row["lut_sha256"].as_str().unwrap(),
            "{name}"
        );
        for literal in rows(&row["literals"]) {
            assert_eq!(
                output[usize::from(byte(&literal["input"]))],
                byte(&literal["expected"]),
                "{name}"
            );
        }
        for (x, y) in [0, 64, 128, 192, 255].into_iter().zip(p) {
            assert_eq!(output[x], y, "{name}: exact knot {x}");
        }
        aggregate.update(output);
        assert!(tables.insert(name.to_owned(), output).is_none());
    }
    assert_eq!(
        hex(&aggregate.finalize()),
        o["aggregate_lut_sha256"].as_str().unwrap()
    );
    assert!(actual_work > 0);
    assert_eq!(tables["identity"], std::array::from_fn(|i| i as u8));
    assert_eq!(rows(&o["half_ties"]).len(), 12);
    for literal in rows(&o["half_ties"]) {
        let table = &tables[literal["name"].as_str().unwrap()];
        assert_eq!(literal["exact"][1], 2);
        assert_eq!(literal["exact"][0].as_u64().unwrap() % 2, 1);
        assert_eq!(
            table[usize::from(byte(&literal["input"]))],
            byte(&literal["expected"])
        );
    }
    assert_eq!(rows(&o["rgba_mask_cases"]).len(), 432);
    let mut hidden_changes = 0;
    for row in rows(&o["rgba_mask_cases"]) {
        let input = rgba(&row["input"]);
        let source = input;
        let table = &tables[row["name"].as_str().unwrap()];
        let candidate = evaluate_pixel(table, input);
        assert_eq!(candidate.0[3], input.0[3]);
        let output = crate::mix_point_coverage(input, candidate, byte(&row["coverage"]));
        assert_eq!(output, rgba(&row["expected"]));
        assert_eq!(input, source);
        if input.0[3] == 0 && input.0[..3] != output.0[..3] {
            hidden_changes += 1;
        }
        if row["name"] == "identity" {
            assert_eq!(output, input);
        }
    }
    assert!(hidden_changes > 0, "Hidden RGB controls must be causal");
    assert_eq!(rows(&o["all_alpha_streams"]).len(), 144);
    let mut alpha_aggregate = Sha256::new();
    let mut alpha_records = 0;
    for row in rows(&o["all_alpha_streams"]) {
        let table = &tables[row["name"].as_str().unwrap()];
        let rgb: [u8; 3] = std::array::from_fn(|i| byte(&row["input_rgb"][i]));
        let m = byte(&row["coverage"]);
        let mut stream = Sha256::new();
        assert_eq!(row["records"], 256);
        for a in 0u8..=255 {
            let incoming = Rgba([rgb[0], rgb[1], rgb[2], a]);
            let candidate = evaluate_pixel(table, incoming);
            let output = crate::mix_point_coverage(incoming, candidate, m);
            assert_eq!(output.0[3], a);
            if m == 0 {
                assert_eq!(output, incoming);
            } else if m == 255 {
                assert_eq!(output, candidate);
            }
            stream.update(output.0);
            alpha_aggregate.update(output.0);
            alpha_records += 1;
        }
        assert_eq!(
            hex(&stream.finalize()),
            row["rgba_sha256"].as_str().unwrap()
        );
    }
    assert_eq!(alpha_records, 36864);
    assert_eq!(o["all_alpha_records"], alpha_records);
    assert_eq!(
        hex(&alpha_aggregate.finalize()),
        o["aggregate_all_alpha_sha256"].as_str().unwrap()
    );
}

#[test]
fn independent_vertex_intermediate_bounds_fit_checked_i64_for_declared_order() {
    let o = oracle();
    for (i, row) in rows(&o["profiles"])[..32].iter().enumerate() {
        assert_eq!(row["name"], format!("vertex-{i:02}"));
        let expected: [u8; 5] =
            std::array::from_fn(|j| if i & (1 << (4 - j)) == 0 { 0 } else { 255 });
        assert_eq!(points(&row["points"]), expected);
    }
    let bounds = &o["bounds"];
    assert_eq!(bounds["cases"], 32 * 256);
    assert_eq!(
        bounds["vertex_bytes_crosschecked_against_fraction_solver"],
        32 * 256
    );
    assert_eq!(bounds["i64_sufficient_for_this_order"], true);
    assert_eq!(bounds["signed_bits_required"], 42);
    let mut maximum = 0u64;
    for (name, extrema) in bounds["extrema"].as_object().unwrap() {
        let min = extrema["min"].as_i64().unwrap();
        let max = extrema["max"].as_i64().unwrap();
        assert!(min <= max, "{name}");
        let abs = min.unsigned_abs().max(max.unsigned_abs());
        maximum = maximum.max(abs);
        assert!(abs < (1u64 << 41), "{name}");
        let witness = &extrema["absolute_witness"];
        let n = witness["value"].as_i64().unwrap();
        assert!(n == min || n == max, "{name}");
        assert_eq!(n.unsigned_abs(), abs, "{name}");
        assert!(
            points(&witness["points"])
                .into_iter()
                .all(|v| v == 0 || v == 255)
        );
        assert!(witness["input"].as_u64().unwrap() < 256);
    }
    assert_eq!(maximum, 2_194_728_288_256);
    assert_eq!(bounds["max_absolute"], maximum);
    assert_eq!(
        bounds["mask_intermediates_universal_bounds"]["rounded_numerator"],
        65152
    );
    // This test binds independently proven fixture bounds to the stated order;
    // it does not inspect arbitrary compiler reassociation. All32 vertex LUTs
    // are separately compared to their independent complete hashes above.
}
