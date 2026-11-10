//! The Retro presentation emits commands through the existing native editor host.
use super::*;
use paintcraft_retro::{Intent, ToolChoice};
use paintcraft_ui::{chrome::Controls, t};
#[path = "retro/forms.rs"]
pub(super) mod forms;
#[path = "retro/pan.rs"]
mod pan;

impl App {
    fn retro_controls(&self) -> Controls {
        Controls {
            tool: self.editor.tool,
            options: self.editor.options,
            primary: self.editor.color,
            secondary: self.editor.secondary,
        }
    }

    pub(super) fn retro_toolbar(&mut self, ui: &mut egui::Ui) {
        let context = self.action_context();
        let intents = paintcraft_retro::menu_bar(ui, &mut self.retro, context);
        self.retro_intents(ui.ctx(), intents);
        if self.editor.tool == Tool::Text && self.retro.text_toolbar {
            let before = self.settings.text.clone();
            forms::text(ui, &mut self.settings.text, &self.fonts.families);
            self.finish_text_preferences(ui, &before);
            self.editor.set_text_style(self.settings.text.clone());
        }
    }

    pub(super) fn retro_panels(&mut self, ui: &mut egui::Ui) {
        let context = self.action_context();
        if self.retro.colors {
            egui::Panel::bottom("retro-colors")
                .frame(paintcraft_retro::frame())
                .show(ui, |ui| {
                    let mut controls = self.retro_controls();
                    let intents =
                        paintcraft_retro::palette(ui, &mut controls, &mut self.retro, context);
                    if ui.ctx().current_pass_index() == 0 && !context.blocked {
                        self.editor.color = controls.primary;
                        self.editor.secondary = controls.secondary;
                    }
                    self.retro_intents(ui.ctx(), intents);
                });
        }
        if self.retro.toolbox {
            egui::Panel::left("retro-toolbox")
                .resizable(false)
                .default_size(58.0)
                .frame(paintcraft_retro::frame())
                .show(ui, |ui| {
                    let mut controls = self.retro_controls();
                    let intents =
                        paintcraft_retro::toolbox(ui, &mut controls, &mut self.retro, context);
                    if ui.ctx().current_pass_index() == 0 && !context.blocked {
                        self.editor.options = controls.options;
                    }
                    self.retro_intents(ui.ctx(), intents);
                });
        }
    }

    fn retro_intents(&mut self, ctx: &egui::Context, intents: Vec<Intent>) {
        if ctx.current_pass_index() != 0 {
            return;
        }
        // Dispatch in widget order. Every operation rechecks its current owner/context.
        for intent in intents {
            match intent {
                Intent::Action(action) => self.dispatch_chrome(ctx, vec![action]),
                Intent::ChooseTool(choice) => self.retro_choose_tool(choice),
                Intent::About => self.open_about(ctx),
            }
        }
    }

    pub(super) fn retro_choose_tool(&mut self, choice: ToolChoice) {
        if self.experience() != Experience::Retro || self.action_context().blocked {
            return;
        }
        if self.editor.has_image_paste() || self.ime_composing {
            self.editor.set_status(t!(
                &self.context,
                "Finish pending edits before changing tools."
            ));
            return;
        }
        self.editor.commit_text();
        self.editor.finish_selection_move();
        self.editor.run_pending();
        if !self.editor.text_save_ready()
            || self.editor.has_pending_edits()
            || self.editor.has_floating_selection()
        {
            self.editor.set_status(t!(
                &self.context,
                "Finish pending edits before changing tools."
            ));
            return;
        }
        self.editor.select_tool(choice.tool());
        if self.editor.tool == choice.tool() {
            let mut controls = self.retro_controls();
            choice.apply_to(&mut controls);
            self.editor.options = controls.options;
        }
    }
}

#[cfg(test)]
#[path = "retro/tests.rs"]
mod tests;
