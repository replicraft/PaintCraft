//! Classic font and text appearance controls.
pub use paintcraft_core::types::text::{TextAlignment, TextOptions};

use egui::{Align, Button, Color32, Response, RichText, Ui, Vec2};
use paintcraft_ui::i18n::tr;

mod fonts;
pub use fonts::family as font_family;
mod popup;
mod size;

const CONTROL_HEIGHT: f32 = 40.0;
const FAMILY_WIDTH: f32 = 238.0;

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
        .inner_margin(egui::Margin::symmetric(20, 10))
        .show(ui, |ui| {
            ui.spacing_mut().interact_size.y = CONTROL_HEIGHT;
            ui.spacing_mut().item_spacing.x = 12.0;
            for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                ui.style_mut()
                    .text_styles
                    .insert(style, egui::FontId::proportional(16.0));
            }
            egui::ScrollArea::horizontal()
                .id_salt("classic-text-options")
                .max_height(CONTROL_HEIGHT + 6.0)
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
                            ui.spacing_mut().icon_width = 24.0;
                            crate::form_controls::style(ui);
                            ui.checkbox(
                                &mut options.background_fill,
                                tr(ui.ctx(), "Background fill"),
                            )
                            .on_hover_text(tr(
                                ui.ctx(),
                                "Fill the text background with the secondary color",
                            ));
                        });
                    });
                });
        });
}

fn style_button(ui: &mut Ui, text: RichText, label: &str, selected: bool) -> Response {
    let response = crate::control_surface::button(
        ui,
        Button::new(text)
            .min_size(Vec2::splat(CONTROL_HEIGHT))
            .frame_when_inactive(selected)
            .frame(true),
        selected,
    );
    let label = tr(ui.ctx(), label);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, &label)
    });
    response.on_hover_text(tr(ui.ctx(), label))
}

fn alignment(ui: &mut Ui, alignment: &mut TextAlignment) {
    for (value, label) in [
        (TextAlignment::Left, "Align left"),
        (TextAlignment::Center, "Align center"),
        (TextAlignment::Right, "Align right"),
    ] {
        let response = style_button(ui, RichText::new("     "), label, *alignment == value);
        let rect = response.rect.shrink(9.0);
        let ink = ui.visuals().strong_text_color();
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

fn selected_outline(ui: &Ui, response: &Response, selected: bool) {
    if selected {
        ui.painter().rect_stroke(
            response.rect.shrink(0.5),
            4,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
}
