//! Resize-handle previews. Actual bounds editing stays in the Canvas size dialog.
use egui::{Color32, CursorIcon, Rect, Sense, Vec2};
use paintcraft_ui::t;
pub(super) fn show(ui: &mut egui::Ui, canvas: Rect) {
    // Keep marker pixels outside the document. Their hover targets touch the
    // canvas edge without consuming ordinary painting inside the image.
    let outer = canvas.expand(3.5);
    let handles = ui.new_child(
        egui::UiBuilder::new()
            .scope_id(ui.scope_id().with("canvas-resize-handles"))
            .max_rect(canvas.expand(7.0))
            .disabled(),
    );
    for (name, point) in [
        ("top left", outer.left_top()),
        ("top", outer.center_top()),
        ("top right", outer.right_top()),
        ("right", outer.right_center()),
        ("bottom right", outer.right_bottom()),
        ("bottom", outer.center_bottom()),
        ("bottom left", outer.left_bottom()),
        ("left", outer.left_center()),
    ] {
        let target = Rect::from_center_size(point, Vec2::splat(7.0));
        let response = handles.interact(target, handles.scope_id().with(name), Sense::hover());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Button,
                false,
                format!("Resize canvas from {name}"),
            )
        });
        if response.contains_pointer() {
            ui.ctx().set_cursor_icon(CursorIcon::NotAllowed);
        }
        response.on_disabled_hover_text(t!(
            ui.ctx(),
            "Canvas resize handles are not available yet. Use the View menu's Canvas size command.",
        ));
        ui.painter().rect(
            Rect::from_center_size(point, Vec2::splat(5.0)),
            0,
            Color32::WHITE,
            egui::Stroke::new(1.0, Color32::from_gray(48)),
            egui::StrokeKind::Inside,
        );
    }
}
