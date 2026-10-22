//! Skin-owned quick actions. All working buttons emit shared host intentions.
use super::{Action, ClassicVariant};
use paintcraft_ui::{actions::Context, icons::Icon};

pub(super) fn show(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    context: Context,
    actions: &mut Vec<Action>,
) {
    if variant != ClassicVariant::V11 {
        for (icon, action) in [
            (Icon::New, Action::New),
            (Icon::Open, Action::Open),
            (Icon::Save, Action::Save),
            (Icon::Export, Action::Export),
            (Icon::Undo, Action::Undo),
            (Icon::Redo, Action::Redo),
            (Icon::ClearCanvas, Action::ClearCanvas),
        ] {
            button(ui, context, actions, icon, action);
        }
        return;
    }
    button(ui, context, actions, Icon::Save, Action::Save);
    ui.separator();
    ui.add_enabled_ui(false, |ui| {
        paintcraft_ui::widgets::icon_button(ui, "Share", Icon::Share, false, false)
    })
    .inner
    .on_disabled_hover_text("System sharing is not available yet.");
    ui.separator();
    button(ui, context, actions, Icon::Undo, Action::Undo);
    button(ui, context, actions, Icon::Redo, Action::Redo);
    // Clear Canvas is the requested PaintCraft extension to the reference bar.
    ui.separator();
    button(ui, context, actions, Icon::ClearCanvas, Action::ClearCanvas);
}

fn button(
    ui: &mut egui::Ui,
    context: Context,
    actions: &mut Vec<Action>,
    icon: Icon,
    action: Action,
) {
    if paintcraft_ui::actions::toolbar_button(ui, action, icon, context) {
        actions.push(action);
    }
}
