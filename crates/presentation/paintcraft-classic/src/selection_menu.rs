//! Classic selection presentation; host actions retain selection/history semantics.
use super::{Action, Controls};
use egui::{Align2, FontId, Rect, Vec2};
use paintcraft_tools::Tool;
use paintcraft_ui::{actions::Context, icons::Icon};

pub(super) fn show(
    ui: &mut egui::Ui,
    state: &mut Controls,
    context: Context,
    actions: &mut Vec<Action>,
) {
    let height = (ui.ctx().content_rect().bottom() - ui.cursor().min.y - 12.0).clamp(40.0, 480.0);
    ui.set_width(260.0);
    let dark = ui.visuals().dark_mode;
    ui.visuals_mut().override_text_color =
        Some(egui::Color32::from_gray(if dark { 245 } else { 24 }));
    ui.visuals_mut().weak_text_color = Some(egui::Color32::from_gray(if dark { 208 } else { 96 }));
    ui.set_max_height(height);
    ui.spacing_mut().item_spacing.y = 0.0;
    egui::ScrollArea::vertical()
        .id_salt("classic-selection-menu")
        .max_height(height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let rectangle = row(
                ui,
                "Rectangle",
                "Rectangular selection",
                Some(Icon::RectangleSelect),
                // The mode persists independently of which drawing tool is active.
                Some(true),
                None,
            );
            if rectangle.clicked() {
                state.tool = Tool::Select;
                actions.push(Action::SelectTool(Tool::Select));
                ui.close();
            }
            unavailable(
                ui,
                "Free-form",
                "Free-form selection",
                Some(Icon::LassoSelect),
                None,
                "Free-form selection is not available yet.",
            );
            ui.separator();
            action(ui, Action::SelectAll, context, actions);
            unavailable(
                ui,
                "Invert selection",
                "Invert selection",
                Some(Icon::InvertSelection),
                None,
                "Inverting selections is not available yet.",
            );
            ui.separator();
            unavailable(
                ui,
                "Transparent selection",
                "Transparent selection",
                None,
                None,
                "Transparent selection is not available yet.",
            );
            ui.separator();
            unavailable(
                ui,
                "Delete",
                "Delete",
                Some(Icon::DeleteLayer),
                Some("Delete"),
                "Deleting selected pixels is not available yet.",
            );
            // PaintCraft extension follows the reference choices, keeping the
            // existing working Deselect action reachable without a shortcut.
            ui.separator();
            action(ui, Action::Deselect, context, actions);
        });
}

fn action(ui: &mut egui::Ui, action: Action, context: Context, actions: &mut Vec<Action>) {
    let descriptor = action.descriptor_for(context.frontend);
    let shortcut = descriptor
        .shortcut
        .map(|shortcut| ui.ctx().format_shortcut(&shortcut));
    let response = ui
        .add_enabled_ui(action.enabled(context), |ui| {
            row(
                ui,
                descriptor.label,
                descriptor.label,
                action.classic_menu_icon(context),
                None,
                shortcut.as_deref(),
            )
        })
        .inner;
    if response.clicked() {
        actions.push(action);
        ui.close();
    }
}

fn unavailable(
    ui: &mut egui::Ui,
    label: &str,
    accessible: &str,
    icon: Option<Icon>,
    shortcut: Option<&str>,
    help: &str,
) {
    ui.add_enabled_ui(false, |ui| row(ui, label, accessible, icon, None, shortcut))
        .inner
        .on_disabled_hover_text(help);
}

fn row(
    ui: &mut egui::Ui,
    label: &str,
    accessible: &str,
    icon: Option<Icon>,
    selected: Option<bool>,
    shortcut: Option<&str>,
) -> egui::Response {
    let background = (selected == Some(true)).then(|| ui.painter().add(egui::Shape::Noop));
    let mut button = egui::Button::new(" ")
        .frame_when_inactive(selected == Some(true))
        .min_size(Vec2::new(ui.available_width().max(260.0), 40.0));
    if let Some(selected) = selected {
        button = button.selected(selected);
        if selected {
            button = button
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE);
        }
    }
    let response = ui.add(button);
    if let Some(background) = background {
        ui.painter().set(
            background,
            egui::Shape::rect_filled(
                response.rect.shrink(0.5),
                5,
                crate::control_surface::fill(
                    ui,
                    response.hovered() || response.has_focus(),
                    response.is_pointer_button_down_on(),
                ),
            ),
        );
    }
    response.widget_info(|| match selected {
        Some(selected) => {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, accessible)
        }
        None => egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), accessible),
    });
    // Disabled rows already fade through their scoped painter.
    let ink = ui.visuals().text_color();
    if selected == Some(true) {
        ui.painter().rect_filled(
            Rect::from_center_size(
                response.rect.left_center() + Vec2::new(4.0, 0.0),
                Vec2::new(3.0, 22.0),
            ),
            1.5,
            ui.visuals().selection.stroke.color,
        );
    }
    if let Some(icon) = icon {
        let rect = Rect::from_center_size(
            response.rect.left_center() + Vec2::new(26.0, 0.0),
            Vec2::splat(22.0),
        );
        paintcraft_ui::icons::classic_menu_image(
            ui.ctx(),
            icon,
            22.0,
            ui.visuals().strong_text_color(),
        )
        .paint_at(ui, rect);
    }
    ui.painter().text(
        response.rect.left_center() + Vec2::new(50.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(16.0),
        ink,
    );
    if let Some(shortcut) = shortcut {
        ui.painter().text(
            response.rect.right_center() - Vec2::new(12.0, 0.0),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::proportional(16.0),
            ui.visuals().weak_text_color(),
        );
    }
    response
}
