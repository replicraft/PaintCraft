use super::*;
use egui::{Context, Id, Pos2, RawInput, Rect, Vec2};
fn frame(ctx: &Context, scope: Id, size: Vec2, overscroll: bool, tick: usize) -> Rect {
    let mut rect = Rect::NOTHING;
    let mut output = ctx.run_ui(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
            time: Some(tick as f64 / 30.0),
            ..Default::default()
        },
        |ui| {
            rect = canvas_for_profile(
                ui,
                Id::unique("anchor-canvas"),
                scope,
                size,
                overscroll,
                |_, response, _| response.rect,
            );
        },
    );
    output.textures_delta.clear();
    rect
}
#[test]
fn anchor_tracks_new_zoom_and_translation_with_both_overscroll_profiles() {
    for overscroll in [false, true] {
        let ctx = Context::default();
        let id = Id::unique("anchor-canvas");
        let scope = Id::unique("document-new-zoom");
        let mut before = Rect::NOTHING;
        for tick in 0..8 {
            before = frame(&ctx, scope, Vec2::new(200.0, 100.0), overscroll, tick);
        }
        let point = egui::pos2(50.0, 30.0);
        let target = before.min + point.to_vec2() + egui::vec2(-25.0, -15.0);
        anchor_at(&ctx, id, scope, point, target, 4.0);
        let mut after = Rect::NOTHING;
        for tick in 8..40 {
            after = frame(&ctx, scope, Vec2::new(800.0, 400.0), overscroll, tick);
        }
        assert!(
            (after.min + point.to_vec2() * 4.0 - target).length() < 1.0,
            "overscroll={overscroll} {before:?} -> {after:?} target={target:?}"
        );
        assert!(
            ctx.data(|data| data.get_temp::<anchor::Anchor>(id.with("studio-anchor-request")))
                .is_none()
        );
    }
}
#[test]
fn stale_scope_and_nonfinite_requests_leave_the_existing_view_unchanged() {
    let ctx = Context::default();
    let id = Id::unique("anchor-canvas");
    let scope = Id::unique("current");
    let size = Vec2::new(1000.0, 800.0);
    let mut before = Rect::NOTHING;
    for tick in 0..8 {
        before = frame(&ctx, scope, size, true, tick);
    }
    anchor_at(
        &ctx,
        id,
        Id::unique("old-document-or-zoom"),
        egui::pos2(500.0, 400.0),
        Pos2::ZERO,
        1.0,
    );
    assert_eq!(frame(&ctx, scope, size, true, 8), before);
    for (point, screen, zoom) in [
        (egui::pos2(f32::INFINITY, 0.0), Pos2::ZERO, 1.0),
        (Pos2::ZERO, egui::pos2(0.0, f32::NAN), 1.0),
        (Pos2::ZERO, Pos2::ZERO, -1.0),
        (Pos2::ZERO, Pos2::ZERO, f32::INFINITY),
    ] {
        anchor_at(&ctx, id, scope, point, screen, zoom);
        assert_eq!(frame(&ctx, scope, size, true, 9), before);
    }
}
#[test]
fn anchors_respect_owner_clamp_and_do_not_override_later_pan() {
    let ctx = Context::default();
    let id = Id::unique("anchor-canvas");
    let scope = Id::unique("document");
    let small = Vec2::new(80.0, 60.0);
    let before = frame(&ctx, scope, small, false, 0);
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
        after = frame(&ctx, scope, small, false, tick);
    }
    assert_eq!(
        after, before,
        "disabled overscroll retains small-image clamp"
    );
    let size = Vec2::new(1000.0, 800.0);
    for tick in 35..43 {
        after = frame(&ctx, scope, size, true, tick);
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
        after = frame(&ctx, scope, size, true, tick);
    }
    pan_by(&ctx, id, scope, egui::vec2(20.0, 10.0));
    let moved = frame(&ctx, scope, size, true, 77);
    assert!((moved.min - after.min - egui::vec2(20.0, 10.0)).length() < 1.0);
}

#[test]
fn large_image_anchor_uses_current_viewport_for_a_new_zoom_scope() {
    for floating in [false, true] {
        let ctx = Context::default();
        ctx.all_styles_mut(|style| style.spacing.scroll.floating = floating);
        let id = Id::unique("large-anchor");
        let mut tick = 0;
        let mut render = |zoom: f32| {
            let mut rect = Rect::NOTHING;
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(600.0, 800.0))),
                    time: Some(tick as f64 / 30.0),
                    ..Default::default()
                },
                |ui| {
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(Rect::from_min_max(
                            egui::pos2(0.0, 130.0),
                            egui::pos2(600.0, 727.0),
                        )),
                        |ui| {
                            rect = canvas_for_scope(
                                ui,
                                id,
                                Id::unique(zoom.to_bits()),
                                egui::vec2(1000.0, 486.0) * zoom,
                                |_, response, _| response.rect,
                            );
                        },
                    );
                },
            );
            tick += 1;
            output.textures_delta.clear();
            rect
        };
        let mut before = Rect::NOTHING;
        for _ in 0..8 {
            before = render(4.0);
        }
        let point = egui::pos2(500.0, 220.1);
        let start = before.min + point.to_vec2() * 4.0;
        anchor_at(&ctx, id, Id::unique(4f32.to_bits()), point, start, 4.0);
        let mut initial = Rect::NOTHING;
        for _ in 0..8 {
            initial = render(4.0);
        }
        assert!(
            (initial.min - before.min).length() < 1.0,
            "first anchor shifted image: floating={floating} {before:?} -> {initial:?}"
        );
        let target = start + egui::vec2(-10.0, 20.0);
        anchor_at(&ctx, id, Id::unique(8f32.to_bits()), point, target, 8.0);
        let mut after = Rect::NOTHING;
        for _ in 0..32 {
            after = render(8.0);
        }
        assert!(
            (after.min + point.to_vec2() * 8.0 - target).length() < 1.0,
            "floating={floating} {before:?} -> {after:?} target={target:?}"
        );
    }
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
                        |ui, response, _| {
                            if pan {
                                pan_by(ui.ctx(), id, scope, egui::vec2(20.0, 10.0));
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
            pan_by(&ctx, id, scope, egui::vec2(20.0, 10.0));
        }
        let after = render(9, false);
        assert!(
            (after.min - before.min - egui::vec2(20.0, 10.0)).length() < 1.0,
            "during_paint={during_paint}: {before:?}->{after:?}"
        );
        assert!(
            ctx.data(|data| data.get_temp::<anchor::Anchor>(id.with("studio-anchor-request")))
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
                ctx.data(|data| data.get_temp::<anchor::Anchor>(id.with("studio-anchor-request")))
                    .is_none(),
                "old target must retire without waiting a32frame cap"
            );
        }
    }
}
