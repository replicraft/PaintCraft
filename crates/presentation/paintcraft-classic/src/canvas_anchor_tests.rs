use super::*;
use egui::{Context, Id, Pos2, RawInput, Rect, Vec2};
fn frame(ctx: &Context, scope: Id, size: Vec2, ruled: bool, tick: usize) -> Rect {
    let mut rect = Rect::NOTHING;
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
            time: Some(tick as f64 / 30.0),
            ..Default::default()
        },
        |ui| {
            let id = Id::unique("anchor-canvas");
            rect = if ruled {
                ruled_canvas_for_scope(ui, id, scope, size, (160, 100), 24.0, |_, response, _| {
                    response.rect
                })
            } else {
                canvas_for_scope(ui, id, scope, size, 24.0, |_, response, _| response.rect)
            };
        },
    );
    output.textures_delta.clear();
    rect
}
#[test]
fn anchor_survives_zoom_scrollbar_changes_and_ruled_origin() {
    for ruled in [false, true] {
        let ctx = Context::default();
        let id = Id::unique("anchor-canvas");
        let scope = Id::unique("document-new-zoom");
        let mut before = Rect::NOTHING;
        for tick in 0..8 {
            before = frame(&ctx, scope, Vec2::new(160.0, 100.0), ruled, tick);
        }
        let point = egui::pos2(80.0, 50.0);
        let target = before.min + point.to_vec2() + egui::vec2(-15.0, -10.0);
        anchor_at(&ctx, id, scope, point, target, 4.0);
        let mut after = Rect::NOTHING;
        for tick in 8..40 {
            after = frame(&ctx, scope, Vec2::new(640.0, 400.0), ruled, tick);
        }
        assert!(
            (after.min + point.to_vec2() * 4.0 - target).length() < 1.0,
            "ruled={ruled} {before:?} -> {after:?} target={target:?}"
        );
        assert!(
            ctx.data(|data| data.get_temp::<Anchor>(id.with("classic-anchor-request")))
                .is_none(),
            "anchor settles without perpetual repaint"
        );
    }
}
#[test]
fn stale_document_view_and_nonfinite_anchors_do_not_retarget_canvas() {
    let ctx = Context::default();
    let id = Id::unique("anchor-canvas");
    let old = Id::unique("old");
    let current = Id::unique("new-document-or-zoom");
    let size = Vec2::new(800.0, 600.0);
    let mut before = Rect::NOTHING;
    for tick in 0..8 {
        before = frame(&ctx, current, size, false, tick);
    }
    anchor_at(
        &ctx,
        id,
        old,
        egui::pos2(400.0, 300.0),
        egui::pos2(20.0, 20.0),
        1.0,
    );
    assert_eq!(frame(&ctx, current, size, false, 8), before);
    for (point, screen, zoom) in [
        (egui::pos2(f32::NAN, 0.0), Pos2::ZERO, 1.0),
        (Pos2::ZERO, egui::pos2(0.0, f32::INFINITY), 1.0),
        (Pos2::ZERO, Pos2::ZERO, 0.0),
        (Pos2::ZERO, Pos2::ZERO, f32::NAN),
    ] {
        anchor_at(&ctx, id, current, point, screen, zoom);
        assert_eq!(frame(&ctx, current, size, false, 9), before);
    }
}
#[test]
fn anchor_obeys_classic_small_image_clamp_and_leaves_later_pan_free() {
    let ctx = Context::default();
    let id = Id::unique("anchor-canvas");
    let scope = Id::unique("document");
    let small = Vec2::new(80.0, 60.0);
    let before = frame(&ctx, scope, small, true, 0);
    anchor_at(
        &ctx,
        id,
        scope,
        Pos2::ZERO,
        egui::pos2(-10000.0, -10000.0),
        1.0,
    );
    let mut after = Rect::NOTHING;
    for tick in 1..35 {
        after = frame(&ctx, scope, small, true, tick);
    }
    assert_eq!(
        after, before,
        "Classic does not invent overscroll for small images"
    );
    let size = Vec2::new(1000.0, 800.0);
    for tick in 35..43 {
        after = frame(&ctx, scope, size, false, tick);
    }
    anchor_at(
        &ctx,
        id,
        scope,
        egui::pos2(500.0, 400.0),
        egui::pos2(200.0, 150.0),
        1.0,
    );
    for tick in 43..77 {
        after = frame(&ctx, scope, size, false, tick);
    }
    pan_by(&ctx, id, egui::vec2(20.0, 10.0));
    let moved = frame(&ctx, scope, size, false, 77);
    assert!((moved.min - after.min - egui::vec2(20.0, 10.0)).length() < 1.0);
}

