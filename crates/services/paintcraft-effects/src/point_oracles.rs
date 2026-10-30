//! Product streams against the independent Python/Fraction oracle, not Rust math.
use super::*;
use paintcraft_core::types::{
    filters::*,
    json::{self, Value},
    policy::hex,
};
use sha2::{Digest, Sha256};
fn oracle() -> Value {
    json::parse(include_bytes!("../tests/point-fixtures/oracle.json")).unwrap()
}
fn rgba(v: &Value) -> Rgba {
    Rgba(
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| u8::try_from(x.as_i64().unwrap()).unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
    )
}
fn descriptor(row: &Value) -> Descriptor {
    let kind = row.get("kind").unwrap().as_str().unwrap();
    let parameter = row.get("parameter").unwrap();
    let value = match kind {
        "black-white-rgb-v1" => {
            Value::object([("kind", Value::str(kind)), ("strength", parameter.clone())])
        }
        "threshold-luma-v1" => {
            Value::object([("kind", Value::str(kind)), ("level", parameter.clone())])
        }
        "posterize-rgb-v1" => {
            Value::object([("kind", Value::str(kind)), ("levels", parameter.clone())])
        }
        "solarize-rgb-v1" => Value::object([("kind", Value::str(kind))]),
        _ => panic!("unknown oracle kind"),
    };
    Descriptor::from_value(&value).unwrap()
}
#[derive(Default)]
struct Stream {
    hash: Sha256,
    records: u64,
    bytes: u64,
}
impl Stream {
    fn add(&mut self, bytes: &[u8]) {
        self.hash.update(bytes);
        self.records += 1;
        self.bytes += bytes.len() as u64;
    }
    fn finish(self, oracle: &Value, key: &str) -> u64 {
        let expected = oracle.get("streams").unwrap().get(key).unwrap();
        assert_eq!(
            self.records,
            expected.get("records").unwrap().as_i64().unwrap() as u64,
            "{key} records"
        );
        assert_eq!(
            self.bytes,
            expected.get("bytes").unwrap().as_i64().unwrap() as u64,
            "{key} bytes"
        );
        assert_eq!(
            hex(&self.hash.finalize()),
            expected.get("sha256").unwrap().as_str().unwrap(),
            "{key} product stream"
        );
        self.records
    }
}
#[test]
fn all_445554_independent_records_match_six_frozen_stream_hashes() {
    let oracle = oracle();
    let mut total = 0;
    let mut solar = Stream::default();
    for c in 0..=255u8 {
        solar.add(&evaluate_pixel(&[Descriptor::SolarizeRgbV1], Rgba([c, c, c, 0])).0[..1]);
    }
    total += solar.finish(&oracle, "solarize_all_channels");
    let mut poster = Stream::default();
    for n in 2..=32 {
        let d = Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(n).unwrap());
        for c in 0..=255u8 {
            poster.add(&evaluate_pixel(&[d], Rgba([c, c, c, 0])).0[..1]);
        }
    }
    total += poster.finish(&oracle, "posterize_all_parameters_channels");
    let mut gray = Stream::default();
    for s in 0..=100 {
        let d = Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(s).unwrap());
        for c in 0..=255u8 {
            for a in [0, 1, 128, 255] {
                gray.add(&evaluate_pixel(&[d], Rgba([c, c, c, a])).0);
            }
        }
    }
    total += gray.finish(&oracle, "black_white_gray_identity");
    let mut threshold = Stream::default();
    for t in 0..=255 {
        let d = Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(t).unwrap());
        for c in 0..=255u8 {
            for a in [0, 1, 128, 255] {
                threshold.add(&evaluate_pixel(&[d], Rgba([c, c, c, a])).0);
            }
        }
    }
    total += threshold.finish(&oracle, "threshold_all_gray_boundaries");
    let anchors: Vec<_> = oracle
        .get("chromatic_threshold_anchors")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    let mut brackets = Stream::default();
    for t in 0..=255i64 {
        let d = Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(t as i16).unwrap());
        for &r in &anchors {
            for &g in &anchors {
                let remaining = 10000 * t - 2126 * r - 7152 * g;
                let crossing = -(-remaining).div_euclid(722);
                let mut blue = vec![0, 255];
                blue.extend(
                    [crossing - 1, crossing, crossing + 1]
                        .into_iter()
                        .filter(|v| (0..=255).contains(v)),
                );
                blue.sort_unstable();
                blue.dedup();
                for b in blue {
                    let result = evaluate_pixel(&[d], Rgba([r as u8, g as u8, b as u8, 0]));
                    brackets.add(&[t as u8, r as u8, g as u8, b as u8, result.0[0]]);
                }
            }
        }
    }
    total += brackets.finish(&oracle, "threshold_chromatic_brackets");
    let mut cross = Stream::default();
    for s in 0..=100 {
        let d = Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(s).unwrap());
        for r in [0, 127, 128, 255] {
            for g in [0, 127, 128, 255] {
                for b in [0, 127, 128, 255] {
                    for a in [0, 1, 128, 255] {
                        cross.add(&evaluate_pixel(&[d], Rgba([r, g, b, a])).0);
                    }
                }
            }
        }
    }
    total += cross.finish(&oracle, "black_white_cross_channels");
    assert_eq!(total, 445554);
}
#[test]
fn independent_literals_masks_ties_and_stage_order_are_exact() {
    let o = oracle();
    for row in o.get("literal_controls").unwrap().as_array().unwrap() {
        assert_eq!(
            evaluate_pixel(&[descriptor(row)], rgba(row.get("rgba").unwrap())),
            rgba(row.get("expected").unwrap())
        );
    }
    let context = o.get("context").unwrap();
    for row in context.get("mask_samples").unwrap().as_array().unwrap() {
        let source = rgba(row.get("rgba").unwrap());
        let candidate = evaluate_pixel(&[descriptor(row)], source);
        assert_eq!(
            mix_point_coverage(
                source,
                candidate,
                row.get("coverage").unwrap().as_i64().unwrap() as u8
            ),
            rgba(row.get("expected").unwrap())
        );
    }
    let bw = Descriptor::BlackWhiteRgbV1(BlackWhiteV1::default());
    let threshold = Descriptor::ThresholdLumaV1(ThresholdLumaV1::default());
    let order = context.get("order").unwrap();
    let source = rgba(order.get("rgba").unwrap());
    assert_eq!(
        evaluate_pixel(&[bw, threshold], source),
        rgba(order.get("black_white100_then_threshold128").unwrap())
    );
    assert_eq!(
        evaluate_pixel(&[threshold], source),
        rgba(order.get("direct_threshold128").unwrap())
    );
    let tie = context.get("rounded_candidate_before_mask").unwrap();
    let source = rgba(tie.get("rgba").unwrap());
    assert_eq!(
        mix_point_coverage(source, evaluate_pixel(&[bw], source), 128),
        rgba(tie.get("expected").unwrap())
    );
    let poster = Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(3).unwrap());
    let input = Rgba([127, 128, 255, 0]);
    assert_ne!(
        evaluate_pixel(&[poster, Descriptor::SolarizeRgbV1], input),
        evaluate_pixel(&[Descriptor::SolarizeRgbV1, poster], input)
    );
    let input = Rgba([200, 200, 200, 0]);
    let first = mix_point_coverage(
        input,
        evaluate_pixel(&[Descriptor::SolarizeRgbV1], input),
        64,
    );
    let second = mix_point_coverage(
        first,
        evaluate_pixel(&[Descriptor::SolarizeRgbV1], first),
        64,
    );
    assert_eq!(
        (first, second),
        (Rgba([164, 164, 164, 0]), Rgba([146, 146, 146, 0]))
    );
}
#[test]
fn new_profiles_retain_row_cancellation_and_private_partial_output_contract() {
    use std::cell::Cell;
    let source = vec![Rgba([17, 129, 241, 0]); 130];
    let sentinel = Rgba([9; 4]);
    let mut output = vec![sentinel; 130];
    let filters = [
        Descriptor::BlackWhiteRgbV1(BlackWhiteV1::default()),
        Descriptor::ThresholdLumaV1(ThresholdLumaV1::default()),
        Descriptor::PosterizeRgbV1(PosterizeRgbV1::default()),
        Descriptor::SolarizeRgbV1,
    ];
    let calls = Cell::new(0);
    assert_eq!(
        evaluate_row(&filters, &source, &mut output, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 4 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "point pipeline stop",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert!(output[..64].iter().all(|p| *p == Rgba([113, 113, 113, 0])));
    assert!(output[64..].iter().all(|p| *p == sentinel));
    for partition in [1, 3, 64, 129] {
        for (input, out) in source.chunks(partition).zip(output.chunks_mut(partition)) {
            evaluate_row(&filters, input, out, &|| Ok(())).unwrap();
        }
        assert!(output.iter().all(|p| *p == Rgba([0, 0, 0, 0])));
    }
    assert!(source.iter().all(|p| *p == Rgba([17, 129, 241, 0])));
}
