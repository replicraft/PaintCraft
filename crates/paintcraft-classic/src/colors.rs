//! Staged Classic color picker. The host adopts the result only on OK.
mod fields;
mod picker;
use egui::{Color32, Vec2, ecolor::Hsva};
use paintcraft_core::types::Rgba;

pub const CUSTOM_COLOR_COUNT: usize = 28;
pub type CustomColors = [Option<Rgba>; CUSTOM_COLOR_COUNT];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorResponse {
    Pending,
    Accept,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumericMode {
    Rgb,
    Hsl,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColorEditor {
    pub color: Rgba,
    pub custom: CustomColors,
    hsv: Hsva,
    hsl: [f32; 3],
    hex: String,
    mode: NumericMode,
    selected_slot: usize,
}

impl ColorEditor {
    pub fn new(color: Rgba, custom: CustomColors) -> Self {
        let mut editor = Self {
            color,
            custom,
            hsv: Hsva::default(),
            hsl: [0.0; 3],
            hex: String::new(),
            mode: NumericMode::Rgb,
            selected_slot: 0,
        };
        editor.set_rgb(color.0[..3].try_into().expect("three channels"));
        editor.selected_slot = editor.custom.iter().position(Option::is_none).unwrap_or(0);
        editor
    }

    fn set_rgb(&mut self, rgb: [u8; 3]) {
        let previous = self.hsv;
        self.hsv = Hsva::from_srgba_unmultiplied([rgb[0], rgb[1], rgb[2], 255]);
        if self.hsv.s == 0.0 {
            self.hsv.h = previous.h;
        }
        if self.hsv.v == 0.0 {
            self.hsv.s = previous.s;
        }
        self.color.0[..3].copy_from_slice(&rgb);
        self.hex = format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
        fields::sync_hsl(self);
    }

    fn set_hsv(&mut self) {
        let rgb = self.hsv.to_srgba_unmultiplied();
        self.color.0[..3].copy_from_slice(&rgb[..3]);
        self.hex = format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
        fields::sync_hsl(self);
    }

    fn valid(&self) -> bool {
        fields::parse_hex(&self.hex).is_some()
    }
}

pub fn edit_colors_dialog(ctx: &egui::Context, editor: &mut ColorEditor) -> ColorResponse {
    let mut result = ColorResponse::Pending;
    let modal = egui::Modal::new(egui::Id::unique("classic-edit-colors")).show(ctx, |ui| {
        ui.set_width(620.0);
        ui.heading("Edit colors");
        ui.add_space(12.0);
        egui::ScrollArea::vertical()
            .max_height((ctx.viewport_rect().height() - 160.0).max(160.0))
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    picker::square(ui, editor);
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(36.0, 220.0), egui::Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        4.0,
                        Color32::from_rgb(editor.color.0[0], editor.color.0[1], editor.color.0[2]),
                    );
                    picker::brightness(ui, editor);
                    ui.add_space(8.0);
                    ui.vertical(|ui| fields::controls(ui, editor));
                });
                ui.add_space(18.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Basic colors");
                        palette(ui, editor);
                    });
                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("Custom colors");
                            let add = ui.add_enabled(editor.valid(), egui::Button::new("+"));
                            add.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::Button,
                                    ui.is_enabled() && editor.valid(),
                                    "Add custom color",
                                )
                            });
                            if add
                                .on_hover_text("Add current color to the selected custom slot")
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
                        });
                        custom_palette(ui, editor);
                    });
                });
            });
        ui.add_space(12.0);
        ui.separator();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    editor.valid(),
                    egui::Button::new("OK").min_size(Vec2::new(295.0, 30.0)),
                )
                .clicked()
            {
                result = ColorResponse::Accept;
            }
            if ui
                .add_sized([295.0, 30.0], egui::Button::new("Cancel"))
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

