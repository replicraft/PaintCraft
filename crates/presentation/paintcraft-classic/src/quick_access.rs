//! Skin-owned quick actions. All working buttons emit shared host intentions.
use super::{Action, ClassicVariant};
use paintcraft_ui::i18n::tr;
use paintcraft_ui::{actions::Context, icons::Icon};

pub(super) fn show(
    ui: &mut egui::Ui,
    _variant: ClassicVariant,
    context: Context,
    actions: &mut Vec<Action>,
) {
    button(ui, context, actions, Icon::Save, Action::Save, true);
    ui.separator();
    ui.add_enabled_ui(false, |ui| {
        paintcraft_ui::widgets::classic_menu_button(ui, "Share", Icon::Share, false, "Share")
    })
    .inner
    .on_disabled_hover_text(tr(ui.ctx(), "System sharing is not available yet."));
    ui.separator();
    button(ui, context, actions, Icon::Undo, Action::Undo, true);
    button(ui, context, actions, Icon::Redo, Action::Redo, true);
    // Clear Canvas is the requested PaintCraft extension to the reference bar.
    ui.separator();
    button(
        ui,
        context,
        actions,
        Icon::ClearCanvas,
        Action::ClearCanvas,
        true,
    );
}

fn button(
    ui: &mut egui::Ui,
    context: Context,
    actions: &mut Vec<Action>,
    icon: Icon,
    action: Action,
    classic: bool,
) {
    let clicked = if classic {
        paintcraft_ui::actions::classic_toolbar_button(ui, action, icon, context)
    } else {
        paintcraft_ui::actions::toolbar_button(ui, action, icon, context)
    };
    if clicked {
        actions.push(action);
    }
}

/// Keep top-bar metrics local so ribbon and historical skins retain their layout.
pub(super) fn menu_bar(ui: &egui::Ui, variant: ClassicVariant) -> egui::MenuBar {
    // Short windows retain compact targets so the full ribbon and canvas fit.
    let height = if ui.ctx().content_rect().height() < 420.0 {
        32.0
    } else {
        36.0
    };
    egui::MenuBar::new().style(move |style: &mut egui::Style| {
        egui::containers::menu::menu_style(style);
        if variant == ClassicVariant::V11 {
            style.spacing.interact_size.y = height;
            style.spacing.button_padding = egui::vec2(8.0, 0.0);
            for text in [egui::TextStyle::Body, egui::TextStyle::Button] {
                style
                    .text_styles
                    .insert(text, egui::FontId::proportional(16.0));
            }
            style.visuals.override_text_color =
                Some(egui::Color32::from_gray(if style.visuals.dark_mode {
                    245
                } else {
                    24
                }));
        }
    })
}
