//! Independently authored Fraction oracle; no expected value uses compiler math.
use super::{
    test_api::{Backend, Reference, Wide},
    *,
};
use paintcraft_core::types::policy::hex;
use serde_json::Value;
use sha2::{Digest, Sha256};
fn oracle() -> Value {
    let bytes = include_bytes!("../../tests/exposure-fixtures/oracle.json");
    assert_eq!(
        hex(&Sha256::digest(bytes)),
        "777c5c7a8306d7ce0ee1c6facfbe1f50aaca6bd20579969d17266c8b6a02fd0f"
    );
    let o: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(o["schema"], "paintcraft.exposure-independent-oracle/v1");
    o
}
fn rows(v: &Value) -> &[Value] {
    v.as_array().unwrap()
}
fn number(v: &Value) -> i64 {
    v.as_i64().unwrap()
}
fn rgba(v: &Value) -> Rgba {
    Rgba(std::array::from_fn(|i| number(&v[i]) as u8))
}
fn full_oracle<B: Backend>() {
    let o = oracle();
    let start = std::time::Instant::now();
    let mut scratch = B::Scratch::default();
    let mut all = Sha256::new();
    let mut tables = std::collections::BTreeMap::new();
    let mut work = 0u64;
    let mut max_work = 0;
    let mut max_quote = 0;
    assert_eq!(rows(&o["luts"]).len(), 1601);
    assert_eq!(rows(&o["exact_half_ties"]).len(), 10);
    assert_eq!(rows(&o["literal_controls"]).len(), 6);
    for (index, row) in rows(&o["luts"]).iter().enumerate() {
        let stops = number(&row["stops_hundredths"]) as i16;
        assert_eq!(stops, index as i16 - 800);
        let p = ExposureSrgbV1::new(stops).unwrap();
        let q = B::quote(p);
        let mut table = [197; 256];
        let used = B::build(p, &mut scratch, &mut table, q.into(), &|| Ok(())).unwrap();
        assert!(used.units <= q.work);
        work += used.units;
        max_work = max_work.max(used.units);
        max_quote = max_quote.max(q.work);
        assert_eq!(
            hex(&Sha256::digest(table)),
            row["lut_sha256"].as_str().unwrap(),
            "EV{stops}"
        );
        assert!(table.windows(2).all(|v| v[0] <= v[1]));
        assert_eq!(table[0], 0);
        for literal in rows(&row["literals"]) {
            assert_eq!(
                table[number(&literal["input"]) as usize],
                number(&literal["output"]) as u8
            );
        }
        for key in ["exact_half_ties", "literal_controls"] {
            for literal in rows(&o[key])
                .iter()
                .filter(|v| number(&v["stops_hundredths"]) == i64::from(stops))
            {
                assert_eq!(
                    table[number(&literal["input"]) as usize],
                    number(&literal["output"]) as u8
                );
            }
        }
        all.update(table);
        if [-800, -100, -1, 0, 1, 100, 800].contains(&stops) {
            tables.insert(stops, table);
        }
    }
    assert_eq!(
        hex(&all.finalize()),
        o["aggregate_lut_sha256"].as_str().unwrap()
    );
    assert_eq!(rows(&o["rgba_mask_controls"]).len(), 420);
    for row in rows(&o["rgba_mask_controls"]) {
        let input = rgba(&row["input_rgba"]);
        let source = input;
        let table = &tables[&(number(&row["stops_hundredths"]) as i16)];
        let candidate = evaluate_pixel(table, input);
        assert_eq!(candidate.0[3], input.0[3]);
        let output = crate::mix_point_coverage(input, candidate, number(&row["coverage"]) as u8);
        assert_eq!(output, rgba(&row["output_rgba"]));
        assert_eq!(input, source);
    }
    let alpha = &o["all_alpha_mask_controls"];
    let mut aggregate = Sha256::new();
    let mut pixels = 0;
    assert_eq!(rows(&alpha["streams"]).len(), 105);
    for row in rows(&alpha["streams"]) {
        let table = &tables[&(number(&row["stops_hundredths"]) as i16)];
        let rgb: [u8; 3] = std::array::from_fn(|i| number(&row["input_rgb"][i]) as u8);
        let mut stream = Sha256::new();
        for a in 0..=255 {
            let input = Rgba([rgb[0], rgb[1], rgb[2], a]);
            let result = crate::mix_point_coverage(
                input,
                evaluate_pixel(table, input),
                number(&row["coverage"]) as u8,
            );
            assert_eq!(result.0[3], a);
            stream.update(result.0);
            aggregate.update(result.0);
            pixels += 1;
        }
        assert_eq!(
            hex(&stream.finalize()),
            row["rgba_sha256"].as_str().unwrap()
        );
    }
    assert_eq!(pixels, 26880);
    assert_eq!(pixels, number(&alpha["rgba_pixels"]));
    assert_eq!(pixels * 4, number(&alpha["bytes"]));
    assert_eq!(
        hex(&aggregate.finalize()),
        alpha["aggregate_rgba_sha256"].as_str().unwrap()
    );
    eprintln!(
        "Exposure {}:1601 complete LUTs/409856bytes,10ties,420RGBA/mask,26880all-alpha; actual_units={work} max_actual={max_work} max_quote={max_quote} scratch={} elapsed_ms={} (test time only; not frontend/performance qualification)",
        B::NAME,
        std::mem::size_of::<B::Scratch>(),
        start.elapsed().as_millis()
    );
}
#[test]
fn reference_all_1601_hashes_ties_literal_rgba_and_all_alpha_match_independent_oracle() {
    full_oracle::<Reference>();
}
#[test]
fn wide_all_1601_hashes_ties_literal_rgba_and_all_alpha_match_independent_oracle() {
    full_oracle::<Wide>();
}
