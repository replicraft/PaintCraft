use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba,
    policy::{MaskIntent, MaskMode},
    resources::SharedBudget,
};
use paintcraft_tools::pen_stroke::{FrameworkPoint, PenStroke, Style};
use std::cell::Cell;
fn style() -> Style {
    Style {
        layer: uuid::Uuid::from_u128(1),
        diameter: 12,
        color: Rgba([23, 59, 151, 191]),
        mode: MaskMode::SourceOver,
        intent: MaskIntent::Brush,
        settings: Default::default(),
        media: Default::default(),
    }
}
fn point(force: f32) -> FrameworkPoint {
    FrameworkPoint {
        at: (12 * 256, 12 * 256),
        force,
    }
}
#[test]
fn framework_points_remain_separate_and_snapshots_cannot_reuse_a_changed_mask() {
    let budget = SharedBudget::new(32 << 20);
    let clip = Rect::new(0, 0, 32, 32).unwrap();
    let mut stroke = PenStroke::begin_framework(style(), point(0.2), &budget).unwrap();
    assert!(stroke.is_framework());
    assert!(stroke.samples().is_empty());
    let snapshot = stroke.clone();
    let before = snapshot
        .resolve_captured(clip, &|| Ok(()))
        .unwrap()
        .spans()
        .to_vec();
    stroke
        .append_framework(FrameworkPoint {
            at: (22 * 256, 12 * 256),
            force: 1.0,
        })
        .unwrap();
    assert_ne!(stroke, snapshot);
    assert_eq!(
        snapshot.resolve_captured(clip, &|| Ok(())).unwrap().spans(),
        before
    );
    assert_ne!(
        stroke.resolve_captured(clip, &|| Ok(())).unwrap().spans(),
        before
    );
    let calls = Cell::new(0);
    stroke
        .resolve_captured(clip, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    let retained = budget.usage().reserved;
    for stop in 1..=calls.get() {
        let count = Cell::new(0);
        let error = stroke
            .resolve_captured(clip, &|| {
                count.set(count.get() + 1);
                if count.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(budget.usage().reserved, retained);
    }
    drop((stroke, snapshot));
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn force_width_and_opacity_use_existing_policies_and_bad_axes_never_allocate_a_capture() {
    let budget = SharedBudget::new(32 << 20);
    let clip = Rect::new(0, 0, 32, 32).unwrap();
    let low = PenStroke::begin_framework(style(), point(0.25), &budget)
        .unwrap()
        .resolve_captured(clip, &|| Ok(()))
        .unwrap();
    let full = PenStroke::begin_framework(style(), point(1.0), &budget)
        .unwrap()
        .resolve_captured(clip, &|| Ok(()))
        .unwrap();
    assert!(low.spans().len() < full.spans().len());
    let mut opacity = style();
    opacity.settings.size_pressure = false;
    opacity.settings.opacity_pressure = true;
    let low = PenStroke::begin_framework(opacity, point(0.25), &budget)
        .unwrap()
        .resolve_captured(clip, &|| Ok(()))
        .unwrap();
    let full = PenStroke::begin_framework(opacity, point(1.0), &budget)
        .unwrap()
        .resolve_captured(clip, &|| Ok(()))
        .unwrap();
    assert!(low.spans().iter().map(|s| s.coverage).max().unwrap() <= 64);
    assert!(full.spans().iter().map(|s| s.coverage).max().unwrap() > 240);
    let retained = budget.usage().reserved;
    for invalid in [f32::NAN, f32::INFINITY, -0.01, 1.01] {
        assert!(PenStroke::begin_framework(style(), point(invalid), &budget).is_err());
        assert_eq!(budget.usage().reserved, retained);
    }
    assert_eq!(
        PenStroke::begin_framework(style(), point(0.5), &SharedBudget::new(0))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
