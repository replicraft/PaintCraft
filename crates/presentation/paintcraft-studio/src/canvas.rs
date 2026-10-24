//! Studio canvas presentation. Layered navigation is independent of Classic.
mod geometry;
use geometry::Geometry;

#[derive(Clone, Copy)]
struct Layout {
    outer: egui::Vec2,
    inner: egui::Vec2,
    geometry: Geometry,
}

pub fn canvas<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    size: egui::Vec2,
    paint: impl FnOnce(&mut egui::Ui, egui::Response, egui::Painter) -> R,
) -> R {
    canvas_for_scope(ui, id, id, size, paint)
}

/// Scoped requests cannot apply to a replaced document or a changed view.
pub fn canvas_for_scope<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    scope: egui::Id,
    size: egui::Vec2,
    paint: impl FnOnce(&mut egui::Ui, egui::Response, egui::Painter) -> R,
) -> R {
    canvas_for_profile(ui, id, scope, size, true, paint)
}

/// Overscroll is a frontend preference, separate from canvas/document dimensions.
pub fn canvas_for_profile<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    scope: egui::Id,
    size: egui::Vec2,
    overscroll: bool,
    paint: impl FnOnce(&mut egui::Ui, egui::Response, egui::Painter) -> R,
) -> R {
    let viewport = ui.available_rect_before_wrap();
    ui.scope_builder(
        egui::UiBuilder::new()
            .scope_id(id.with("studio-canvas-presentation"))
            .max_rect(viewport),
        |ui| {
            // Keep navigation hints from changing the displayed document colors.
            ui.spacing_mut().scroll.fade.strength = 0.0;
            let pending = id.with("studio-pan-request");
            let offset = id.with("studio-pan-offset");
            let delta = ui.data_mut(|data| {
                let request = data.get_temp::<(egui::Id, egui::Vec2)>(pending);
                data.remove::<(egui::Id, egui::Vec2)>(pending);
                request
            });
            let layout_id = id.with("studio-canvas-geometry");
            let previous = ui.data(|data| data.get_temp::<Layout>(layout_id));
            let outer = viewport.size();
            let inner = previous.map_or(outer, |old| outer - (old.outer - old.inner));
            let geometry = Geometry::new(inner, size, overscroll);
            let old_offset = ui
                .data(|data| data.get_temp::<egui::Vec2>(offset))
                .unwrap_or_default();
            let mut position = previous.map_or(geometry.initial, |old| {
                geometry.preserve_center(old.geometry, old_offset)
            });
            let mut force_position = previous.is_none_or(|old| old.geometry != geometry);
            if let Some((source, delta)) = delta
                && source == scope
            {
                position = geometry.clamp(position - delta);
                force_position = true;
            }
            let mut area = egui::ScrollArea::both()
                .id_salt(("studio-canvas", id))
                .auto_shrink([false, false])
                .scroll_source(egui::scroll_area::ScrollSource {
                    drag: egui::scroll_area::DragScroll::Never,
                    ..Default::default()
                });
            if force_position {
                area = area.scroll_offset(position);
            }
            let output = area.show_viewport(ui, |ui, view| {
                let actual = Geometry::new(view.size(), size, overscroll);
                let (content, _) = ui.allocate_exact_size(actual.extent, egui::Sense::hover());
                let rect = egui::Rect::from_min_size(content.min + actual.leading, size);
                let response = ui.interact(
                    rect,
                    id.with("studio-canvas-image"),
                    egui::Sense::click_and_drag(),
                );
                let painter = ui.painter_at(rect);
                ui.painter().rect_filled(
                    rect.expand(1.0),
                    0,
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                );
                (paint(ui, response, painter), actual)
            });
            let (result, actual) = output.inner;
            ui.data_mut(|data| {
                data.insert_temp(offset, output.state.clamped_offset());
                data.insert_temp(
                    layout_id,
                    Layout {
                        outer,
                        inner: output.inner_rect.size(),
                        geometry,
                    },
                );
            });
            if (actual.viewport - geometry.viewport).length() > 0.5 {
                ui.ctx().request_repaint();
            }
            result
        },
    )
    .inner
}

