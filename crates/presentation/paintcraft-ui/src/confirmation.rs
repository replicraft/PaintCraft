//! Bounded confirmation presentation. Callers own and gate all side effects.
use crate::t;
use egui::{Color32, Context, Id, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Accept,
    Discard,
    Cancel,
}

#[derive(Debug)]
pub struct Prompt<'a> {
    pub title: &'a str,
    pub message: &'a str,
    pub accept: &'a str,
    pub discard: Option<&'a str>,
}

pub fn show(ctx: &Context, id: Id, prompt: Prompt<'_>) -> Option<Choice> {
    let screen = ctx.input(|input| input.content_rect());
    let width = (screen.width() - 56.0).clamp(240.0, 560.0);
    let height = (screen.height() - 72.0).clamp(180.0, 480.0);
    let count = if prompt.discard.is_some() { 3 } else { 2 };
    let mut choice = None;
    let frame = egui::Frame::window(&ctx.style_of(ctx.theme()))
        .inner_margin(16)
        .corner_radius(12);
    let response = egui::Modal::new(id).frame(frame).show(ctx, |ui| {
        ui.set_width(width);
        ui.set_max_height(height);
        for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
            ui.style_mut()
                .text_styles
                .insert(style, egui::FontId::proportional(16.0));
        }
        ui.visuals_mut().override_text_color = Some(ui.visuals().strong_text_color());
        let widest = [Some(prompt.accept), prompt.discard, Some("Cancel")]
            .into_iter()
            .flatten()
            .map(|label| {
                ui.painter()
                    .layout_no_wrap(
                        t!(ctx, label),
                        egui::FontId::proportional(16.0),
                        Color32::WHITE,
                    )
                    .size()
                    .x
            })
            .fold(0.0_f32, f32::max);
        // Include padding, frame strokes and rounding when checking a row.
        let minimum_button = widest + ui.spacing().button_padding.x * 2.0 + 4.0;
        let stacked = width
            < minimum_button * count as f32 + ui.spacing().item_spacing.x * (count - 1) as f32;
        let footer_height = if stacked {
            count as f32 * 40.0 + (count - 1) as f32 * ui.spacing().item_spacing.y
        } else {
            40.0
        };
        let heading = ui.label(
            egui::RichText::new(t!(ctx, prompt.title))
                .size(24.0)
                .strong(),
        );
        ui.add_space(8.0);
        ui.spacing_mut().scroll.floating = false;
        egui::ScrollArea::vertical()
            .id_salt("confirmation-message")
            .max_height((height - footer_height - heading.rect.height() - 40.0).max(24.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.add(egui::Label::new(t!(ctx, prompt.message)).wrap());
            });
        ui.add_space(12.0);
        ui.separator();
        let mut buttons = |ui: &mut egui::Ui| {
            let button_width = if stacked {
                width
            } else {
                (width - ui.spacing().item_spacing.x * (count - 1) as f32) / count as f32
            };
            for (label, value) in [
                (Some(prompt.accept), Choice::Accept),
                (prompt.discard, Choice::Discard),
                (Some("Cancel"), Choice::Cancel),
            ] {
                let Some(label) = label else {
                    continue;
                };
                let mut button = egui::Button::new(t!(ctx, label));
                if value == Choice::Accept {
                    let ink = if ui.visuals().dark_mode {
                        Color32::from_rgb(16, 16, 16)
                    } else {
                        Color32::WHITE
                    };
                    button = egui::Button::new(egui::RichText::new(t!(ctx, label)).color(ink))
                        .fill(ui.visuals().selection.stroke.color);
                }
                if ui
                    .add_sized(Vec2::new(button_width, 40.0), button)
                    .clicked()
                {
                    choice = Some(value);
                }
            }
        };
        if stacked {
            ui.vertical(&mut buttons);
        } else {
            ui.horizontal(&mut buttons);
        }
    });
    choice.or_else(|| response.should_close().then_some(Choice::Cancel))
}
