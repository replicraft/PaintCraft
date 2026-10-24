//! Historical common-dialog layout; adoption remains owned by the host's color draft.
use super::{CUSTOM_COLOR_COUNT, ColorEditor, ColorResponse, Rgba};
use egui::{Color32, Vec2};
use paintcraft_ui::t;
mod fields;
mod picker;

pub(super) fn show(ctx: &egui::Context, editor: &mut ColorEditor) -> ColorResponse {
    let width = (ctx.content_rect().width() - 40.0).clamp(240.0, 486.0);
    let body_height = (ctx.content_rect().height() - 100.0).clamp(64.0, 360.0);
    let mut result = ColorResponse::Pending;
    let modal = egui::Modal::new(egui::Id::unique("historical-edit-colors"))
        .frame(
            egui::Frame::window(&ctx.style_of(ctx.theme()))
                .fill(if ctx.theme() == egui::Theme::Dark {
                    Color32::from_gray(43)
                } else {
                    Color32::from_gray(240)
                })
                .corner_radius(0)
                .inner_margin(10),
        )
        .show(ctx, |ui| {
            ui.set_width(width);
            if !ui.visuals().dark_mode {
                ui.visuals_mut().selection.stroke =
                    egui::Stroke::new(1.0, Color32::from_rgb(0, 120, 215));
                ui.visuals_mut().selection.bg_fill = Color32::from_rgb(204, 232, 255);
            }
            crate::form_controls::style(ui);
            ui.spacing_mut().interact_size.y = 24.0;
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
            for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                ui.style_mut()
                    .text_styles
                    .insert(style, egui::FontId::proportional(12.0));
            }
            let widgets = &mut ui.visuals_mut().widgets;
            for widget in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
                &mut widgets.open,
            ] {
                widget.corner_radius = egui::CornerRadius::ZERO;
            }
            ui.label(egui::RichText::new(t!(ui.ctx(), "Edit colors")).strong());
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("historical-color-body")
                .max_height(body_height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if width >= 470.0 {
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(213.0);
                                palettes(ui, editor);
                            });
                            ui.add_space(8.0);
                            ui.vertical(|ui| {
                                ui.set_width(244.0);
                                definition(ui, editor);
                            });
                        });
                    } else {
                        palettes(ui, editor);
                        ui.separator();
                        definition(ui, editor);
                    }
                });
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        editor.valid(),
                        egui::Button::new(t!(ui.ctx(), "OK")).min_size(Vec2::new(76.0, 24.0)),
                    )
                    .clicked()
                {
                    result = ColorResponse::Accept;
                }
                if ui
                    .add_sized([76.0, 24.0], egui::Button::new(t!(ui.ctx(), "Cancel")))
                    .clicked()
                {
                    result = ColorResponse::Cancel;
                }
            });
        });
    if modal.should_close() {
        result = ColorResponse::Cancel;
    }
    result
}

fn palettes(ui: &mut egui::Ui, editor: &mut ColorEditor) {
    ui.label(t!(ui.ctx(), "Basic colors"));
    egui::Grid::new("historical-basic-colors")
        .min_col_width(0.0)
        .spacing(Vec2::splat(3.0))
        .show(ui, |ui| {
            for (index, rgb) in BASIC.into_iter().enumerate() {
                let label = t!(
                    ui.ctx(),
                    "Basic color #{ $hex }",
                    &[("hex", format!("{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]))],
                );
                if swatch(
                    ui,
                    Some(Rgba([rgb[0], rgb[1], rgb[2], 255])),
                    &label,
                    editor.color.0[..3] == rgb,
                )
                .clicked()
                {
                    editor.set_rgb(rgb);
                }
                if index % 8 == 7 {
                    ui.end_row();
                }
            }
        });
    ui.add_space(4.0);
    ui.label(t!(ui.ctx(), "Custom colors"));
    egui::Grid::new("historical-custom-colors")
        .min_col_width(0.0)
        .spacing(Vec2::splat(3.0))
        .show(ui, |ui| {
            for index in 0..CUSTOM_COLOR_COUNT {
                let label = t!(
                    ui.ctx(),
                    "Custom color { $number }",
                    &[("number", (index + 1).to_string())],
                );
                if swatch(
                    ui,
                    editor.custom[index],
                    &label,
                    editor.selected_slot == index,
                )
                .clicked()
                {
                    editor.selected_slot = index;
                    if let Some(color) = editor.custom[index] {
                        editor.set_rgb(color.0[..3].try_into().expect("three channels"));
                    }
                }
                if index % 8 == 7 {
                    ui.end_row();
                }
            }
        });
}

fn swatch(ui: &mut egui::Ui, color: Option<Rgba>, label: &str, selected: bool) -> egui::Response {
    let response = ui.add(
        egui::Button::new(" ")
            .frame(false)
            .min_size(Vec2::splat(24.0)),
    );
    if selected || response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(0.5),
            0,
            egui::Stroke::new(1.0, ui.visuals().text_color()),
            egui::StrokeKind::Inside,
        );
    }
    let rect = response.rect.shrink(3.0);
    ui.painter().rect_filled(
        rect,
        0,
        color.map_or(
            if ui.visuals().dark_mode {
                Color32::from_gray(32)
            } else {
                Color32::WHITE
            },
            |color| Color32::from_rgb(color.0[0], color.0[1], color.0[2]),
        ),
    );
    ui.painter().rect_stroke(
        rect,
        0,
        ui.visuals().widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label)
    });
    response.on_hover_text(label)
}

