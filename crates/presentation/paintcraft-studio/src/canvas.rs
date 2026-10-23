//! Studio canvas presentation. Layered navigation is independent of Classic.
pub fn canvas<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
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
            egui::ScrollArea::both()
                .id_salt(("studio-canvas", id))
                .auto_shrink([false, false])
                .show(ui, |ui| {
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
                })
                .inner
        },
    )
    .inner
}
