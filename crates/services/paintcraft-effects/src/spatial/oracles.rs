//! Actual product outputs against the separately authored Fraction reference.
use super::*;
use paintcraft_core::types::{
    json::{self, Value},
    policy::hex,
};
use sha2::{Digest, Sha256};

fn oracle() -> Value {
    json::parse(include_bytes!("../../tests/box-fixtures/oracle.json")).unwrap()
}
fn field<'a>(row: &'a Value, key: &str) -> &'a Value {
    row.get(key).unwrap()
}
fn number(row: &Value) -> i64 {
    row.as_i64().unwrap()
}
fn array(row: &Value) -> &[Value] {
    row.as_array().unwrap()
}
fn bytes(row: &Value) -> Vec<u8> {
    array(row)
        .iter()
        .map(|v| u8::try_from(number(v)).unwrap())
        .collect()
}
fn rgba(row: &Value) -> Rgba {
    Rgba(bytes(row).try_into().unwrap())
}
fn rect(row: &Value) -> Rect {
    let values = array(row);
    Rect::new(
        number(&values[0]),
        number(&values[1]),
        number(&values[2]),
        number(&values[3]),
    )
    .unwrap()
}
fn render(source: Rect, output: Rect, raw: &[u8], radius: u8) -> Vec<Rgba> {
    let plan =
        BoxBlurPlan::new(source, output, BoxBlurV1::new(i16::from(radius)).unwrap()).unwrap();
    let input_rect = plan.required_input();
    let mut input = Vec::new();
    for y in input_rect.y0..input_rect.y1 {
        for x in input_rect.x0..input_rect.x1 {
            let i = ((y - source.y0) * source.width() + (x - source.x0)) as usize * 4;
            input.push(Rgba(raw[i..i + 4].try_into().unwrap()));
        }
    }
    let mut output = vec![Rgba([199; 4]); plan.output_pixels()];
    box_blur(
        plan,
        &input,
        &mut output,
        &mut vec![u64::MAX; plan.scratch_elements()],
        &|| Ok(()),
    )
    .unwrap();
    output
}
fn flatten(pixels: &[Rgba]) -> Vec<u8> {
    pixels.iter().flat_map(|pixel| pixel.0).collect()
}
fn source_rect(row: &Value) -> Rect {
    let origin = array(field(row, "origin"));
    let size = array(field(row, "size"));
    Rect::new(
        number(&origin[0]),
        number(&origin[1]),
        number(&size[0]),
        number(&size[1]),
    )
    .unwrap()
}

#[test]
fn independent_literal_planes_partitions_and_alpha_aware_masking() {
    let oracle = oracle();
    let literals = array(field(&oracle, "literals"));
    assert_eq!(literals.len(), 15);
    for row in literals {
        let source = source_rect(row);
        let raw = bytes(field(row, "rgba"));
        let radius = number(field(row, "radius")) as u8;
        let output = render(source, rect(field(row, "region")), &raw, radius);
        assert_eq!(
            flatten(&output),
            bytes(field(row, "expected")),
            "{:?}",
            field(row, "name")
        );
        for partition in array(field(row, "partitions")) {
            let output = render(source, rect(field(partition, "region")), &raw, radius);
            assert_eq!(flatten(&output), bytes(field(partition, "expected")));
        }
    }
    let masks = array(field(&oracle, "mask_literals"));
    assert_eq!(masks.len(), 45);
    for row in masks {
        assert_eq!(
            mix_spatial_coverage(
                rgba(field(row, "incoming")),
                rgba(field(row, "candidate")),
                number(field(row, "coverage")) as u8
            ),
            rgba(field(row, "expected"))
        );
    }
    let planes = array(field(&oracle, "masked_planes"));
    assert_eq!(planes.len(), 12);
    for row in planes {
        let source = source_rect(row);
        let raw = bytes(field(row, "rgba"));
        let candidate = render(source, source, &raw, number(field(row, "radius")) as u8);
        assert_eq!(flatten(&candidate), bytes(field(row, "rounded_candidate")));
        let output: Vec<_> = raw
            .as_chunks::<4>()
            .0
            .iter()
            .zip(candidate)
            .map(|(p, q)| mix_spatial_coverage(Rgba(*p), q, number(field(row, "coverage")) as u8))
            .collect();
        assert_eq!(flatten(&output), bytes(field(row, "expected")));
    }
}

#[derive(Default)]
struct Stream {
    hash: Sha256,
    records: i64,
    bytes: i64,
}
impl Stream {
    fn add(&mut self, record: &[u8]) {
        self.hash.update(record);
        self.records += 1;
        self.bytes += record.len() as i64;
    }
    fn finish(self, oracle: &Value, name: &str) -> i64 {
        let expected = field(field(oracle, "streams"), name);
        assert_eq!(self.records, number(field(expected, "records")));
        assert_eq!(self.bytes, number(field(expected, "bytes")));
        assert_eq!(
            hex(&self.hash.finalize()),
            field(expected, "sha256").as_str().unwrap(),
            "{name}"
        );
        self.records
    }
}

#[test]
fn all_54726_product_records_match_independent_frozen_streams() {
    let oracle = oracle();
    let source = Rect::new(0, 0, 1, 1).unwrap();
    let mut one = Stream::default();
    for radius in 0..=32 {
        for alpha in 0..=255 {
            let pixel = render(source, source, &[17, 93, 201, alpha], radius)[0];
            one.add(&[
                radius, alpha, pixel.0[0], pixel.0[1], pixel.0[2], pixel.0[3],
            ]);
        }
    }
    let mut count = one.finish(&oracle, "one_by_one_all_alphas_all_radii");
    let recipe = field(&oracle, "source_recipe");
    let mut edges = Stream::default();
    let source = Rect::new(0, 0, 2, 1).unwrap();
    for radius in 0..=32 {
        for (index, raw) in array(field(recipe, "edge_pairs")).iter().enumerate() {
            for (x, pixel) in render(source, source, &bytes(raw), radius)
                .iter()
                .enumerate()
            {
                edges.add(&[
                    radius,
                    index as u8,
                    x as u8,
                    pixel.0[0],
                    pixel.0[1],
                    pixel.0[2],
                    pixel.0[3],
                ]);
            }
        }
    }
    count += edges.finish(&oracle, "two_pixel_edges_all_radii");
    let mut masked = Stream::default();
    for (index, pair) in array(field(recipe, "mask_rgb_pairs")).iter().enumerate() {
        let pair = array(pair);
        let p = bytes(&pair[0]);
        let q = bytes(&pair[1]);
        for ap in 0..=255 {
            for aq in [0, 1, 127, 128, 255] {
                for m in [0, 1, 64, 85, 127, 128, 170, 254, 255] {
                    let pixel = mix_spatial_coverage(
                        Rgba([p[0], p[1], p[2], ap]),
                        Rgba([q[0], q[1], q[2], aq]),
                        m,
                    );
                    masked.add(&[
                        index as u8,
                        ap,
                        aq,
                        m,
                        pixel.0[0],
                        pixel.0[1],
                        pixel.0[2],
                        pixel.0[3],
                    ]);
                }
            }
        }
    }
    count += masked.finish(&oracle, "masked_all_input_alphas_selected_candidate_alphas");
    assert_eq!(count, 54_726);
}
