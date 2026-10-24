//! Shared icon widgets; no operating-system or editor-session side effects.
use crate::i18n::tr;
use egui::{Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

pub use crate::icons::Icon;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum IconArtwork {
    Colorful,
    ClassicMenu,
    ClassicStatus,
}

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
    icon_button_artwork(
        ui,
        label,
        icon,
        selected,
        large,
        tooltip,
        IconArtwork::Colorful,
    )
}

/// Theme-tinted outline artwork for compact Classic action bars.
pub fn classic_menu_button(
    ui: &mut Ui,
    label: &str,
    icon: Icon,
    selected: bool,
    tooltip: impl Into<egui::WidgetText>,
) -> Response {
    icon_button_artwork(
        ui,
        label,
        icon,
        selected,
        false,
        tooltip,
        IconArtwork::ClassicMenu,
    )
}

pub(crate) fn icon_button_artwork(
    ui: &mut Ui,
    label: &str,
    icon: Icon,
    selected: bool,
    large: bool,
    tooltip: impl Into<egui::WidgetText>,
    artwork: IconArtwork,
) -> Response {
    let localized_label = tr(ui.ctx(), label);
    let label = localized_label.as_str();
    let size = if large {
        Vec2::new(64.0, 54.0)
    } else {
        Vec2::splat(30.0_f32.max(ui.spacing().interact_size.y))
    };
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let accessible_label = tr(ui.ctx(), label);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::Role::Button,
            ui.is_enabled(),
            selected,
            &accessible_label,
        )
    });
    let visuals = ui.style().interact_selectable(&response, false);
    let ink = if selected {
        ui.visuals().selection.stroke.color
    } else if artwork != IconArtwork::Colorful {
        ui.visuals().strong_text_color()
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
    let glyph = Rect::from_center_size(center, Vec2::splat(22.0));
    match artwork {
        IconArtwork::ClassicMenu => {
            crate::icons::classic_menu_image(ui.ctx(), icon, 22.0, ink).paint_at(ui, glyph);
        }
        IconArtwork::ClassicStatus => {
            crate::icons::classic_status_image(ui.ctx(), icon, 22.0, ui.visuals().dark_mode)
                .paint_at(ui, glyph);
        }
        IconArtwork::Colorful => crate::icons::paint(ui.painter(), glyph, icon),
    }
    if large {
        ui.painter().text(
            rect.center_bottom() - Vec2::new(0.0, 8.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::TextStyle::Small.resolve(ui.style()),
            ink,
        );
    }
    let tooltip = tooltip.into();
    response.on_hover_text(tr(ui.ctx(), tooltip.text()))
}
