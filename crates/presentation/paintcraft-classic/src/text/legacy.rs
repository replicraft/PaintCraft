//! Borrowed font preferences let the historical ribbon reuse the text editor controls.
use super::{Metrics, TextAlignment, TextOptions, fonts, size};
use crate::ClassicVariant;
use egui::{RichText, Vec2};
use paintcraft_ui::t;
#[derive(Debug)]
pub struct TextControls<'a> {
    pub options: &'a mut TextOptions,
    pub families: &'a [String],
    pub search: &'a mut String,
    pub loading: bool,
}

impl TextControls<'_> {
    pub(crate) fn historical(self, ui: &mut egui::Ui, variant: ClassicVariant) {
        ui.spacing_mut().interact_size.y = Metrics::LEGACY.height;
        let options = self.options;
        crate::legacy_ribbon::group(ui, "Font", |ui| {
            fonts::family_with_metrics(
                ui,
                &mut options.family,
                self.families,
                self.search,
                self.loading,
                Metrics::LEGACY,
            );
            ui.horizontal(|ui| {
                size::font_size_with_metrics(ui, &mut options.points, Metrics::LEGACY);
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
                    let label = t!(ui.ctx(), label);
                    let response = crate::legacy_surface::button(
                        ui,
                        variant,
                        egui::Button::new(text.size(14.0)).min_size(Vec2::splat(24.0)),
                        *value,
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::Role::Button,
                            ui.is_enabled(),
                            *value,
                            &label,
                        )
                    });
                    if response.on_hover_text(label).clicked() && ui.ctx().current_pass_index() == 0
                    {
                        *value = !*value;
                    }
                }
            });
        });
        ui.separator();
        crate::legacy_ribbon::group(ui, "Background", |ui| {
            for (fill, label) in [(true, "Opaque"), (false, "Transparent")] {
                let label = t!(ui.ctx(), label);
                let selected = options.background_fill == fill;
                let response = crate::legacy_surface::button(
                    ui,
                    variant,
                    egui::Button::new(&label).min_size(Vec2::new(108.0, 24.0)),
                    selected,
                );
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::Role::Button,
                        ui.is_enabled(),
                        selected,
                        &label,
                    )
                });
                if response.clicked() {
                    options.background_fill = fill;
                }
            }
        });
        ui.separator();
        crate::legacy_ribbon::group(ui, "Alignment", |ui| {
            ui.horizontal(|ui| {
                for (alignment, label) in [
                    (TextAlignment::Left, "Align left"),
                    (TextAlignment::Center, "Align center"),
                    (TextAlignment::Right, "Align right"),
                ] {
                    let label = t!(ui.ctx(), label);
                    let selected = options.alignment == alignment;
                    let response = crate::legacy_surface::button(
                        ui,
                        variant,
                        egui::Button::new(" ").min_size(Vec2::splat(24.0)),
                        selected,
                    );
                    super::draw_alignment(
                        ui.painter(),
                        response.rect.shrink(5.0),
                        alignment,
                        ui.visuals().text_color(),
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::Role::Button,
                            ui.is_enabled(),
                            selected,
                            &label,
                        )
                    });
                    if response.on_hover_text(label).clicked() {
                        options.alignment = alignment;
                    }
                }
            });
        });
    }
}
