use super::*;
use paintcraft_ui::t;
pub(super) fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let text = text.trim().strip_prefix('#').unwrap_or(text.trim());
    if text.len() != 6 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some([
        u8::from_str_radix(&text[0..2], 16).ok()?,
        u8::from_str_radix(&text[2..4], 16).ok()?,
        u8::from_str_radix(&text[4..6], 16).ok()?,
    ])
}

pub(super) fn controls(ui: &mut egui::Ui, editor: &mut ColorEditor, width: f32) {
    ui.set_width(width);
    let field_width = (width - 100.0).clamp(100.0, 150.0);
    let response = ui.add(
        egui::TextEdit::singleline(&mut editor.hex)
            .desired_width(field_width)
            .margin(Vec2::new(14.0, 8.0))
            .hint_text("#RRGGBB"),
    );
    let accessible_label = t!(ui.ctx(), "Hex RGB");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::TextInput, ui.is_enabled(), &accessible_label)
    });
    if response.changed()
        && let Some(rgb) = parse_hex(&editor.hex)
    {
        let text = editor.hex.clone();
        editor.set_rgb(rgb);
        editor.hex = text;
    }
    response.on_hover_text(t!(ui.ctx(), "Hex RGB"));
    if !editor.valid() {
        ui.weak(t!(ui.ctx(), "Enter six hex digits, such as #2080FF."));
    }
    ui.add_space(8.0);
    let model = ui
        .scope(|ui| {
            ui.spacing_mut().button_padding.x = 14.0;
            egui::ComboBox::from_id_salt("classic-color-model")
                .width(field_width)
                .height(100.0)
                .icon(|ui, rect, visuals, _| {
                    let center = rect.center();
                    ui.painter().add(egui::Shape::line(
                        vec![
                            center + Vec2::new(-4.0, -2.0),
                            center + Vec2::new(0.0, 2.0),
                            center + Vec2::new(4.0, -2.0),
                        ],
                        egui::Stroke::new(1.0, visuals.fg_stroke.color),
                    ));
                })
                .selected_text(match editor.mode {
                    NumericMode::Rgb => "RGB",
                    NumericMode::Hsl => "HSL",
                })
                .show_ui(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.spacing_mut().button_padding.x = 14.0;
                    for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                        ui.style_mut()
                            .text_styles
                            .insert(style, egui::FontId::proportional(16.0));
                    }
                    for (mode, label) in [(NumericMode::Rgb, "RGB"), (NumericMode::Hsl, "HSL")] {
                        let selected = editor.mode == mode;
                        let response = ui.add(
                            egui::Button::new(())
                                .left_text(label)
                                .selected(selected)
                                .frame_when_inactive(selected)
                                .frame(true)
                                .min_size(Vec2::new(field_width, 40.0)),
                        );
                        if selected {
                            ui.painter().rect_stroke(
                                response.rect.shrink(0.5),
                                4,
                                ui.visuals().selection.stroke,
                                egui::StrokeKind::Inside,
                            );
                        }
                        if response.clicked() {
                            editor.mode = mode;
                            ui.close();
                        }
                    }
                })
        })
        .inner;
    let accessible_label = t!(ui.ctx(), "Color model");
    model.response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::ComboBox, ui.is_enabled(), &accessible_label)
    });
    model.response.on_hover_text(t!(ui.ctx(), "Color model"));
    ui.add_space(8.0);
    match editor.mode {
        NumericMode::Rgb => {
            let mut rgb: [u8; 3] = editor.color.0[..3].try_into().expect("three channels");
            let mut changed = false;
            for (i, label) in ["Red", "Green", "Blue"].into_iter().enumerate() {
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), 40.0),
                    egui::Layout::left_to_right(egui::Align::Center)
                        .with_main_align(egui::Align::Min),
                    |ui| {
                        ui.spacing_mut().button_padding.x = 14.0;
                        ui.style_mut().drag_value_text_style = egui::TextStyle::Body;
                        let value = ui.add(
                            egui::DragValue::new(&mut rgb[i])
                                .min_size(Vec2::new(field_width, 40.0))
                                .range(0..=255),
                        );
                        let caption = ui.label(t!(ui.ctx(), label));
                        changed |= value.labelled_by(caption.id).changed();
                    },
                );
            }
            if changed {
                editor.set_rgb(rgb);
            }
        }
        NumericMode::Hsl => {
            let mut hsl = editor.hsl;
            let mut changed = false;
            for (i, label, max) in [
                (0, "Hue", 360.0),
                (1, "Saturation", 100.0),
                (2, "Lightness", 100.0),
            ] {
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), 40.0),
                    egui::Layout::left_to_right(egui::Align::Center)
                        .with_main_align(egui::Align::Min),
                    |ui| {
                        ui.spacing_mut().button_padding.x = 14.0;
                        ui.style_mut().drag_value_text_style = egui::TextStyle::Body;
                        let value = ui.add(
                            egui::DragValue::new(&mut hsl[i])
                                .min_size(Vec2::new(field_width, 40.0))
                                .speed(0.5)
                                .range(0.0..=max),
                        );
                        let caption = ui.label(t!(ui.ctx(), label));
                        changed |= value.labelled_by(caption.id).changed();
                    },
                );
            }
            if changed {
                editor.set_hsl(hsl);
            }
        }
    }
}

