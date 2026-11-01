use super::*;
use crate::types::json::{self, Value};

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value.get(name).expect("frozen independent oracle field")
}
fn number(value: &Value) -> u8 {
    u8::try_from(value.as_i64().expect("oracle byte integer")).expect("oracle byte domain")
}
fn rgba(value: &Value) -> Rgba {
    let array = value.as_array().expect("oracle RGBA");
    assert_eq!(array.len(), 4);
    Rgba(std::array::from_fn(|i| number(&array[i])))
}
fn mode(value: &Value) -> LayerBlendMode {
    match value.as_str().expect("oracle mode") {
        "normal" => LayerBlendMode::Normal,
        "multiply" => LayerBlendMode::Multiply,
        "screen" => LayerBlendMode::Screen,
        _ => panic!("unknown frozen oracle mode"),
    }
}
fn oracle() -> Value {
    json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/layer-blend-fixtures/oracle.json"
    )))
    .expect("independent Fraction oracle JSON")
}

#[test]
fn independent_fraction_vectors_keep_every_rounding_boundary_and_hidden_rgb() {
    let data = oracle();
    let rows = field(&data, "vectors").as_array().unwrap();
    assert_eq!(rows.len(), 18);
    let mut actual = 0;
    for row in rows {
        let source = rgba(field(row, "front"))
            .with_opacity(number(field(row, "own_mask")))
            .with_opacity(number(field(row, "opacity")));
        let backdrop = rgba(field(row, "back"));
        for case in field(row, "cases").as_array().unwrap() {
            assert_eq!(
                mode(field(case, "mode")).composite(source, backdrop),
                rgba(field(case, "rgba")),
                "{} / {}",
                field(row, "name").as_str().unwrap(),
                field(case, "mode").as_str().unwrap(),
            );
            actual += 1;
        }
    }
    assert_eq!(actual, 54);
}

#[test]
fn opaque_neutral_and_absorbing_colors_hold_across_all_byte_channels() {
    for channel in 0..=255 {
        let color = Rgba([channel, 255 - channel, channel / 2, 255]);
        let black = Rgba([0, 0, 0, 255]);
        let white = Rgba([255; 4]);
        assert_eq!(LayerBlendMode::Multiply.composite(white, color), color);
        assert_eq!(LayerBlendMode::Multiply.composite(black, color), black);
        assert_eq!(LayerBlendMode::Screen.composite(black, color), color);
        assert_eq!(LayerBlendMode::Screen.composite(white, color), white);
    }
}

#[test]
fn exact_half_channel_ties_round_up_before_presentation() {
    // Frozen Fraction channel results are253/2 and255/2. The independent
    // oracle also pins the output alpha and exact source/backdrop bytes.
    let data = oracle();
    let ties = field(&data, "exact_half_ties").as_array().unwrap();
    assert_eq!(ties.len(), 2);
    for (case, expected) in ties
        .iter()
        .zip([Rgba([127, 127, 127, 4]), Rgba([128, 128, 128, 4])])
    {
        assert_eq!(rgba(field(case, "back")), Rgba([0, 0, 0, 2]));
        assert_eq!(rgba(field(case, "front")), Rgba([254, 254, 254, 2]));
        assert_eq!(
            field(case, "exact").as_array().unwrap()[1].as_i64(),
            Some(2)
        );
        assert_eq!(
            mode(field(case, "mode"))
                .composite(rgba(field(case, "front")), rgba(field(case, "back"))),
            expected
        );
        assert_eq!(rgba(field(case, "rgba")), expected);
    }
}

#[test]
fn u32_bound_extremum_is_attained_without_overflow_or_narrowing_loss() {
    let denominator_max = 255u32.pow(3);
    let numerator_max = 255u32.pow(4);
    assert_eq!(denominator_max, 16_581_375);
    assert_eq!(numerator_max, 4_228_250_625);
    assert_eq!(
        numerator_max.checked_add(denominator_max / 2),
        Some(4_236_541_312)
    );
    // Both modes execute the maximal numerator path, including debug-overflow
    // checks when Root qualifies this test in the debug profile.
    for blend in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        for backdrop in [Rgba([255; 4]), Rgba([255, 255, 255, 0])] {
            // Covers both overlap*mixed and uncovered*255 maximal products.
            assert_eq!(blend.composite(Rgba([255; 4]), backdrop), Rgba([255; 4]));
        }
    }
}

#[test]
fn transparent_backdrop_and_zero_source_alpha_do_not_use_hidden_colors() {
    let hidden = Rgba([91, 7, 241, 0]);
    for blend in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        assert_eq!(
            blend.composite(Rgba([204, 77, 13, 128]), hidden),
            Rgba([204, 77, 13, 128])
        );
        assert_eq!(blend.composite(Rgba([204, 77, 13, 0]), hidden), hidden);
        let back = Rgba([91, 7, 241, 128]);
        assert_eq!(blend.composite(Rgba([204, 77, 13, 0]), back), back);
    }
}

#[test]
fn ordered_three_layer_literal_uses_actual_backdrop_and_each_byte_boundary() {
    let backdrop = Rgba([30, 80, 140, 255]);
    let multiply = Rgba([210, 40, 80, 128]).with_opacity(129);
    let screen = Rgba([40, 190, 230, 192]).with_opacity(173);
    let first = LayerBlendMode::Screen.composite(
        screen,
        LayerBlendMode::Multiply.composite(multiply, backdrop),
    );
    let reversed = LayerBlendMode::Multiply
        .composite(multiply, LayerBlendMode::Screen.composite(screen, backdrop));
    assert_eq!(first, Rgba([47, 136, 180, 255]));
    assert_eq!(reversed, Rgba([46, 115, 159, 255]));
}

#[test]
fn separate_mask_then_opacity_rounds_cannot_collapse_to_one_alpha_product() {
    // round(round(1*128/255)*128/255)=1; round(1*128*128/255^2)=0.
    let source = Rgba([200, 60, 10, 1]).with_opacity(128).with_opacity(128);
    assert_eq!(source, Rgba([200, 60, 10, 1]));
    for blend in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        assert_eq!(blend.composite(source, Rgba([0; 4])), source);
    }
}

#[test]
fn default_normal_keeps_existing_translucent_source_over_bytes() {
    assert_eq!(
        LayerBlendMode::default().composite(Rgba([200, 30, 180, 128]), Rgba([10, 240, 80, 128])),
        Rgba([137, 100, 147, 192])
    );
}