/// Queue logical-screen displacement. Scroll area bounds apply on the next frame.
pub fn pan_by(ctx: &egui::Context, id: egui::Id, scope: egui::Id, delta: egui::Vec2) {
    if !delta.is_finite() || delta == egui::Vec2::ZERO {
        return;
    }
    let id = id.with("studio-pan-request");
    ctx.data_mut(|data| {
        let previous = data
            .get_temp::<(egui::Id, egui::Vec2)>(id)
            .filter(|(source, _)| *source == scope)
            .map(|(_, delta)| delta)
            .unwrap_or_default();
        let next = previous + delta;
        if next.is_finite() {
            data.insert_temp(id, (scope, next));
        }
    });
    ctx.request_repaint();
}

/// Queue bounded movement only when there is room to scroll. Edge drags stop
/// requesting animation frames at a limit, including when overscroll is disabled.
pub fn pan_by_clamped(
    ctx: &egui::Context,
    id: egui::Id,
    scope: egui::Id,
    delta: egui::Vec2,
) -> bool {
    if !delta.is_finite() || delta == egui::Vec2::ZERO {
        return false;
    }
    let admitted = ctx
        .data(|data| {
            let layout = data.get_temp::<Layout>(id.with("studio-canvas-geometry"))?;
            let offset = data.get_temp::<egui::Vec2>(id.with("studio-pan-offset"))?;
            Some(offset - layout.geometry.clamp(offset - delta))
        })
        .unwrap_or_default();
    if admitted.length_sq() < 0.000001 {
        return false;
    }
    pan_by(ctx, id, scope, admitted);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(ctx: &egui::Context, scope: egui::Id) -> egui::Rect {
        let mut canvas = egui::Rect::NOTHING;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(400.0, 300.0),
                )),
                ..Default::default()
            },
            |ui| {
                canvas = canvas_for_scope(
                    ui,
                    egui::Id::unique("tab"),
                    scope,
                    egui::Vec2::splat(2000.0),
                    |_, response, _| response.rect,
                );
            },
        );
        output.textures_delta.clear();
        canvas
    }
    #[test]
    fn bounded_pan_stops_at_limits_and_rejects_nonfinite_requests() {
        let ctx = egui::Context::default();
        let tab = egui::Id::unique("tab");
        let scope = egui::Id::unique("source");
        frame(&ctx, scope);
        assert!(!pan_by_clamped(&ctx, tab, scope, egui::vec2(f32::NAN, 0.0)));
        assert!(pan_by_clamped(
            &ctx,
            tab,
            scope,
            egui::vec2(-100_000.0, -100_000.0)
        ));
        let last = frame(&ctx, scope);
        assert!(!pan_by_clamped(&ctx, tab, scope, egui::vec2(-50.0, -50.0)));
        assert_eq!(frame(&ctx, scope), last);
        assert!(pan_by_clamped(&ctx, tab, scope, egui::vec2(50.0, 50.0)));
        assert_eq!(frame(&ctx, scope).min - last.min, egui::vec2(50.0, 50.0));
    }

    #[test]
    fn queued_pan_cannot_cross_a_document_or_view_scope() {
        let ctx = egui::Context::default();
        let tab = egui::Id::unique("tab");
        let old = egui::Id::unique("old-document-view");
        let current = egui::Id::unique("new-document-view");
        frame(&ctx, old);
        let before = frame(&ctx, old);
        pan_by(&ctx, tab, old, egui::vec2(-50.0, -25.0));
        let rejected = frame(&ctx, current);
        assert_eq!(rejected.min, before.min);
        pan_by(&ctx, tab, old, egui::vec2(-20.0, -10.0));
        pan_by(&ctx, tab, current, egui::vec2(-50.0, -25.0));
        let accepted = frame(&ctx, current);
        assert!((accepted.min - rejected.min - egui::vec2(-50.0, -25.0)).length() < 0.1);
    }
}
