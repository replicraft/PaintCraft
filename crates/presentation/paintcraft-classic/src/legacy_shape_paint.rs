//! Historical shape paint popups. Choices only alter the host's tool preferences.
use crate::{ClassicVariant, Controls};
use egui::{Color32, Rect, Stroke, StrokeKind, Vec2};
use paintcraft_tools::{ShapeStyle, Tool};
use paintcraft_ui::{
    i18n::tr,
    icons::{self, Icon},
};

pub(super) fn show(ui: &mut egui::Ui, variant: ClassicVariant, state: &mut Controls) {
    let enabled = matches!(
        state.tool,
        Tool::Rect | Tool::Ellipse | Tool::Preset(_) | Tool::Polygon
    );
    for (outline, caption, accessible, icon) in [
        (true, "Outline", "Shape outline", Icon::Pencil),
        (false, "Fill", "Shape fill", Icon::Bucket),
    ] {
        ui.add_enabled_ui(enabled, |ui| {
            let (button, _) = egui::containers::menu::MenuButton::from_button(
                egui::Button::new((
                    icons::legacy_image(ui.ctx(), icon, 14.0),
                    tr(ui.ctx(), caption),
                ))
                .frame_when_inactive(false),
            )
            .config(crate::legacy_surface::menu_config())
            .ui(ui, |ui| {
                ui.set_width(220.0);
                ui.spacing_mut().interact_size.y = 24.0;
                ui.spacing_mut().item_spacing.y = 0.0;
                for (solid, label) in [
                    (false, if outline { "No outline" } else { "No fill" }),
                    (
                        true,
                        if outline {
                            "Solid outline"
                        } else {
                            "Solid fill"
                        },
                    ),
                ] {
                    let painted = if outline {
                        state.options.style != ShapeStyle::Filled
                    } else {
                        state.options.style != ShapeStyle::Outline
                    };
                    let response = crate::legacy_surface::menu_row(
                        ui,
                        variant,
                        label,
                        label,
                        None,
                        Some(painted == solid),
                        None,
                    );
                    let rect = Rect::from_center_size(
                        response.rect.left_center() + Vec2::new(14.0, 0.0),
                        Vec2::splat(12.0),
                    );
                    sample(ui, rect, outline, solid);
                    if response.clicked() {
                        crate::dropdowns::set_shape_paint(state, outline, solid);
                        ui.close();
                    }
                }
            });
            crate::legacy_surface::open_state(ui, &button, accessible);
            if enabled {
                button.on_hover_text(tr(ui.ctx(), accessible));
            } else {
                button.on_disabled_hover_text(tr(
                    ui.ctx(),
                    "Select a rectangle or ellipse to change its outline and fill",
                ));
            }
        });
    }
}

fn sample(ui: &egui::Ui, rect: Rect, outline: bool, solid: bool) {
    let ink = if ui.visuals().dark_mode {
        Color32::from_rgb(150, 205, 240)
    } else {
        Color32::from_rgb(46, 91, 130)
    };
    if solid && !outline {
        ui.painter().rect_filled(rect, 0, ink);
    } else {
        ui.painter().rect_stroke(
            rect,
            0,
            Stroke::new(if solid { 2.0 } else { 1.0 }, ink),
            StrokeKind::Inside,
        );
    }
    if !solid {
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_top()],
            Stroke::new(1.0, Color32::from_rgb(190, 65, 55)),
        );
    }
}
