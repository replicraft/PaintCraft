//! Windows 11 File/Edit presentation; unavailable reference operations stay disabled.
use super::ClassicVariant;
use egui::{Button, Key, KeyboardShortcut, Modifiers, Ui};
use paintcraft_ui::{
    actions::{Action, Context},
    icons::Icon,
};

pub(super) fn file(ui: &mut Ui, context: Context, actions: &mut Vec<Action>) {
    layout(ui);
    // The full reference menu is taller than a small window. Keep all rows,
    // including Exit, reachable without drawing outside the viewport.
    let height = (ui.ctx().content_rect().bottom() - ui.cursor().min.y - 16.0).clamp(80.0, 640.0);
    ui.set_max_height(height);
    egui::ScrollArea::vertical()
        .id_salt("classic-file-menu")
        .max_height(height)
        .show(ui, |ui| {
            action(ui, context, actions, Action::New, "New");
            action(ui, context, actions, Action::Open, "Open");
            ui.menu_image_text_button(
                paintcraft_ui::icons::image(ui.ctx(), Icon::Open, 16.0),
                "Import to canvas",
                |ui| {
                    layout(ui);
                    unavailable(
                        ui,
                        "From file…",
                        Icon::Open,
                        None,
                        "Importing into the current canvas is not available yet.",
                    );
                    unavailable(
                        ui,
                        "From scanner or camera…",
                        Icon::New,
                        None,
                        "Scanner and camera acquisition is not available yet.",
                    );
                },
            );
            unavailable(
                ui,
                "Recent",
                Icon::Open,
                None,
                "Recent files are not available yet.",
            );
            ui.separator();
            action(ui, context, actions, Action::Save, "Save");
            ui.menu_image_text_button(
                paintcraft_ui::icons::image(ui.ctx(), Icon::SaveAs, 16.0),
                "Save as",
                |ui| {
                    layout(ui);
                    action(ui, context, actions, Action::Export, "Picture…");
                    ui.weak("PNG, BMP or JPEG");
                },
            );
            action(ui, context, actions, Action::SaveAs, "Save as project");
            ui.separator();
            ui.menu_image_text_button(
                paintcraft_ui::icons::image(ui.ctx(), Icon::Print, 16.0),
                "Print",
                |ui| {
                    layout(ui);
                    for label in ["Print…", "Page setup…", "Print preview"] {
                        unavailable(
                            ui,
                            label,
                            Icon::Print,
                            None,
                            "Printing is not available yet.",
                        );
                    }
                },
            );
            unavailable(
                ui,
                "Share",
                Icon::Share,
                None,
                "System sharing is not available yet.",
            );
            ui.separator();
            unavailable(
                ui,
                "Set as desktop background",
                Icon::CanvasSize,
                None,
                "Setting the desktop background is not available yet.",
            );
            ui.separator();
            unavailable(
                ui,
                "Image properties",
                Icon::CanvasSize,
                Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::E)),
                "Image properties are not available yet. Canvas size is available in View.",
            );
            // Close is a PaintCraft document-workspace extension; Exit closes the
            // window through the same host-owned unsaved-work guard as the OS menu.
            action(ui, context, actions, Action::Close, "Close");
            ui.separator();
            action(ui, context, actions, Action::Exit, "Exit");
        });
}

pub(super) fn edit(ui: &mut Ui, context: Context, actions: &mut Vec<Action>) {
    layout(ui);
    for (label, icon, key, shift) in [
        ("Cut", Icon::Cut, Key::X, false),
        ("Copy", Icon::Copy, Key::C, false),
        ("Paste", Icon::Paste, Key::V, false),
        ("Copy visible layers", Icon::CopyMerged, Key::C, true),
    ] {
        if shift {
            ui.separator();
        }
        unavailable(
            ui,
            label,
            icon,
            Some(KeyboardShortcut::new(
                Modifiers {
                    shift,
                    ..Modifiers::COMMAND
                },
                key,
            )),
            "Canvas clipboard operations are not available yet.",
        );
    }
    // Preserve existing working PaintCraft commands below the reference entries.
    ui.separator();
    for &item in paintcraft_ui::actions::EDIT {
        action(ui, context, actions, item, item.descriptor().label);
    }
}