fn definition(ui: &mut egui::Ui, editor: &mut ColorEditor) {
    ui.horizontal_top(|ui| {
        picker::spectrum(ui, editor);
        picker::luminosity(ui, editor);
    });
    ui.horizontal_top(|ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(56.0, 76.0), egui::Sense::hover());
        ui.painter().rect_filled(
            rect,
            0,
            Color32::from_rgb(editor.color.0[0], editor.color.0[1], editor.color.0[2]),
        );
        ui.painter().rect_stroke(
            rect,
            0,
            ui.visuals().widgets.noninteractive.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let label = t!(ui.ctx(), "Current color preview");
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::ColorWell, ui.is_enabled(), &label)
        });
        ui.vertical(|ui| {
            ui.set_width(172.0);
            fields::numbers(ui, editor);
        });
    });
    fields::hex_entry(ui, editor);
    if ui
        .add_enabled(
            editor.valid(),
            egui::Button::new(t!(ui.ctx(), "Add custom color")).min_size(Vec2::new(234.0, 24.0)),
        )
        .clicked()
        && ui.ctx().current_pass_index() == 0
    {
        let mut color = editor.color;
        color.0[3] = 255;
        editor.custom[editor.selected_slot] = Some(color);
        editor.selected_slot = editor
            .custom
            .iter()
            .position(Option::is_none)
            .unwrap_or((editor.selected_slot + 1) % CUSTOM_COLOR_COUNT);
    }
}

// Conventional common-dialog RGB values, cross-checked against Microsoft's
// illustration and Wine's published numeric palette. See dialog evidence.
const BASIC: [[u8; 3]; 48] = [
    [255, 128, 128],
    [255, 255, 128],
    [128, 255, 128],
    [0, 255, 128],
    [128, 255, 255],
    [0, 128, 255],
    [255, 128, 192],
    [255, 128, 255],
    [255, 0, 0],
    [255, 255, 0],
    [128, 255, 0],
    [0, 255, 64],
    [0, 255, 255],
    [0, 128, 192],
    [128, 128, 192],
    [255, 0, 255],
    [128, 64, 64],
    [255, 128, 64],
    [0, 255, 0],
    [0, 128, 128],
    [0, 64, 128],
    [128, 128, 255],
    [128, 0, 64],
    [255, 0, 128],
    [128, 0, 0],
    [255, 128, 0],
    [0, 128, 0],
    [0, 128, 64],
    [0, 0, 255],
    [0, 0, 160],
    [128, 0, 128],
    [128, 0, 255],
    [64, 0, 0],
    [128, 64, 0],
    [0, 64, 0],
    [0, 64, 64],
    [0, 0, 128],
    [0, 0, 64],
    [64, 0, 64],
    [64, 0, 128],
    [0, 0, 0],
    [128, 128, 0],
    [128, 128, 64],
    [128, 128, 128],
    [64, 128, 128],
    [192, 192, 192],
    [64, 0, 64],
    [255, 255, 255],
];