#[test]
fn explicit_pan_takes_over_immediately_even_when_queued_during_anchor_paint() {
    for during_paint in [false, true] {
        let ctx = Context::default();
        let id = Id::unique("takeover");
        let scope = Id::unique("document");
        let render = |tick: usize, pan: bool| {
            let mut rect = Rect::NOTHING;
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))),
                    time: Some(tick as f64 / 30.0),
                    ..Default::default()
                },
                |ui| {
                    rect = canvas_for_scope(
                        ui,
                        id,
                        scope,
                        egui::vec2(1000.0, 800.0),
                        24.0,
                        |ui, response, _| {
                            if pan {
                                pan_by(ui.ctx(), id, egui::vec2(20.0, 10.0));
                            }
                            response.rect
                        },
                    );
                },
            );
            output.textures_delta.clear();
            rect
        };
        for tick in 0..8 {
            render(tick, false);
        }
        anchor_at(
            &ctx,
            id,
            scope,
            egui::pos2(500.0, 400.0),
            egui::pos2(200.0, 150.0),
            1.0,
        );
        let before = render(8, during_paint);
        if !during_paint {
            pan_by(&ctx, id, egui::vec2(20.0, 10.0));
        }
        let after = render(9, false);
        assert!(
            (after.min - before.min - egui::vec2(20.0, 10.0)).length() < 1.0,
            "during_paint={during_paint}: {before:?}->{after:?}"
        );
        assert!(
            ctx.data(|data| data.get_temp::<Anchor>(id.with("classic-anchor-request")))
                .is_none()
        );
    }
}

#[test]
fn anchor_retires_already_started_scroll_animation_at_120hz() {
    let ctx = Context::default();
    let id = Id::unique("animated-anchor");
    let scope = Id::unique("document");
    let render = |tick: usize, animate: bool| {
        let mut rect = Rect::NOTHING;
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))),
                time: Some(tick as f64 / 120.0),
                ..Default::default()
            },
            |ui| {
                rect = canvas_for_scope(
                    ui,
                    id,
                    scope,
                    egui::vec2(1000.0, 800.0),
                    24.0,
                    |ui, response, _| {
                        if animate {
                            ui.scroll_to_rect_animation(
                                Rect::from_center_size(
                                    response.rect.min + egui::vec2(800.0, 600.0),
                                    Vec2::ZERO,
                                ),
                                Some(egui::Align::Center),
                                egui::style::ScrollAnimation::duration(1.0),
                            );
                        }
                        response.rect
                    },
                );
            },
        );
        output.textures_delta.clear();
        rect
    };
    let mut before = Rect::NOTHING;
    for tick in 0..8 {
        before = render(tick, false);
    }
    render(8, true);
    let mut moving = Rect::NOTHING;
    for tick in 9..33 {
        moving = render(tick, false);
    }
    assert!(
        (moving.min - before.min).length() > 2.0,
        "old animation must already be moving"
    );
    let point = egui::pos2(500.0, 400.0);
    let target = egui::pos2(200.0, 150.0);
    anchor_at(&ctx, id, scope, point, target, 1.0);
    for tick in 33..200 {
        let actual = render(tick, false).min + point.to_vec2();
        assert!(
            actual.distance(target) < 1.0,
            "frame={tick}: {actual:?}!={target:?}"
        );
        if tick == 37 {
            assert!(
                ctx.data(|data| data.get_temp::<Anchor>(id.with("classic-anchor-request")))
                    .is_none(),
                "old target must retire without waiting a32frame cap"
            );
        }
    }
}