fn layout(ui: &mut Ui) {
    ui.set_min_width(300.0);
    ui.spacing_mut().interact_size.y = 36.0;
    for text in [egui::TextStyle::Body, egui::TextStyle::Button] {
        ui.style_mut()
            .text_styles
            .insert(text, egui::FontId::proportional(15.0));
    }
}

fn action(
    ui: &mut Ui,
    context: Context,
    actions: &mut Vec<Action>,
    item: Action,
    label: &'static str,
) {
    if paintcraft_ui::actions::menu_item_with_label(ui, item, context, label) {
        actions.push(item);
        ui.close();
    }
}

fn unavailable(
    ui: &mut Ui,
    label: &str,
    icon: Icon,
    shortcut: Option<KeyboardShortcut>,
    help: &str,
) {
    let mut button =
        Button::image_and_text(paintcraft_ui::icons::image(ui.ctx(), icon, 16.0), label);
    if let Some(shortcut) = shortcut {
        button = button.shortcut_text(ui.ctx().format_shortcut(&shortcut));
    }
    let response = ui.add_enabled(false, button);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, false, label));
    response.on_disabled_hover_text(help);
}

/// Visibility choices add rows, so retain access to lower view commands on
/// short displays instead of relying on the popup's initial size estimate.
pub(super) fn view(
    ui: &mut Ui,
    variant: ClassicVariant,
    context: Context,
    actions: &mut Vec<Action>,
) {
    if variant == ClassicVariant::V11 {
        layout(ui);
    }
    let height = (ui.ctx().content_rect().bottom() - ui.cursor().min.y - 16.0).clamp(80.0, 640.0);
    ui.set_max_height(height);
    egui::ScrollArea::vertical()
        .id_salt("classic-view-menu")
        .max_height(height)
        .show(ui, |ui| {
            ui.menu_button("Zoom", |ui| {
                for &action in paintcraft_ui::actions::VIEW {
                    if action == paintcraft_ui::actions::ViewAction::PixelGrid {
                        continue;
                    }
                    if paintcraft_ui::actions::menu_item(ui, Action::View(action), context) {
                        actions.push(Action::View(action));
                        ui.close();
                    }
                }
            });
            if paintcraft_ui::actions::menu_item(
                ui,
                Action::View(paintcraft_ui::actions::ViewAction::Rulers),
                context,
            ) {
                actions.push(Action::View(paintcraft_ui::actions::ViewAction::Rulers));
                ui.close();
            }
            if paintcraft_ui::actions::menu_item(
                ui,
                Action::View(paintcraft_ui::actions::ViewAction::PixelGrid),
                context,
            ) {
                actions.push(Action::View(paintcraft_ui::actions::ViewAction::PixelGrid));
                ui.close();
            }
            for &action in paintcraft_ui::actions::WORKSPACE_VIEW {
                if action == paintcraft_ui::actions::ViewAction::Rulers {
                    continue;
                }
                if matches!(
                    action,
                    paintcraft_ui::actions::ViewAction::FullScreen
                        | paintcraft_ui::actions::ViewAction::AlwaysShowToolbar
                ) {
                    ui.separator();
                }
                if paintcraft_ui::actions::menu_item(ui, Action::View(action), context) {
                    actions.push(Action::View(action));
                    ui.close();
                }
            }
            if variant == ClassicVariant::V11 {
                ui.add_enabled(false, egui::Button::new("Thumbnail"))
                    .on_disabled_hover_text("The thumbnail window is not available yet.");
            }
            ui.separator();
            for action in [Action::CanvasSize, Action::CanvasDefaults] {
                if paintcraft_ui::actions::menu_item(ui, action, context) {
                    actions.push(action);
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Change Classic skin…").clicked() {
                actions.push(Action::Interface);
                ui.close();
            }
        });
}
