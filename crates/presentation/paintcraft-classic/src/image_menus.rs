//! Image controls with real selection Crop and staged Resize/skew presentation.
use egui::{Rect, Vec2};
use paintcraft_ui::icons::Icon;

/// The resize control spans the two Image rows, matching the reference grouping.
pub(super) fn controls(
    ui: &mut egui::Ui,
    context: paintcraft_ui::actions::Context,
    actions: &mut Vec<super::Action>,
) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        egui::Grid::new("image-ribbon")
            .spacing(Vec2::new(6.0, 4.0))
            .show(ui, |ui| {
                let enabled = super::Action::Crop.enabled(context);
                let response = ui
                    .add_enabled_ui(enabled, |ui| {
                        let response = ui.add(
                            egui::Button::new("")
                                .min_size(Vec2::splat(40.0))
                                .frame_when_inactive(false)
                                .frame(true),
                        );
                        paintcraft_ui::icons::classic_image(
                            ui.ctx(),
                            Icon::Crop,
                            22.0,
                            ui.visuals().dark_mode,
                        )
                        .paint_at(
                            ui,
                            Rect::from_center_size(response.rect.center(), Vec2::splat(22.0)),
                        );
                        response
                    })
                    .inner;
                response
                    .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, "Crop"));
                if response.clicked() {
                    actions.push(super::Action::Crop);
                }
                if enabled {
                    response.on_hover_text("Crop");
                } else {
                    response.on_disabled_hover_text("Select an area of the canvas to crop");
                }
                rotate(ui);
                ui.end_row();
                unavailable(
                    ui,
                    "Remove background",
                    Icon::RemoveBackground,
                    Vec2::splat(40.0),
                    22.0,
                    "Background removal is not available yet.",
                );
                flip(ui);
            });
        let enabled = super::Action::ResizeSkew.enabled(context);
        let response = ui
            .add_enabled_ui(enabled, |ui| {
                let response = ui.add(
                    egui::Button::new("")
                        .min_size(Vec2::new(64.0, 84.0))
                        .frame_when_inactive(false)
                        .frame(true),
                );
                paintcraft_ui::icons::classic_image(
                    ui.ctx(),
                    Icon::Resize,
                    32.0,
                    ui.visuals().dark_mode,
                )
                .paint_at(
                    ui,
                    Rect::from_center_size(response.rect.center(), Vec2::splat(32.0)),
                );
                response
            })
            .inner;
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, enabled, "Resize and skew")
        });
        if response
            .on_hover_text("Resize and skew")
            .on_disabled_hover_text("Open an image to preview resize and skew.")
            .clicked()
        {
            actions.push(super::Action::ResizeSkew);
        }
    });
}

fn unavailable(ui: &mut egui::Ui, label: &str, icon: Icon, size: Vec2, points: f32, help: &str) {
    let response = ui
        .add_enabled_ui(false, |ui| {
            let response = ui.add(
                egui::Button::new("")
                    .min_size(size)
                    .frame_when_inactive(false)
                    .frame(true),
            );
            paintcraft_ui::icons::classic_image(ui.ctx(), icon, points, ui.visuals().dark_mode)
                .paint_at(
                    ui,
                    Rect::from_center_size(response.rect.center(), Vec2::splat(points)),
                );
            response
        })
        .inner;
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, false, label));
    response.on_disabled_hover_text(help);
}

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
        Icon::FlipVertical,
        &[
            ("Flip vertical", Icon::FlipVertical),
            ("Flip horizontal", Icon::FlipHorizontal),
        ],
    );
}

fn menu(ui: &mut egui::Ui, label: &str, icon: Icon, entries: &[(&str, Icon)]) {
    let (response, _) = egui::containers::menu::MenuButton::from_button(
        egui::Button::new("     ")
            .min_size(Vec2::new(56.0, 40.0))
            .frame_when_inactive(false)
            .frame(true),
    )
    .config(
        egui::containers::menu::MenuConfig::new()
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
    )
    .ui(ui, |ui| {
        ui.set_width(240.0);
        ui.spacing_mut().interact_size.y = 40.0;
        ui.spacing_mut().item_spacing.y = 0.0;
        for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
            ui.style_mut()
                .text_styles
                .insert(style, egui::FontId::proportional(16.0));
        }
        let height = super::popup_layout::height(ui, 40.0, 480.0);
        egui::ScrollArea::vertical()
            .id_salt(("image-transform-menu", label))
            .max_height(height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for &(name, icon) in entries {
                    ui.add_enabled(
                        false,
                        egui::Button::new((
                            paintcraft_ui::icons::classic_image(
                                ui.ctx(),
                                icon,
                                22.0,
                                ui.visuals().dark_mode,
                            ),
                            name,
                        ))
                        .min_size(Vec2::new(240.0, 40.0)),
                    )
                    .on_disabled_hover_text("This image transformation is not available yet.");
                }
            });
    });
    super::dropdowns::open_state(ui, &response, label);
    paintcraft_ui::icons::classic_image(ui.ctx(), icon, 22.0, ui.visuals().dark_mode).paint_at(
        ui,
        Rect::from_center_size(
            response.rect.left_center() + Vec2::new(17.0, 0.0),
            Vec2::splat(22.0),
        ),
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
