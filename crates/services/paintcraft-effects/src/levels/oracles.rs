//! Product LUTs against independent Python binary half-threshold classification.
use super::test_api::{Backend, Reference, Wide};
use super::*;
use paintcraft_core::types::policy::hex;
use serde_json::Value;
use sha2::{Digest, Sha256};
fn oracle() -> Value {
    // Heterogeneous independent test data includes intentionally invalid floats;
    // product wire parsing remains the strict integer-only Core parser.
    let value: Value =
        serde_json::from_slice(include_bytes!("../../tests/levels-fixtures/oracle.json")).unwrap();
    assert_eq!(
        value.get("schema").unwrap(),
        "paintcraft.levels-numerical-oracle/v1"
    );
    assert_eq!(
        value
            .get("invalid_parameters")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        14
    );
    value
}
fn field<'a>(v: &'a Value, key: &str) -> &'a Value {
    v.get(key).unwrap()
}
fn array(v: &Value) -> &[Value] {
    v.as_array().unwrap()
}
fn number(v: &Value) -> i64 {
    v.as_i64().unwrap()
}
fn params(v: &Value) -> [i16; 5] {
    std::array::from_fn(|i| number(&array(v)[i]) as i16)
}
fn rgba(v: &Value) -> Rgba {
    Rgba(std::array::from_fn(|i| number(&array(v)[i]) as u8))
}
fn policy(v: [i16; 5]) -> LevelsRgbV1 {
    LevelsRgbV1::new(v[0], v[1], v[2], v[3], v[4]).unwrap()
}
#[derive(Default)]
struct Counts {
    builds: u64,
    work: u64,
    max_work: u64,
    max_quote: u64,
}
fn table<B: Backend>(p: [i16; 5], scratch: &mut B::Scratch, counts: &mut Counts) -> [u8; 256] {
    let p = policy(p);
    let q = B::quote(p);
    let mut output = [199; 256];
    let used = B::build(p, scratch, &mut output, q.into(), &|| Ok(())).unwrap();
    assert!(used.units <= q.work);
    counts.builds += 1;
    counts.work += used.units;
    counts.max_work = counts.max_work.max(used.units);
    counts.max_quote = counts.max_quote.max(q.work);
    output
}
#[derive(Default)]
struct Stream {
    hash: Sha256,
    records: u64,
}
impl Stream {
    fn add(&mut self, p: [i16; 5], v: u8, result: u8) {
        let g = (p[2] as u16).to_le_bytes();
        self.hash.update([
            p[0] as u8, p[1] as u8, g[0], g[1], p[3] as u8, p[4] as u8, v, result,
        ]);
        self.records += 1;
    }
    fn finish(self, oracle: &Value, key: &str) {
        let expected = field(field(oracle, "streams"), key);
        assert_eq!(self.records, number(field(expected, "records")) as u64);
        assert_eq!(self.records * 8, number(field(expected, "bytes")) as u64);
        assert_eq!(
            hex(&self.hash.finalize()),
            field(expected, "sha256").as_str().unwrap(),
            "{key}"
        );
    }
}
fn report<B: Backend>(name: &str, counts: &Counts, start: std::time::Instant) {
    eprintln!(
        "{name} ({}): builds={} actual_work={} max_build_work={} max_quote={} scratch={} elapsed_ms={} (serial test time, not UI/performance qualification)",
        B::NAME,
        counts.builds,
        counts.work,
        counts.max_work,
        counts.max_quote,
        std::mem::size_of::<B::Scratch>(),
        start.elapsed().as_millis()
    );
}
fn ten_full_luts_six_ties_and_rgba_mask_alpha_source_preservation<B: Backend>() {
    let o = oracle();
    let mut scratch = B::Scratch::default();
    let mut counts = Counts::default();
    let mut tables = std::collections::HashMap::new();
    assert_eq!(array(field(&o, "luts")).len(), 10);
    for row in array(field(&o, "luts")) {
        let p = params(field(row, "parameters"));
        let lut = table::<B>(p, &mut scratch, &mut counts);
        assert_eq!(
            lut,
            std::array::from_fn(|i| number(&array(field(row, "expected"))[i]) as u8)
        );
        tables.insert(p, lut);
    }
    assert_eq!(array(field(&o, "half_ties")).len(), 6);
    for row in array(field(&o, "half_ties")) {
        let p = params(field(row, "parameters"));
        let lut = table::<B>(p, &mut scratch, &mut counts);
        assert_eq!(
            lut[number(field(row, "input")) as usize],
            number(field(row, "expected")) as u8
        );
    }
    assert_eq!(array(field(&o, "rgba_mask_cases")).len(), 200);
    for row in array(field(&o, "rgba_mask_cases")) {
        let p = params(field(row, "parameters"));
        let input = rgba(field(row, "input"));
        let before = input;
        let candidate = evaluate_pixel(tables.get(&p).unwrap(), input);
        assert_eq!(candidate.0[3], input.0[3]);
        assert_eq!(
            crate::mix_point_coverage(input, candidate, number(field(row, "coverage")) as u8),
            rgba(field(row, "expected"))
        );
        assert_eq!(input, before);
    }
    let all_alpha = field(&o, "all_alpha_mask_stream");
    let mut hash = Sha256::new();
    let mut records = 0;
    let rows = [[16, 240, 200, 255, 0], [0, 255, 50, 0, 255]]
        .map(|p| table::<B>(p, &mut scratch, &mut counts));
    for alpha in 0..=255 {
        for coverage in [0, 128, 255] {
            for lut in &rows {
                let p = Rgba([17, 128, 239, alpha]);
                let result = crate::mix_point_coverage(p, evaluate_pixel(lut, p), coverage);
                assert_eq!(result.0[3], alpha);
                hash.update(result.0);
                records += 1;
            }
        }
    }
    assert_eq!(records, number(field(all_alpha, "records")));
    assert_eq!(
        hex(&hash.finalize()),
        field(all_alpha, "sha256").as_str().unwrap()
    );
}
fn every_gamma_35640_records_match_independent_stream<B: Backend>() {
    let o = oracle();
    let mut scratch = B::Scratch::default();
    let mut counts = Counts::default();
    let mut stream = Stream::default();
    let start = std::time::Instant::now();
    for gamma in 10..=999 {
        for [ib, iw, ob, ow] in [[0, 255, 0, 255], [0, 255, 255, 0], [64, 192, 16, 240]] {
            let p = [ib, iw, gamma, ob, ow];
            let lut = table::<B>(p, &mut scratch, &mut counts);
            for v in [0, 1, 2, 63, 64, 127, 128, 191, 192, 253, 254, 255] {
                stream.add(p, v, lut[usize::from(v)]);
            }
        }
    }
    stream.finish(&o, "every-gamma");
    assert_eq!(counts.builds, 2970);
    report::<B>("every-gamma", &counts, start);
}
fn all_67584_channel_bytes_match_independent_stream<B: Backend>() {
    let o = oracle();
    let mut scratch = B::Scratch::default();
    let mut counts = Counts::default();
    let mut stream = Stream::default();
    let start = std::time::Instant::now();
    for [ib, iw] in [[0, 255], [0, 2], [1, 255], [17, 201], [64, 192], [127, 128]] {
        for gamma in [10, 11, 25, 50, 99, 100, 101, 125, 200, 333, 999] {
            for [ob, ow] in [[0, 255], [255, 0], [16, 240], [73, 73]] {
                let p = [ib, iw, gamma, ob, ow];
                let lut = table::<B>(p, &mut scratch, &mut counts);
                for v in 0..=255u8 {
                    stream.add(p, v, lut[usize::from(v)]);
                }
            }
        }
    }
    stream.finish(&o, "all-byte-cases");
    assert_eq!(counts.builds, 264);
    report::<B>("all-byte-cases", &counts, start);
}
fn all_32640_input_intervals_226444_edges_match_independent_stream<B: Backend>() {
    let o = oracle();
    let mut scratch = B::Scratch::default();
    let mut counts = Counts::default();
    let mut stream = Stream::default();
    let start = std::time::Instant::now();
    for ib in 0..=254u8 {
        for iw in ib + 1..=255u8 {
            let p = [i16::from(ib), i16::from(iw), 100, 19, 237];
            let lut = table::<B>(p, &mut scratch, &mut counts);
            let mut values = [
                ib.saturating_sub(1),
                ib,
                ib + 1,
                ((u16::from(ib) + u16::from(iw)) / 2) as u8,
                iw - 1,
                iw,
                iw.saturating_add(1),
            ]
            .to_vec();
            values.sort_unstable();
            values.dedup();
            for v in values {
                stream.add(p, v, lut[usize::from(v)]);
            }
        }
    }
    stream.finish(&o, "all-input-interval-edges");
    assert_eq!(counts.builds, 32640);
    report::<B>("all-input-interval-edges", &counts, start);
}

macro_rules! backend_tests {
    ($module:ident,$backend:ty) => {
        mod $module {
            use super::*;
            #[test]
            fn ten_full_luts_six_ties_and_rgba_mask_alpha_source_preservation() {
                super::ten_full_luts_six_ties_and_rgba_mask_alpha_source_preservation::<$backend>();
            }
            #[test]
            fn every_gamma_35640_records_match_independent_stream() {
                super::every_gamma_35640_records_match_independent_stream::<$backend>();
            }
            #[test]
            fn all_67584_channel_bytes_match_independent_stream() {
                super::all_67584_channel_bytes_match_independent_stream::<$backend>();
            }
            #[test]
            fn all_32640_input_intervals_226444_edges_match_independent_stream() {
                super::all_32640_input_intervals_226444_edges_match_independent_stream::<$backend>(
                );
            }
        }
    };
}
backend_tests!(reference, Reference);
backend_tests!(wide, Wide);
