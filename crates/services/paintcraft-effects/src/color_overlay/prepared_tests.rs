use super::*;
use prepared::{PreparedColorOverlay, quote};
use serde_json::Value;
use std::cell::{Cell, RefCell};
fn byte(v: &Value) -> u8 {
    u8::try_from(v.as_u64().unwrap()).unwrap()
}
fn tuple(v: &Value) -> Rgba {
    Rgba(std::array::from_fn(|i| byte(&v[i])))
}
#[test]
fn prepared_scalar_and_both_row_views_match_all6912_independent_literals() {
    let raw = include_bytes!("../../tests/color-overlay-fixtures/oracle.json");
    let o: Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(o["records"].as_array().unwrap().len(), 6912);
    for row in o["records"].as_array().unwrap() {
        let policy = ColorOverlayRgbV1::new(
            std::array::from_fn(|i| byte(&row[3][i])),
            byte(&row[2]),
            EffectBlendMode::from_code(byte(&row[1])).unwrap(),
        );
        let input = tuple(&row[4]);
        let expected = tuple(&row[5]);
        let p = PreparedColorOverlay::new(policy);
        assert_eq!(p.pixel(input), expected, "record {row}");
        assert_eq!(p.pixel(input), evaluate_pixel(policy, input).unwrap());
        let mut tuples = [input];
        p.pixels_in_place(&mut tuples, &|| Ok(())).unwrap();
        assert_eq!(tuples, [expected]);
        let mut bytes = input.0;
        p.bytes_in_place(&mut bytes, &|| Ok(())).unwrap();
        assert_eq!(bytes, expected.0);
        assert_eq!(p.pixel(input).a(), input.a());
    }
}
#[test]
fn row63_64_and_empty_have_exact_callbacks_guards_and_geometry_refusal() {
    let p = PreparedColorOverlay::new(ColorOverlayRgbV1::new(
        [220, 40, 0],
        128,
        EffectBlendMode::Normal,
    ));
    for count in [0, 1, 63, 64] {
        let mut bytes = vec![199; count * 4 + 8];
        bytes[4..4 + count * 4]
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .for_each(|v| v.copy_from_slice(&[20, 80, 200, 127]));
        let calls = Cell::new(0);
        p.bytes_in_place(&mut bytes[4..4 + count * 4], &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(calls.get(), if count == 0 { 2 } else { 3 });
        assert_eq!(&bytes[..4], &[199; 4]);
        assert_eq!(&bytes[4 + count * 4..], &[199; 4]);
        for out in bytes[4..4 + count * 4].as_chunks::<4>().0 {
            assert_eq!(out, &[120, 60, 100, 127]);
        }
    }
    for n in [3, 65 * 4] {
        let mut bytes = vec![199; n];
        let before = bytes.clone();
        assert_eq!(
            p.bytes_in_place(&mut bytes, &|| panic!("geometry callback"))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
        assert_eq!(bytes, before);
    }
    let mut tuples = [Rgba([199; 4]); 65];
    assert_eq!(
        p.pixels_in_place(&mut tuples, &|| panic!("geometry callback"))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
    assert_eq!(tuples, [Rgba([199; 4]); 65]);
}
#[test]
fn every_callback_cut_returns_original_error_and_private_output_is_never_a_source() {
    let p = PreparedColorOverlay::new(ColorOverlayRgbV1::new(
        [220, 40, 0],
        128,
        EffectBlendMode::Normal,
    ));
    let source = [Rgba([20, 80, 200, 0]); 2];
    for fail in 1..=3 {
        let original = Error::new(
            ErrorCode::OperationCancelled,
            "original prepared row cancellation",
        );
        let ptr = original.message.as_ptr();
        let held = RefCell::new(Some(original));
        let calls = Cell::new(0);
        let mut output = source;
        let error = p
            .pixels_in_place(&mut output, &|| {
                calls.set(calls.get() + 1);
                if calls.get() == fail {
                    Err(held.borrow_mut().take().unwrap())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.message.as_ptr(), ptr);
        assert_eq!(source, [Rgba([20, 80, 200, 0]); 2]);
        assert_eq!(
            output,
            if fail < 3 {
                source
            } else {
                [Rgba([120, 60, 100, 0]); 2]
            }
        );
    }
}
#[test]
fn defining_layout_and_finite_quote_do_not_allocate_budget_or_grant_work() {
    use paintcraft_core::types::resources::SharedBudget;
    let original = SharedBudget::new(8192);
    let before = original.usage();
    let named = prepared::named_control_bytes().unwrap();
    assert!(named <= 1024);
    assert_eq!(std::mem::size_of::<PreparedColorOverlay>(), 24);
    assert_eq!(
        quote(0).unwrap(),
        prepared::Quote {
            work: 4096,
            control_bytes: 1024
        }
    );
    assert_eq!(
        quote(64).unwrap(),
        prepared::Quote {
            work: 20480,
            control_bytes: 1024
        }
    );
    assert_eq!(quote(65).unwrap_err().code, ErrorCode::InputSchema);
    assert_eq!(original.usage(), before);
}
