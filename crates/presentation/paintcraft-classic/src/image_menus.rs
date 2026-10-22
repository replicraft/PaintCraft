//! Image menu presentation. Transform operations remain unavailable.
use egui::{Rect, Vec2};
use paintcraft_ui::icons::Icon;

pub(super) fn rotate(ui: &mut egui::Ui) {
    menu(
        ui,
        "Rotate",
        Icon::RotateClockwise,
        &[
            ("Rotate right 90°", Icon::RotateClockwise),
            ("Rotate left 90°", Icon::RotateCounterclockwise),
            ("Rotate 180°", Icon::Rotate180),
            ("Rotate custom", Icon::RotateCustom),
        ],
    );
}

pub(super) fn flip(ui: &mut egui::Ui) {
    menu(
        ui,
        "Flip",
        Icon::FlipHorizontal,
        &[
            ("Flip vertical", Icon::FlipVertical),
            ("Flip horizontal", Icon::FlipHorizontal),
        ],
    );
}

fn menu(ui: &mut egui::Ui, label: &str, icon: Icon, entries: &[(&str, Icon)]) {
    let (response, _) = egui::containers::menu::MenuButton::from_button(
        egui::Button::new("     ")
            .min_size(Vec2::new(46.0, 30.0))
            .frame(false),
    )
    .config(
        egui::containers::menu::MenuConfig::new()
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
    )
    .ui(ui, |ui| {
        ui.set_min_width(220.0);
        ui.spacing_mut().interact_size.y = 40.0;
        for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
            ui.style_mut()
                .text_styles
                .insert(style, egui::FontId::proportional(15.0));
        }
        let height =
            (ui.ctx().content_rect().bottom() - ui.cursor().min.y - 12.0).clamp(40.0, 480.0);
        ui.set_max_height(height);
        egui::ScrollArea::vertical()
            .id_salt(("image-transform-menu", label))
            .max_height(height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for &(name, icon) in entries {
                    ui.add_enabled(
                        false,
                        egui::Button::new((
                            paintcraft_ui::icons::image(ui.ctx(), icon, 22.0),
                            name,
                        ))
                        .min_size(Vec2::new(220.0, 40.0)),
                    )
                    .on_disabled_hover_text("This image transformation is not available yet.");
                }
            });
    });
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    paintcraft_ui::icons::paint(
        ui.painter(),
        Rect::from_center_size(
            response.rect.left_center() + Vec2::new(15.0, 0.0),
            Vec2::splat(22.0),
        ),
        icon,
    );
    let center = response.rect.right_center() - Vec2::new(7.0, 0.0);
    let stroke = egui::Stroke::new(1.0, ui.visuals().text_color());
    ui.painter().line_segment(
        [center + Vec2::new(-3.0, -1.5), center + Vec2::new(0.0, 1.5)],
        stroke,
    );
    ui.painter().line_segment(
        [center + Vec2::new(0.0, 1.5), center + Vec2::new(3.0, -1.5)],
        stroke,
    );
    response.on_hover_text(label);
}
