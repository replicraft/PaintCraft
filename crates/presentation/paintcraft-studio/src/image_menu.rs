//! Studio image actions use shared editor contracts, with no Classic UI dependency.
use paintcraft_core::types::orthogonal::OrthogonalTransform;
use paintcraft_ui::actions::{Action, Context};
use paintcraft_ui::t;

pub(super) fn show(ui: &mut egui::Ui, context: Context, actions: &mut Vec<Action>) {
    ui.menu_button(t!(ui.ctx(), "Image"), |ui| {
        if paintcraft_ui::actions::menu_item(ui, Action::CanvasSize, context) {
            actions.push(Action::CanvasSize);
            ui.close();
        }
        ui.separator();
        if paintcraft_ui::actions::menu_item(ui, Action::Crop, context) {
            actions.push(Action::Crop);
            ui.close();
        }
        ui.separator();
        for transform in [
            OrthogonalTransform::RotateRight,
            OrthogonalTransform::RotateLeft,
            OrthogonalTransform::Rotate180,
            OrthogonalTransform::FlipHorizontal,
            OrthogonalTransform::FlipVertical,
        ] {
            let action = Action::TransformImage(transform);
            if paintcraft_ui::actions::menu_item(ui, action, context) {
                actions.push(action);
                ui.close();
            }
        }
        ui.separator();
        if paintcraft_ui::actions::menu_item(ui, Action::ClearCanvas, context) {
            actions.push(Action::ClearCanvas);
            ui.close();
        }
    });
}
