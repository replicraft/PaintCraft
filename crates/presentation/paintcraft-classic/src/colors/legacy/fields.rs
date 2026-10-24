use super::*;
use paintcraft_ui::i18n::tr;

pub(super) fn native_hsl(editor: &ColorEditor) -> [u16; 3] {
    [
        (editor.hsl[0] * 240.0 / 360.0).round().clamp(0.0, 239.0) as u16,
        (editor.hsl[1] * 2.4).round().clamp(0.0, 240.0) as u16,
        (editor.hsl[2] * 2.4).round().clamp(0.0, 240.0) as u16,
    ]
}
pub(super) fn set_native(editor: &mut ColorEditor, [h, s, l]: [u16; 3]) {
    editor.set_hsl([
        f32::from(h.min(239)) * 1.5,
        f32::from(s.min(240)) / 2.4,
        f32::from(l.min(240)) / 2.4,
    ]);
}

pub(super) fn hex_entry(ui: &mut egui::Ui, editor: &mut ColorEditor) {
    ui.scope(|ui| {
        let fill = if ui.visuals().dark_mode {
            Color32::from_gray(32)
        } else {
            Color32::WHITE
        };
        ui.visuals_mut().extreme_bg_color = fill;
        ui.visuals_mut().widgets.inactive.bg_fill = fill;
        ui.visuals_mut().widgets.inactive.weak_bg_fill = fill;

        let response = ui.add(
            egui::TextEdit::singleline(&mut editor.hex)
                .desired_width(218.0)
                .hint_text("#RRGGBB"),
        );
        let label = tr(ui.ctx(), "Hex RGB");
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::TextInput, ui.is_enabled(), &label)
        });
        if response.changed()
            && let Some(rgb) = super::super::fields::parse_hex(&editor.hex)
        {
            let text = editor.hex.clone();
            editor.set_rgb(rgb);
            editor.hex = text;
        }
        if !editor.valid() {
            ui.weak(tr(ui.ctx(), "Enter six hex digits, such as #2080FF."));
        }
    });
}

pub(super) fn numbers(ui: &mut egui::Ui, editor: &mut ColorEditor) {
    ui.scope(|ui| {
        let fill = if ui.visuals().dark_mode {
            Color32::from_gray(32)
        } else {
            Color32::WHITE
        };
        ui.visuals_mut().extreme_bg_color = fill;
        ui.visuals_mut().widgets.inactive.bg_fill = fill;
        ui.visuals_mut().widgets.inactive.weak_bg_fill = fill;

        let mut rgb = [editor.color.0[0], editor.color.0[1], editor.color.0[2]];
        let mut hsl = native_hsl(editor);
        let mut rgb_changed = false;
        let mut hsl_changed = false;
        egui::Grid::new("historical-color-numbers")
            .min_col_width(0.0)
            .spacing(Vec2::new(4.0, 2.0))
            .show(ui, |ui| {
                for (index, (short, full, rgb_label)) in [
                    ("Hue", "Hue", "Red"),
                    ("Sat", "Saturation", "Green"),
                    ("Lum", "Luminosity", "Blue"),
                ]
                .into_iter()
                .enumerate()
                {
                    ui.label(tr(ui.ctx(), short));
                    let value = ui.add(
                        egui::DragValue::new(&mut hsl[index])
                            .range(0..=if index == 0 { 239 } else { 240 })
                            .min_size(Vec2::new(40.0, 24.0)),
                    );
                    let label = tr(ui.ctx(), full);
                    value.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::SpinButton, ui.is_enabled(), &label)
                    });
                    hsl_changed |= value.changed();
                    let caption = ui.label(tr(ui.ctx(), rgb_label));
                    rgb_changed |= ui
                        .add(
                            egui::DragValue::new(&mut rgb[index])
                                .range(0..=255)
                                .min_size(Vec2::new(40.0, 24.0)),
                        )
                        .labelled_by(caption.id)
                        .changed();
                    ui.end_row();
                }
            });
        if rgb_changed {
            editor.set_rgb(rgb);
        } else if hsl_changed {
            set_native(editor, hsl);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documented_windows_hsl_named_color_vectors_match_and_idle_values_preserve_rgb() {
        let mut editor = ColorEditor::new(Rgba([17, 93, 157, 91]), [None; CUSTOM_COLOR_COUNT]);
        let original = editor.color;
        let _ = native_hsl(&editor);
        assert_eq!(editor.color, original);
        for (h, rgb) in [
            (0, [255, 0, 0]),
            (40, [255, 255, 0]),
            (80, [0, 255, 0]),
            (120, [0, 255, 255]),
            (160, [0, 0, 255]),
            (200, [255, 0, 255]),
        ] {
            set_native(&mut editor, [h, 240, 120]);
            assert_eq!(editor.color.0[..3], rgb);
            assert_eq!(editor.color.0[3], 91);
        }
        set_native(&mut editor, [0, 0, 240]);
        assert_eq!(editor.color.0[..3], [255; 3]);
        set_native(&mut editor, [0, 0, 0]);
        assert_eq!(editor.color.0[..3], [0; 3]);
    }
}
