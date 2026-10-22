//! Classic font and text appearance controls.
pub use paintcraft_core::types::text::{TextAlignment, TextOptions};

use egui::{Align, Button, Color32, Response, RichText, Ui, Vec2};

const FONT_SIZES: [u16; 16] = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72];

pub fn text_options(
    ui: &mut Ui,
    options: &mut TextOptions,
    families: &[String],
    search: &mut String,
    loading: bool,
) {
    egui::Frame::new()
        .corner_radius(8)
        .fill(if ui.visuals().dark_mode {
            Color32::from_gray(43)
        } else {
            Color32::from_gray(245)
        })
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .inner_margin(8)
        .show(ui, |ui| {
            ui.spacing_mut().interact_size.y = 36.0;
            for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                ui.style_mut()
                    .text_styles
                    .insert(style, egui::FontId::proportional(16.0));
            }
            egui::ScrollArea::horizontal()
                .id_salt("classic-text-options")
                .max_height(42.0)
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        font_family(ui, &mut options.family, families, search, loading);
                        font_size(ui, &mut options.points);
                        ui.separator();
                        for (value, text, label) in [
                            (&mut options.bold, RichText::new("B").strong(), "Bold"),
                            (&mut options.italic, RichText::new("I").italics(), "Italic"),
                            (
                                &mut options.underline,
                                RichText::new("U").underline(),
                                "Underline",
                            ),
                            (
                                &mut options.strikeout,
                                RichText::new("S").strikethrough(),
                                "Strikeout",
                            ),
                        ] {
                            if style_button(ui, text.size(22.0), label, *value).clicked()
                                && ui.ctx().current_pass_index() == 0
                            {
                                *value = !*value;
                            }
                        }
                        ui.separator();
                        alignment(ui, &mut options.alignment);
                        ui.separator();
                        ui.scope(|ui| {
                            ui.spacing_mut().icon_width = 22.0;
                            ui.visuals_mut().widgets.inactive.bg_stroke =
                                egui::Stroke::new(1.0, ui.visuals().weak_text_color());
                            ui.checkbox(&mut options.background_fill, "Background fill")
                                .on_hover_text("Fill the text background with the secondary color");
                        });
                    });
                });
        });
}

fn font_family(
    ui: &mut Ui,
    family: &mut String,
    families: &[String],
    search: &mut String,
    loading: bool,
) {
    let font = egui::ComboBox::from_id_salt("classic-font-family")
        .width(220.0)
        .height(240.0)
        .selected_text(&*family)
        .show_ui(ui, |ui| {
            let find = ui.add(egui::TextEdit::singleline(search).hint_text("Search fonts"));
            find.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::TextInput, ui.is_enabled(), "Search fonts")
            });
            let filter = search.to_lowercase();
            let mut matches = 0;
            for name in families
                .iter()
                .filter(|name| name.to_lowercase().contains(&filter))
            {
                matches += 1;
                ui.selectable_value(family, name.clone(), name);
            }
            if loading {
                ui.weak("Loading installed fonts…");
            } else if matches == 0 {
                ui.weak("No matching fonts");
            }
        });
    font.response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::ComboBox, ui.is_enabled(), "Font family")
    });
    font.response.on_hover_text("Font family");
}

fn font_size(ui: &mut Ui, points: &mut u16) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let size = ui.add_sized([44.0, 36.0], egui::DragValue::new(points).range(1..=512));
        size.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::SpinButton, ui.is_enabled(), "Font size")
        });
        size.on_hover_text("Font size in points · type a value or choose a preset");
        let (response, _) = egui::containers::menu::MenuButton::from_button(
            Button::new(" ").min_size(Vec2::new(24.0, 36.0)),
        )
        .ui(ui, |ui| {
            ui.set_min_width(76.0);
            egui::ScrollArea::vertical()
                .id_salt("classic-font-size-presets")
                .max_height(280.0)
                .show(ui, |ui| {
                    for size in FONT_SIZES {
                        let choice = ui.selectable_label(*points == size, size.to_string());
                        choice.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::Button,
                                ui.is_enabled(),
                                *points == size,
                                format!("{size} pt"),
                            )
                        });
                        if choice.clicked() {
                            *points = size;
                            ui.close();
                        }
                    }
                });
        });
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "Font size presets")
        });
        let center = response.rect.center();
        ui.painter().add(egui::Shape::line(
            vec![
                center + Vec2::new(-3.0, -1.5),
                center + Vec2::new(0.0, 1.5),
                center + Vec2::new(3.0, -1.5),
            ],
            egui::Stroke::new(1.0, ui.visuals().text_color()),
        ));
        response.on_hover_text("Font size presets");
    });
}

fn style_button(ui: &mut Ui, text: RichText, label: &str, selected: bool) -> Response {
    let response = ui.add(
        Button::new(text)
            .selected(selected)
            .min_size(Vec2::splat(36.0))
            .frame(selected),
    );
    if selected {
        ui.painter().rect_stroke(
            response.rect.shrink(0.5),
            4,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label)
    });
    response.on_hover_text(label)
}

fn alignment(ui: &mut Ui, alignment: &mut TextAlignment) {
    for (value, label) in [
        (TextAlignment::Left, "Align left"),
        (TextAlignment::Center, "Align center"),
        (TextAlignment::Right, "Align right"),
    ] {
        let response = style_button(ui, RichText::new("     "), label, *alignment == value);
        let rect = response.rect.shrink(9.0);
        let ink = ui.visuals().text_color();
        draw_alignment(ui.painter(), rect, value, ink);
        if response.clicked() {
            *alignment = value;
        }
    }
}

fn draw_alignment(
    painter: &egui::Painter,
    rect: egui::Rect,
    alignment: TextAlignment,
    ink: Color32,
) {
    let anchor = match alignment {
        TextAlignment::Left => Align::Min,
        TextAlignment::Center => Align::Center,
        TextAlignment::Right => Align::Max,
    };
    for (i, fraction) in [1.0, 0.65, 1.0, 0.65].into_iter().enumerate() {
        let x = anchor.align_size_within_range(rect.width() * fraction, rect.x_range());
        let y = rect.center().y - 6.0 + i as f32 * 4.0;
        painter.line_segment(
            [egui::pos2(x.min, y), egui::pos2(x.max, y)],
            egui::Stroke::new(1.0, ink),
        );
    }
}
