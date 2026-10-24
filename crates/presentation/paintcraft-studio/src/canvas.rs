//! Studio canvas presentation. Layered navigation is independent of Classic.
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
            let mut area = egui::ScrollArea::both()
                .id_salt(("studio-canvas", id))
                .auto_shrink([false, false])
                .scroll_source(egui::scroll_area::ScrollSource {
                    drag: egui::scroll_area::DragScroll::Never,
                    ..Default::default()
                });
            if let Some((source, delta)) = delta
                && source == scope
            {
                let old = ui
                    .data(|data| data.get_temp::<egui::Vec2>(offset))
                    .unwrap_or_default();
                area = area.scroll_offset((old - delta).max(egui::Vec2::ZERO));
            }
            let output = area.show(ui, |ui| {
                let inset = (ui.available_size() - size) * 0.5;
                ui.add_space(inset.y.max(12.0));
                let (response, painter) = ui
                    .horizontal(|ui| {
                        ui.add_space(inset.x.max(12.0));
                        ui.allocate_painter(size, egui::Sense::click_and_drag())
                    })
                    .inner;
                let border = response.rect.expand(1.0);
                ui.painter().rect_filled(
                    border,
                    0.0,
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                );
                paint(ui, response, painter)
            });
            ui.data_mut(|data| data.insert_temp(offset, output.state.clamped_offset()));
            output.inner
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
