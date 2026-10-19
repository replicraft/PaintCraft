//! Classic canvas presentation, independent of Studio navigation and overlays.
pub fn canvas<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    size: egui::Vec2,
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
                    ui.add_space(inset.x.max(0.0));
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
            let stroke =
                egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
            for point in [
                rect.right_center(),
                rect.center_bottom(),
                rect.right_bottom(),
            ] {
                let handle = egui::Rect::from_center_size(point, egui::vec2(5.0, 5.0));
                ui.painter().rect(
                    handle,
                    0.0,
                    ui.visuals().window_fill(),
                    stroke,
                    egui::StrokeKind::Inside,
                );
            }
            output
        })
        .inner
}
