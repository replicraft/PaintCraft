//! One immutable-source plan: full independent RGBA and exact read allowance.
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba,
    json::{self, Value},
    policy::BlendMode,
    rotation::Rotation,
};
use paintcraft_raster::{Admission, MappedPlan, PaintSnapshot, RasterLayer, RotationMapping};
use std::cell::Cell;
fn a(v: &Value) -> &[Value] {
    v.as_array().unwrap()
}
fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap()
}
fn n(v: &Value) -> i64 {
    v.as_i64().unwrap()
}
fn rect(v: &Value) -> Rect {
    let x = a(v);
    Rect {
        x0: n(&x[0]),
        y0: n(&x[1]),
        x1: n(&x[2]),
        y1: n(&x[3]),
    }
}
fn bytes(v: &Value) -> Vec<u8> {
    a(v).iter().map(|v| u8::try_from(n(v)).unwrap()).collect()
}
#[test]
fn immutable_all_layer_oracles_exact_minus_one_and_cancellation() {
    let fixture = json::parse(include_bytes!(
        "../../../../tests/rotation-q30-fixtures/images.json"
    ))
    .unwrap();
    for case in a(g(&fixture, "whole")) {
        let c = a(g(case, "coefficients"));
        let geometry = Rotation::new(
            Rect::new(0, 0, 4, 3).unwrap(),
            (n(&c[0]) as i32, n(&c[1]) as i32),
        )
        .unwrap();
        for layer in a(g(case, "layers")) {
            let s = g(layer, "source");
            let extent = rect(g(s, "extent"));
            let raw = bytes(g(s, "rgba"));
            let default = bytes(g(s, "default"));
            let mut r = RasterLayer::new(
                extent.width() as u32,
                extent.height() as u32,
                Rgba(default.try_into().unwrap()),
            );
            let mut input = Admission::new(65536);
            for y in 0..extent.height() {
                for x in 0..extent.width() {
                    let i = ((y * extent.width() + x) * 4) as usize;
                    r.blend_pixel(
                        x,
                        y,
                        Rgba(raw[i..i + 4].try_into().unwrap()),
                        BlendMode::Replace,
                        &mut input,
                    )
                    .unwrap();
                }
            }
            let destination = rect(g(layer, "destination"));
            let visits = (destination.width() * destination.height()) as u64;
            let request = RotationMapping {
                transform: geometry,
                source_origin: (extent.x0, extent.y0),
                destination,
                selection: None,
                clear: Rgba::TRANSPARENT,
            };
            let admission = Admission::new(65536);
            let snapshot = PaintSnapshot::capture(&r, &admission, &|| Ok(())).unwrap();
            assert_eq!(
                MappedPlan::prepare_rotation(
                    snapshot.clone(),
                    request.clone(),
                    &admission,
                    visits - 1,
                    &|| Ok(())
                )
                .unwrap_err()
                .code,
                ErrorCode::ResourceLimit
            );
            let plan = MappedPlan::prepare_rotation(
                snapshot.clone(),
                request.clone(),
                &admission,
                visits,
                &|| Ok(()),
            )
            .unwrap();
            assert_eq!(plan.source_reads(), visits);
            let result = plan.replay(&mut Admission::new(65536), &|| Ok(())).unwrap();
            let mut actual = Vec::new();
            for y in 0..result.height() {
                for x in 0..result.width() {
                    actual.extend(result.get(i64::from(x), i64::from(y)).0)
                }
            }
            assert_eq!(actual, bytes(g(layer, "rgba")));
            for cut in 1..=3 {
                let calls = Cell::new(0);
                let check = || {
                    calls.set(calls.get() + 1);
                    if calls.get() == cut {
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "original callback",
                        ))
                    } else {
                        Ok(())
                    }
                };
                assert_eq!(
                    MappedPlan::prepare_rotation(
                        snapshot.clone(),
                        request.clone(),
                        &admission,
                        visits,
                        &check
                    )
                    .unwrap_err()
                    .code,
                    ErrorCode::OperationCancelled
                );
            }
            let mut unchanged = Vec::new();
            for y in 0..r.height() {
                for x in 0..r.width() {
                    unchanged.extend(r.get(i64::from(x), i64::from(y)).0)
                }
            }
            assert_eq!(unchanged, raw);
        }
    }
}
