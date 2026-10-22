//! Classic font and text appearance controls.
pub use paintcraft_core::types::text::{TextAlignment, TextOptions};

use egui::{Align, Button, Color32, Response, RichText, Ui, Vec2};

mod fonts;
mod size;

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
                        fonts::family(ui, &mut options.family, families, search, loading);
                        size::font_size(ui, &mut options.points);
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

fn style_button(ui: &mut Ui, text: RichText, label: &str, selected: bool) -> Response {
    let response = ui.add(
        Button::new(text)
            .selected(selected)
            .min_size(Vec2::splat(36.0))
            .frame_when_inactive(selected)
            .frame(true),
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
