//! Historical rotation rows share the canonical transform actions with modern UI.
use crate::{Action, ClassicVariant};
use paintcraft_core::types::orthogonal::OrthogonalTransform as Transform;
use paintcraft_ui::t;
use paintcraft_ui::{actions::Context, icons::Icon};

pub(super) fn rotate(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    context: Context,
    actions: &mut Vec<Action>,
) {
    let (button, _) = egui::containers::menu::MenuButton::from_button(
        egui::Button::new((
            paintcraft_ui::icons::legacy_image(ui.ctx(), Icon::RotateClockwise, 14.0),
            t!(ui.ctx(), "Rotate"),
        )).frame_when_inactive(false),
    )
    .config(crate::legacy_surface::menu_config())
    .ui(ui, |ui| {
        ui.set_width(220.0);
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.spacing_mut().interact_size.y = 24.0;
        let height = crate::popup_layout::height(ui, 48.0, 240.0);
        egui::ScrollArea::vertical().id_salt("historical-image-transforms")
            .max_height(height).show(ui, |ui| {
                for (index, transform) in [
                    Transform::RotateRight, Transform::RotateLeft, Transform::Rotate180,
                    Transform::FlipVertical, Transform::FlipHorizontal,
                ].into_iter().enumerate() {
                    if index == 3 { ui.separator(); }
                    let action = Action::TransformImage(transform);
                    let label = action.descriptor_for(context.frontend).label;
                    let response = ui.add_enabled_ui(action.enabled(context), |ui| {
                        crate::legacy_surface::menu_row(ui, variant, label, label, action.icon(context), None, None)
                    }).inner;
                    if response.clicked() {
                        actions.push(action);
                        ui.close();
                    }
                    if !action.enabled(context) {
                        response.on_disabled_hover_text(t!(ui.ctx(), "Select a nonempty canvas area and choose a layer, or deselect to transform the whole image."));
                    }
                }
            });
    });
    crate::legacy_surface::open_state(ui, &button, "Rotate");
    button.on_hover_text(t!(ui.ctx(), "Rotate"));
}