fn swatch(ui: &mut egui::Ui, color: Option<Rgba>, label: &str, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(23.0), egui::Sense::click());
    let stroke = egui::Stroke::new(
        if selected { 2.0 } else { 1.0 },
        if selected {
            ui.visuals().selection.bg_fill
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke.color
        },
    );
    if let Some(color) = color {
        ui.painter().circle_filled(
            rect.center(),
            9.0,
            Color32::from_rgb(color.0[0], color.0[1], color.0[2]),
        );
    }
    ui.painter().circle_stroke(rect.center(), 9.0, stroke);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    response.on_hover_text(label)
}

fn palette(ui: &mut egui::Ui, editor: &mut ColorEditor) {
    egui::Grid::new("classic-basic-colors")
        .num_columns(12)
        .min_col_width(0.0)
        .spacing(Vec2::splat(2.0))
        .show(ui, |ui| {
            for (i, rgb) in BASIC.into_iter().enumerate() {
                let label = format!("Basic #{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
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
                if i % 12 == 11 {
                    ui.end_row();
                }
            }
        });
}

fn custom_palette(ui: &mut egui::Ui, editor: &mut ColorEditor) {
    egui::Grid::new("classic-custom-colors")
        .num_columns(7)
        .min_col_width(0.0)
        .spacing(Vec2::splat(2.0))
        .show(ui, |ui| {
            for i in 0..CUSTOM_COLOR_COUNT {
                if swatch(
                    ui,
                    editor.custom[i],
                    &format!("Custom color {}", i + 1),
                    editor.selected_slot == i,
                )
                .clicked()
                {
                    editor.selected_slot = i;
                    if let Some(color) = editor.custom[i] {
                        editor.set_rgb(color.0[..3].try_into().expect("three channels"));
                    }
                }
                if i % 7 == 6 {
                    ui.end_row();
                }
            }
        });
}

const BASIC: [[u8; 3]; 48] = [
    [240, 128, 128],
    [240, 48, 32],
    [112, 64, 64],
    [144, 64, 48],
    [96, 32, 16],
    [160, 255, 255],
    [96, 240, 240],
    [48, 128, 240],
    [0, 64, 255],
    [0, 0, 208],
    [16, 64, 128],
    [0, 0, 128],
    [255, 255, 160],
    [255, 255, 64],
    [240, 160, 80],
    [240, 128, 64],
    [128, 64, 16],
    [128, 128, 32],
    [128, 128, 255],
    [128, 32, 240],
    [48, 128, 176],
    [0, 0, 48],
    [96, 16, 96],
    [64, 0, 64],
    [160, 255, 128],
    [160, 255, 48],
    [96, 240, 32],
    [96, 255, 64],
    [96, 255, 128],
    [128, 128, 64],
    [240, 128, 192],
    [240, 128, 255],
    [224, 32, 240],
    [240, 32, 128],
    [128, 128, 192],
    [128, 16, 64],
    [32, 112, 16],
    [48, 128, 64],
    [48, 128, 128],
    [80, 128, 128],
    [16, 64, 0],
    [0, 64, 64],
    [128, 16, 128],
    [64, 0, 128],
    [0, 0, 0],
    [128, 128, 128],
    [192, 192, 192],
    [255, 255, 255],
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rgb_hsv_and_hsl_preserve_alpha_and_retain_chromatic_intent_at_black() {
        for alpha in [0, 93, 255] {
            let mut editor =
                ColorEditor::new(Rgba([12, 34, 56, alpha]), [None; CUSTOM_COLOR_COUNT]);
            assert_eq!(editor.color, Rgba([12, 34, 56, alpha]));
            editor.set_rgb([0, 0, 0]);
            editor.set_hsl([240.0, 100.0, 0.0]);
            assert_eq!(editor.color, Rgba([0, 0, 0, alpha]));
            editor.set_hsl([240.0, 100.0, 50.0]);
            assert_eq!(editor.color, Rgba([0, 0, 255, alpha]));
            editor.hsv.v = 0.0;
            editor.set_hsv();
            editor.hsv.h = 1.0 / 3.0;
            editor.hsv.s = 1.0;
            editor.set_hsv();
            editor.hsv.v = 1.0;
            editor.set_hsv();
            assert_eq!(editor.color, Rgba([0, 255, 0, alpha]));
        }
    }
}
