//! Classic canvas presentation, independent of Studio navigation and overlays.
#[path = "canvas_anchor.rs"]
mod anchor;
pub(super) use anchor::Anchor;
pub use anchor::anchor_at;
fn content<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    scope: egui::Id,
    size: egui::Vec2,
    left_clearance: f32,
    paint: impl FnOnce(&mut egui::Ui, egui::Response, egui::Painter) -> R,
) -> R {
    super::canvas_scroll::show(ui, id, scope, size, left_clearance, |ui| {
        let inset = (ui.available_size() - size) * 0.5;
        ui.add_space(inset.y.max(0.0));
        let (response, painter) = ui
            .horizontal(|ui| {
                ui.add_space(inset.x.max(left_clearance).max(0.0));
                ui.allocate_painter(size, egui::Sense::click_and_drag())
            })
            .inner;
        let rect = response.rect;
        let shadow = egui::epaint::Shadow {
            offset: [0, 2],
            blur: 10,
            spread: 0,
            color: egui::Color32::from_black_alpha(45),
        };
        ui.painter().add(shadow.as_shape(rect, 0));
        let output = paint(ui, response, painter);
        super::canvas_handles::show(ui, rect);
        output
    })
}

/// Plain and ruled views share a stable document scope, preserving scroll state.
pub fn canvas<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    size: egui::Vec2,
    left_clearance: f32,
    paint: impl FnOnce(&mut egui::Ui, egui::Response, egui::Painter) -> R,
) -> R {
    canvas_for_scope(ui, id, id, size, left_clearance, paint)
}

pub fn canvas_for_scope<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    scope: egui::Id,
    size: egui::Vec2,
    left_clearance: f32,
    paint: impl FnOnce(&mut egui::Ui, egui::Response, egui::Painter) -> R,
) -> R {
    let viewport = ui.available_rect_before_wrap();
    ui.scope_builder(
        egui::UiBuilder::new()
            .scope_id(id.with("classic-canvas-presentation"))
            .max_rect(viewport),
        |ui| content(ui, id, scope, size, left_clearance, paint),
    )
    .inner
}

pub fn ruled_canvas<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    size: egui::Vec2,
    extent: (u32, u32),
    left_clearance: f32,
    paint: impl FnOnce(&mut egui::Ui, egui::Response, egui::Painter) -> R,
) -> R {
    ruled_canvas_for_scope(ui, id, id, size, extent, left_clearance, paint)
}

pub fn ruled_canvas_for_scope<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    scope: egui::Id,
    size: egui::Vec2,
    extent: (u32, u32),
    left_clearance: f32,
    paint: impl FnOnce(&mut egui::Ui, egui::Response, egui::Painter) -> R,
) -> R {
    let viewport = ui.available_rect_before_wrap();
    let body = egui::Rect::from_min_max(
        (viewport.min + egui::Vec2::splat(super::RULER_SIZE)).min(viewport.max),
        viewport.max,
    );
    let (result, rect) = ui
        .scope_builder(
            egui::UiBuilder::new()
                .scope_id(id.with("classic-canvas-presentation"))
                .max_rect(body),
            |ui| {
                content(
                    ui,
                    id,
                    scope,
                    size,
                    left_clearance,
                    |ui, response, painter| {
                        let rect = response.rect;
                        (paint(ui, response, painter), rect)
                    },
                )
            },
        )
        .inner;
    super::rulers::paint(ui, viewport, rect, extent);
    ui.advance_cursor_after_rect(viewport);
    result
}

/// Queue a screen-space pan without changing document coordinates or tool state.
pub fn pan_by(ctx: &egui::Context, id: egui::Id, delta: egui::Vec2) {
    if !delta.is_finite() || delta == egui::Vec2::ZERO {
        return;
    }
    ctx.data_mut(|data| {
        // Explicit navigation takes over from a settling zoom anchor.
        data.remove::<Anchor>(id.with("classic-anchor-request"));
        let key = id.with("classic-pan-request");
        let pending = data.get_temp::<egui::Vec2>(key).unwrap_or_default() + delta;
        if pending.is_finite() {
            data.insert_temp(key, pending);
        }
    });
    ctx.request_repaint();
}

#[cfg(test)]
#[path = "canvas_anchor_tests.rs"]
mod anchor_tests;
