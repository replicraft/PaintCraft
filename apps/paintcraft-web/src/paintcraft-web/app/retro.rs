//! Real Retro host bridge; all painting/history/file work uses existing services.
use super::*;
pub(crate) fn scoped_ui<R>(
    frontend: Frontend,
    ui: &mut egui::Ui,
    draw: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    match frontend {
        Frontend::Retro => paintcraft_retro::with_style(ui, draw),
        Frontend::Classic | Frontend::Studio => draw(ui),
    }
}
impl WebApp {
    pub(super) fn initial_size(
        frontend: Frontend,
        prefs: &crate::preferences::Preferences,
    ) -> (u32, u32) {
        match frontend {
            Frontend::Classic | Frontend::Retro => prefs.canvas.new_size(),
            Frontend::Studio => (800, 600),
        }
    }
    pub(super) fn status_bar_visible(&self) -> bool {
        match self.frontend {
            Frontend::Classic => self.preferences.status_bar,
            Frontend::Studio => true,
            Frontend::Retro => self.retro.status,
        }
    }
    pub(crate) fn retro_canvas_id(&self) -> egui::Id {
        egui::Id::unique((
            "web-retro-canvas",
            self.editor.session.history_ticket().owner,
        ))
    }
    pub(crate) fn retro_canvas_scope(&self) -> egui::Id {
        egui::Id::unique((
            self.editor.session.history_ticket().owner,
            self.zoom.to_bits(),
            Frontend::Retro as u8,
        ))
    }
    pub(super) fn retro_intents(
        &mut self,
        ctx: &egui::Context,
        intents: Vec<paintcraft_retro::Intent>,
    ) {
        for intent in intents {
            match intent {
                paintcraft_retro::Intent::Action(action) if action.enabled(self.context()) => {
                    self.action(ctx, action)
                }
                paintcraft_retro::Intent::Action(_) => (),
                paintcraft_retro::Intent::About if !self.blocked() => self.open_about(ctx),
                paintcraft_retro::Intent::About => (),
                paintcraft_retro::Intent::ChooseTool(choice) => {
                    self.choose_retro_tool(ctx, choice);
                }
            }
        }
    }
    pub(super) fn choose_retro_tool(
        &mut self,
        ctx: &egui::Context,
        choice: paintcraft_retro::ToolChoice,
    ) -> bool {
        if self.frontend != Frontend::Retro
            || self.blocked()
            || !self.floating_file_ready()
            || !self.finish_text()
        {
            return false;
        }
        let tool = choice.tool();
        self.action(ctx, Action::SelectTool(tool));
        if self.controls.tool != tool || self.editor.tools.tool != tool {
            return false;
        }
        choice.apply_to(&mut self.controls);
        self.editor.tools.options = self.controls.options;
        ctx.request_repaint();
        true
    }
    pub(super) fn retro_panels(&mut self, ui: &mut egui::Ui) {
        if self.frontend != Frontend::Retro {
            return;
        }
        let context = self.context();
        let ctx = ui.ctx().clone();
        if self.retro.colors {
            let intents = egui::Panel::bottom("web-retro-colors")
                .frame(paintcraft_retro::frame())
                .show(ui, |ui| {
                    paintcraft_retro::palette(ui, &mut self.controls, &mut self.retro, context)
                })
                .inner;
            self.retro_intents(&ctx, intents);
        }
        if self.retro.toolbox {
            let intents = egui::Panel::left("web-retro-tools")
                .frame(paintcraft_retro::frame())
                .show(ui, |ui| {
                    paintcraft_retro::toolbox(ui, &mut self.controls, &mut self.retro, context)
                })
                .inner;
            self.retro_intents(&ctx, intents);
        }
    }
    pub(super) fn retro_status(&mut self, ui: &mut egui::Ui) {
        let selection = self
            .editor
            .tools
            .selection_preview_size(self.editor.session.document().canvas_rect())
            .or_else(|| self.selected_dimensions());
        let message = tr_status(ui.ctx(), &self.message);
        let intents = paintcraft_retro::status(
            ui,
            paintcraft_retro::Status {
                message: &message,
                recovery: None,
                canvas: Some(self.editor.session.document().canvas),
                pointer: self.canvas.pointer,
                selection,
                zoom: self.zoom,
                blocked: self.blocked(),
            },
        );
        let ctx = ui.ctx().clone();
        self.retro_intents(&ctx, intents);
    }
    pub(crate) fn retro_text_options(&mut self, ui: &mut egui::Ui) {
        paintcraft_retro::with_style(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let family = ui.label(t!(ui.ctx(), "Default font"));
                egui::ComboBox::from_id_salt("retro-text-family")
                    .selected_text(&self.text.style.family)
                    .show_ui(ui, |ui| {
                        for family in &self.text.families {
                            ui.selectable_value(
                                &mut self.text.style.family,
                                family.clone(),
                                family,
                            );
                        }
                    })
                    .response
                    .labelled_by(family.id);
                let size = ui.label(t!(ui.ctx(), "Font size"));
                ui.add(egui::DragValue::new(&mut self.text.style.points).range(1..=512))
                    .labelled_by(size.id);
                ui.checkbox(&mut self.text.style.bold, t!(ui.ctx(), "Bold"));
                ui.checkbox(&mut self.text.style.italic, t!(ui.ctx(), "Italic"));
                ui.checkbox(&mut self.text.style.underline, t!(ui.ctx(), "Underline"));
                ui.checkbox(
                    &mut self.text.style.background_fill,
                    t!(ui.ctx(), "Opaque background"),
                );
            });
        });
    }
    pub(crate) fn configure_paint_style_tools(&mut self) {
        use paintcraft_core::types::picker::{PickerAfter, SampleSize, SampleSource};
        match self.frontend {
            Frontend::Studio => {
                self.editor.tools.options.eraser_color = paintcraft_core::types::Rgba::TRANSPARENT
            }
            Frontend::Classic | Frontend::Retro => {
                if self.controls.tool == Tool::ColorPicker {
                    self.editor.tools.options.picker_source = SampleSource::Image;
                    self.editor.tools.options.picker_size = SampleSize::One;
                    self.editor.tools.options.picker_after = PickerAfter::Previous;
                }
                self.editor.tools.options.eraser_color =
                    paintcraft_tools::commands::canvas_color(self.editor.session.document());
            }
        }
    }
}
#[cfg(test)]
#[path = "retro/tests.rs"]
mod tests;
