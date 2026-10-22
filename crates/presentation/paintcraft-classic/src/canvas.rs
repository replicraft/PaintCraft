//! Classic canvas presentation, independent of Studio navigation and overlays.
fn content<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    size: egui::Vec2,
    left_clearance: f32,
    paint: impl FnOnce(&mut egui::Ui, egui::Response, egui::Painter) -> R,
) -> R {
    egui::ScrollArea::both()
        .id_salt(("classic-canvas", id))
        .auto_shrink([false, false])
        .show(ui, |ui| {
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
        .inner
}

/// Plain and ruled views share a stable document scope, preserving scroll state.
pub fn canvas<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    size: egui::Vec2,
    left_clearance: f32,
    paint: impl FnOnce(&mut egui::Ui, egui::Response, egui::Painter) -> R,
) -> R {
    let viewport = ui.available_rect_before_wrap();
    ui.scope_builder(
        egui::UiBuilder::new()
            .scope_id(id.with("classic-canvas-presentation"))
            .max_rect(viewport),
        |ui| content(ui, id, size, left_clearance, paint),
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
                content(ui, id, size, left_clearance, |ui, response, painter| {
                    let rect = response.rect;
                    (paint(ui, response, painter), rect)
                })
            },
        )
        .inner;
    super::rulers::paint(ui, viewport, rect, extent);
    ui.advance_cursor_after_rect(viewport);
    result
}
