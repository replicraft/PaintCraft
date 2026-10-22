//! Shared icon widgets; no operating-system or editor-session side effects.
use egui::{Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

pub use crate::icons::Icon;

pub fn icon_button(ui: &mut Ui, label: &str, icon: Icon, selected: bool, large: bool) -> Response {
    icon_button_with_tooltip(ui, label, icon, selected, large, label)
}

/// Use one tooltip when an action has shortcut or explanatory text.
pub fn icon_button_with_tooltip(
    ui: &mut Ui,
    label: &str,
    icon: Icon,
    selected: bool,
    large: bool,
    tooltip: impl Into<egui::WidgetText>,
) -> Response {
    let size = if large {
        Vec2::new(64.0, 54.0)
    } else {
        Vec2::splat(30.0_f32.max(ui.spacing().interact_size.y))
    };
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label)
    });
    let visuals = ui.style().interact_selectable(&response, false);
    let ink = if selected {
        ui.visuals().selection.stroke.color
    } else {
        visuals.fg_stroke.color
    };
    if selected || response.hovered() || response.has_focus() {
        ui.painter().rect(
            rect.shrink(1.0),
            4.0,
            if selected {
                ui.visuals().selection.bg_fill
            } else {
                visuals.bg_fill
            },
            if selected {
                Stroke::new(1.0, ink)
            } else {
                visuals.bg_stroke
            },
            StrokeKind::Inside,
        );
    }
    let center = rect.center()
        - if large {
            Vec2::new(0.0, 7.0)
        } else {
            Vec2::ZERO
        };
    crate::icons::paint(
        ui.painter(),
        Rect::from_center_size(center, Vec2::splat(22.0)),
        icon,
    );
    if large {
        ui.painter().text(
            rect.center_bottom() - Vec2::new(0.0, 8.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::TextStyle::Small.resolve(ui.style()),
            ink,
        );
    }
    response.on_hover_text(tooltip)
}