pub(super) fn sync_hsl(editor: &mut ColorEditor) {
    let previous = editor.hsl;
    editor.hsl = to_hsl(editor.color, editor.hsv.h);
    if editor.hsl[2] == 0.0 || editor.hsl[2] == 100.0 {
        editor.hsl[1] = previous[1];
    }
}

impl ColorEditor {
    pub(super) fn set_hsl(&mut self, hsl: [f32; 3]) {
        self.set_rgb(from_hsl(hsl));
        self.hsl = hsl;
        self.hsv.h = (hsl[0] / 360.0).rem_euclid(1.0);
        if hsl[2] == 0.0 {
            let [r, g, b] = from_hsl([hsl[0], hsl[1], 50.0]);
            self.hsv.s = Hsva::from_srgba_unmultiplied([r, g, b, 255]).s;
        }
    }
}

fn to_hsl(color: Rgba, fallback_hue: f32) -> [f32; 3] {
    let [r, g, b] = color.0[..3]
        .try_into()
        .map(|rgb: [u8; 3]| rgb.map(|v| f32::from(v) / 255.0))
        .expect("three channels");
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let light = (max + min) / 2.0;
    let hue = if d == 0.0 {
        fallback_hue
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    let saturation = if d == 0.0 {
        0.0
    } else {
        d / (1.0 - (2.0 * light - 1.0).abs())
    };
    [hue * 360.0, saturation * 100.0, light * 100.0]
}

pub(super) fn from_hsl([h, s, l]: [f32; 3]) -> [u8; 3] {
    let h = h.rem_euclid(360.0) / 60.0;
    let (s, l) = (s.clamp(0.0, 100.0) / 100.0, l.clamp(0.0, 100.0) / 100.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let rgb = match h as u32 {
        0 => [c, x, 0.0],
        1 => [x, c, 0.0],
        2 => [0.0, c, x],
        3 => [0.0, x, c],
        4 => [x, 0.0, c],
        _ => [c, 0.0, x],
    };
    rgb.map(|v| ((v + l - c / 2.0) * 255.0).round().clamp(0.0, 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hex_and_hsl_are_exact_for_named_colors_and_roundtrip_byte_channels() {
        assert_eq!(parse_hex("#2080ff"), Some([32, 128, 255]));
        for text in ["#12", "#12345678", "#GG0000", "é00000"] {
            assert_eq!(parse_hex(text), None);
        }
        assert_eq!(from_hsl([120.0, 100.0, 50.0]), [0, 255, 0]);
        assert_eq!(from_hsl([240.0, 100.0, 50.0]), [0, 0, 255]);
        for r in [0, 1, 32, 128, 254, 255] {
            for g in [0, 32, 128, 255] {
                for b in [0, 64, 192, 255] {
                    let rgba = Rgba([r, g, b, 0]);
                    assert_eq!(from_hsl(to_hsl(rgba, 0.0)), [r, g, b]);
                }
            }
        }
    }
}
