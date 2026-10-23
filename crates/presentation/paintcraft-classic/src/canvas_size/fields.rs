//! Shared Classic dimension rows keep captions and numeric text left aligned.
use egui::{Align, DragValue, FontId, Layout, TextStyle, Ui, Vec2};

/// Render document-pixel dimensions. The host retains draft ownership and range
/// policy; these native egui fields only edit the supplied values.
pub fn dimension_fields(ui: &mut Ui, size: &mut (u32, u32), maximum: u32, labels: [&str; 2]) {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding.x = 14.0;
        ui.style_mut().drag_value_text_style = TextStyle::Body;
        for style in [TextStyle::Body, TextStyle::Button] {
            ui.style_mut()
                .text_styles
                .insert(style, FontId::proportional(16.0));
        }
        let font = TextStyle::Body.resolve(ui.style());
        let caption_width = labels
            .iter()
            .map(|label| {
                ui.painter()
                    .layout_no_wrap((*label).to_owned(), font.clone(), ui.visuals().text_color())
                    .size()
                    .x
            })
            .fold(72.0, f32::max)
            .ceil();
        for (label, value) in [(labels[0], &mut size.0), (labels[1], &mut size.1)] {
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 40.0),
                Layout::left_to_right(Align::Center).with_main_align(Align::Min),
                |ui| {
                    let caption = ui
                        .allocate_ui_with_layout(
                            Vec2::new(caption_width, 40.0),
                            Layout::left_to_right(Align::Center).with_main_align(Align::Min),
                            |ui| {
                                ui.set_width(caption_width);
                                ui.label(label)
                            },
                        )
                        .inner;
                    let field_width = ui.available_width().clamp(80.0, 220.0);
                    ui.add(
                        DragValue::new(value)
                            .min_size(Vec2::new(field_width, 40.0))
                            .range(1..=maximum)
                            .suffix(" px"),
                    )
                    .labelled_by(caption.id);
                },
            );
        }
    });
}
