//! Source7 bounded view-job regression. Original90864 red log remains immutable.
use super::*;
use sha2::{Digest, Sha256};
const WIDTH: u32 = 1000;
const HEIGHT: u32 = 486;
const GENERATIONS: u64 = 20;
fn record(value: Value) {
    println!(
        "SOURCE7_VIEW_DIAGNOSTIC {}",
        String::from_utf8(paintcraft_core::types::json::canonical(&value)).unwrap()
    );
}
fn amount(value: u64) -> Value {
    Value::str(value.to_string())
}
fn document(value: u8, enabled: bool) -> Document {
    let mut doc = paintcraft_document::new_document(
        WIDTH,
        HEIGHT,
        Some(Rgba([20, 115, 200, 128])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.mask = Some(LayerMask::new(
        enabled,
        A8Mask::uniform(
            Rect::new(0, 0, WIDTH.into(), HEIGHT.into()).unwrap(),
            value,
            &SharedBudget::new(1 << 20),
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap(),
    ));
    layer.opacity = 128;
    doc
}
fn diagnostic(name: &str, value: u8, enabled: bool, expected: [u8; 4], pan: bool) {
    let doc = document(value, enabled);
    let (source_bytes, control) = source(&doc, admission(false));
    let mut service = LayerAppearanceService::open(&source_bytes, &text(&control)).unwrap();
    let source_sha = hex(&Sha256::digest(&source_bytes));
    let area = Rect::new(0, 0, WIDTH.into(), HEIGHT.into()).unwrap();
    let actual = paintcraft_render::layer_mask::quote_work(
        &service.state.base.layers[&service.state.base.roots[0]],
        area,
    )
    .unwrap();
    record(Value::object([
        ("phase", Value::str("open")),
        ("scenario", Value::str(name)),
        ("sourceSha256", Value::str(&source_sha)),
        ("width", Value::Int(WIDTH.into())),
        ("height", Value::Int(HEIGHT.into())),
        ("requestedGenerations", amount(GENERATIONS)),
        (
            "viewPolicy",
            Value::str(if pan {
                "alternate-x0-0-or1"
            } else {
                "constant-full"
            }),
        ),
        (
            "baseLimit",
            amount(service.state.job.workspace().work_limit()),
        ),
        (
            "workUsed",
            amount(service.state.job.workspace().work_used()),
        ),
        ("pureWholeMaskWorkActual", amount(actual)),
    ]));
    let mut last_accepted_quote = Value::Null;
    for generation in 1..=GENERATIONS {
        let x0 = u32::from(pan && generation % 2 == 0);
        let width = WIDTH - x0;
        let local = Rect::new(x0.into(), 0, width.into(), HEIGHT.into()).unwrap();
        let mask_work = paintcraft_render::layer_mask::quote_work(
            &service.state.base.layers[&service.state.base.roots[0]],
            local,
        )
        .unwrap();
        let mut request = request(&service, generation, true, Value::Null);
        set(
            &mut request,
            "view",
            Value::object([
                (
                    "region",
                    Value::Array(vec![
                        Value::Int(x0.into()),
                        Value::Int(0),
                        Value::Int(width.into()),
                        Value::Int(HEIGHT.into()),
                    ]),
                ),
                ("level", Value::Int(0)),
            ]),
        );
        record(Value::object([
            ("phase", Value::str("requested-view")),
            ("scenario", Value::str(name)),
            ("generation", amount(generation)),
            ("x0", Value::Int(x0.into())),
            ("x1", Value::Int(WIDTH.into())),
            ("width", Value::Int(width.into())),
            ("pureMaskWorkActual", amount(mask_work)),
        ]));
        let before = service.state.job.workspace().work_used();
        let result = service.prepare(&text(&request)).cloned();
        let after_prepare = service.state.job.workspace().work_used();
        match result {
            Ok(binding) => {
                last_accepted_quote = field(&binding, "workQuote").unwrap().clone();
                record(Value::object([
                    ("phase", Value::str("prepared")),
                    ("scenario", Value::str(name)),
                    ("generation", amount(generation)),
                    ("before", amount(before)),
                    ("after", amount(after_prepare)),
                    ("quote", last_accepted_quote.clone()),
                ]));
            }
            Err(error) => {
                assert_eq!(pcnd::write(&service.state.base).unwrap(), source_bytes);
                record(Value::object([
                    ("phase", Value::str("first-failure")),
                    ("at", Value::str("prepare")),
                    ("scenario", Value::str(name)),
                    ("generation", amount(generation)),
                    ("before", amount(before)),
                    ("after", amount(after_prepare)),
                    ("code", Value::str(error.code.as_str())),
                    ("message", Value::str(&error.message)),
                    ("lastAcceptedQuote", last_accepted_quote),
                    ("sourceUnchanged", Value::Bool(true)),
                ]));
                panic!(
                    "Source7 availability diagnostic: {name} refused view {generation}: {error}"
                );
            }
        }
        let mut complete = false;
        for step in 1..=1024 {
            match service.step() {
                Ok(done) => {
                    if done {
                        complete = true;
                        record(Value::object([
                            ("phase", Value::str("rendered")),
                            ("scenario", Value::str(name)),
                            ("generation", amount(generation)),
                            ("steps", Value::Int(step)),
                            (
                                "workUsed",
                                amount(service.state.job.workspace().work_used()),
                            ),
                        ]));
                        break;
                    }
                }
                Err(error) => {
                    assert_eq!(pcnd::write(&service.state.base).unwrap(), source_bytes);
                    record(Value::object([
                        ("phase", Value::str("first-failure")),
                        ("at", Value::str("step")),
                        ("scenario", Value::str(name)),
                        ("generation", amount(generation)),
                        ("step", Value::Int(step)),
                        ("beforePrepare", amount(before)),
                        ("afterPrepare", amount(after_prepare)),
                        (
                            "workUsed",
                            amount(service.state.job.workspace().work_used()),
                        ),
                        ("code", Value::str(error.code.as_str())),
                        ("message", Value::str(&error.message)),
                        ("acceptedQuote", last_accepted_quote),
                        ("sourceUnchanged", Value::Bool(true)),
                    ]));
                    panic!(
                        "Source7 availability diagnostic: {name} step failed view {generation}: {error}"
                    );
                }
            }
        }
        assert!(complete, "Source7 diagnostic exceeds finite1024-step bound");
        let packet = service.take_frame().unwrap();
        assert_eq!(packet.pixels().len(), width as usize * HEIGHT as usize * 4);
        assert!(
            packet
                .pixels()
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel == &expected)
        );
        let rgba_sha = hex(&Sha256::digest(packet.pixels()));
        drop(packet); // Completed owner MUST be dropped before the next generation.
        assert_eq!(service.baseline.strong_count(), 1);
        assert_eq!(service.controls.strong_count(), 1);
        assert_eq!(pcnd::write(&service.state.base).unwrap(), source_bytes);
        record(Value::object([
            ("phase", Value::str("released")),
            ("scenario", Value::str(name)),
            ("generation", amount(generation)),
            ("rgbaSha256", Value::str(rgba_sha)),
            (
                "workUsed",
                amount(service.state.job.workspace().work_used()),
            ),
            ("sourceUnchanged", Value::Bool(true)),
        ]));
    }
    record(Value::object([
        ("phase", Value::str("completed-requested-diagnostic")),
        ("scenario", Value::str(name)),
        ("generations", amount(GENERATIONS)),
        (
            "workUsed",
            amount(service.state.job.workspace().work_used()),
        ),
        ("qualificationClaim", Value::Bool(false)),
    ]));
}
#[test]
fn open_view_lifetime_uniform128() {
    diagnostic("uniform128-constant", 128, true, [20, 115, 200, 32], false);
}
#[test]
fn open_view_lifetime_enabled_zero() {
    diagnostic("enabled-zero-constant", 0, true, [0, 0, 0, 0], false);
}
#[test]
fn open_view_lifetime_disabled_some() {
    diagnostic(
        "disabled-Some-constant",
        128,
        false,
        [20, 115, 200, 64],
        false,
    );
}

#[test]
fn open_view_lifetime_uniform128_pan() {
    diagnostic("uniform128-pan", 128, true, [20, 115, 200, 32], true);
}
#[test]
fn open_view_lifetime_enabled_zero_pan() {
    diagnostic("enabled-zero-pan", 0, true, [0, 0, 0, 0], true);
}
#[test]
fn open_view_lifetime_disabled_some_pan() {
    diagnostic("disabled-Some-pan", 128, false, [20, 115, 200, 64], true);
}
