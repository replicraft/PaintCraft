//! Presentation only. The host freezes the owner and admits actual operations.
use paintcraft_core::types::Rect;
use paintcraft_ui::t;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionState {
    Inactive,
    Empty,
    Active,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    pub enabled: bool,
    pub domain: Rect,
    pub bands: usize,
    pub runs: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    UseSelection,
    SetEnabled(bool),
    Invert,
    Remove,
}
#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub available: bool,
    pub selection: SelectionState,
    pub mask: Option<Summary>,
}
impl Context {
    /// Active-empty deliberately captures a real zero field; bounds are not state.
    pub fn enabled(self, action: Action) -> bool {
        self.available
            && match action {
                Action::UseSelection => self.selection != SelectionState::Inactive,
                Action::SetEnabled(_) | Action::Invert | Action::Remove => self.mask.is_some(),
            }
    }
}
pub fn panel(ui: &mut egui::Ui, context: Context) -> Vec<Action> {
    let mut actions = Vec::new();
    ui.strong(t!(ui.ctx(), "Layer mask"));
    let help = t!(
        ui.ctx(),
        "Pixels outside the saved mask stay hidden. Changing the selection does not change this mask."
    );
    if ui
        .add_enabled(
            context.enabled(Action::UseSelection),
            egui::Button::new(t!(ui.ctx(), "Use selection as layer mask")).wrap(),
        )
        .on_hover_text(&help)
        .clicked()
    {
        actions.push(Action::UseSelection);
    }
    if let Some(mask) = context.mask {
        let mut enabled = mask.enabled;
        if ui
            .add_enabled(
                context.enabled(Action::SetEnabled(!enabled)),
                egui::Checkbox::new(&mut enabled, t!(ui.ctx(), "Enable layer mask")),
            )
            .changed()
        {
            actions.push(Action::SetEnabled(enabled));
        }
        ui.vertical(|ui| {
            if ui
                .add_enabled(
                    context.enabled(Action::Invert),
                    egui::Button::new(t!(ui.ctx(), "Invert layer mask")).wrap(),
                )
                .clicked()
            {
                actions.push(Action::Invert);
            }
            if ui
                .add_enabled(
                    context.enabled(Action::Remove),
                    egui::Button::new(t!(ui.ctx(), "Remove layer mask")).wrap(),
                )
                .clicked()
            {
                actions.push(Action::Remove);
            }
        });
        ui.small(format!(
            "{}: {},{} {}×{}",
            t!(ui.ctx(), "Saved layer mask domain"),
            mask.domain.x0,
            mask.domain.y0,
            mask.domain.width(),
            mask.domain.height()
        ))
        .on_hover_text(help);
        ui.small(format!(
            "{}: {}; {}: {}",
            t!(ui.ctx(), "Saved layer mask bands"),
            mask.bands,
            t!(ui.ctx(), "Saved layer mask runs"),
            mask.runs
        ));
    } else {
        ui.weak(t!(ui.ctx(), "No saved layer mask"));
    }
    actions
}
#[cfg(test)]
mod tests;
