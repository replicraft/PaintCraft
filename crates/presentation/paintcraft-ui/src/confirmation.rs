//! Bounded confirmation presentation. Callers own and gate all side effects.
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
    let width = (screen.width() - 48.0).clamp(240.0, 560.0);
    let height = (screen.height() - 64.0).clamp(180.0, 480.0);
    let count = if prompt.discard.is_some() { 3 } else { 2 };
    let stacked = width < if count == 3 { 500.0 } else { 340.0 };
    let footer_height = if stacked { count as f32 * 44.0 } else { 44.0 };
    let mut choice = None;
    let response = egui::Modal::new(id).show(ctx, |ui| {
        ui.set_width(width);
        ui.set_max_height(height);
        for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
            ui.style_mut()
                .text_styles
                .insert(style, egui::FontId::proportional(15.0));
        }
        ui.label(egui::RichText::new(prompt.title).size(18.0).strong());
        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .id_salt("confirmation-message")
            .max_height((height - footer_height - 72.0).max(32.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.add(egui::Label::new(prompt.message).wrap());
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
                let mut button = egui::Button::new(label);
                if value == Choice::Accept {
                    let ink = if ui.visuals().dark_mode {
                        Color32::from_rgb(16, 16, 16)
                    } else {
                        Color32::WHITE
                    };
                    button = egui::Button::new(egui::RichText::new(label).color(ink))
                        .fill(ui.visuals().selection.stroke.color);
                }
                if ui
                    .add_sized(Vec2::new(button_width, 36.0), button)
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
