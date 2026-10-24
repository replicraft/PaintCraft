//! Windows 11 File/Edit presentation; unavailable reference operations stay disabled.
use super::ClassicVariant;
use egui::{Button, Key, KeyboardShortcut, Modifiers, Ui};
use paintcraft_ui::t;
use paintcraft_ui::{
    actions::{Action, Context, MenuIconStyle},
    icons::Icon,
};

pub(super) fn file(ui: &mut Ui, context: Context, actions: &mut Vec<Action>) {
    layout(ui);
    // The full reference menu is taller than a small window. Keep all rows,
    // including Exit, reachable without drawing outside the viewport.
    let height = super::popup_layout::height(ui, 80.0, 640.0);
    egui::ScrollArea::vertical()
        .id_salt("classic-file-menu")
        .max_height(height)
        .show(ui, |ui| {
            action(ui, context, actions, Action::New, "New");
            action(ui, context, actions, Action::Open, "Open");
            submenu(ui, "Import to canvas", Icon::ImportImage, |ui| {
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
            });
            action(ui, context, actions, Action::RecentFiles, "Recent");
            ui.separator();
            action(ui, context, actions, Action::Save, "Save");
            submenu(ui, "Save as", Icon::SaveAs, |ui| {
                layout(ui);
                action(ui, context, actions, Action::Export, "Picture…");
                ui.weak(t!(ui.ctx(), "PNG, BMP or JPEG"));
            });
            action(ui, context, actions, Action::SaveAs, "Save as project");
            ui.separator();
            submenu(ui, "Print", Icon::Print, |ui| {
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
            });
            unavailable(
                ui,
                "Share",
                Icon::Share,
                None,
                "System sharing is not available yet.",
            );
            ui.separator();
            unavailable_submenu(
                ui,
                "Set as desktop background",
                Icon::DesktopBackground,
                "Setting the desktop background is not available yet.",
            );
            ui.separator();
            action(
                ui,
                context,
                actions,
                Action::ImageProperties,
                "Image properties",
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
    let height = super::popup_layout::height(ui, 80.0, 640.0);
    egui::ScrollArea::vertical()
        .id_salt("classic-edit-menu")
        .max_height(height)
        .show(ui, |ui| {
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
        });
}

fn layout(ui: &mut Ui) {
    ui.set_min_width(300.0);
    ui.spacing_mut().interact_size.y = 40.0;
    ui.spacing_mut().item_spacing.y = 0.0;
    let dark = ui.visuals().dark_mode;
    ui.visuals_mut().override_text_color =
        Some(egui::Color32::from_gray(if dark { 245 } else { 24 }));
    ui.visuals_mut().weak_text_color = Some(egui::Color32::from_gray(if dark { 208 } else { 96 }));
    for text in [egui::TextStyle::Body, egui::TextStyle::Button] {
        ui.style_mut()
            .text_styles
            .insert(text, egui::FontId::proportional(16.0));
    }
}

fn action(
    ui: &mut Ui,
    context: Context,
    actions: &mut Vec<Action>,
    item: Action,
    label: &'static str,
) {
    if paintcraft_ui::actions::menu_item_with_icon_style(
        ui,
        item,
        context,
        label,
        22.0,
        MenuIconStyle::ClassicOutline,
    ) {
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
) -> egui::Response {
    let mut button = Button::image_and_text(
        paintcraft_ui::icons::classic_menu_image(
            ui.ctx(),
            icon,
            22.0,
            ui.visuals().strong_text_color(),
        ),
        label,
    );
    if let Some(shortcut) = shortcut {
        button = button.shortcut_text(ui.ctx().format_shortcut(&shortcut));
    }
    let response = ui.add_enabled(false, button);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, false, label));
    response.on_disabled_hover_text(t!(ui.ctx(), help))
}

fn unavailable_submenu(ui: &mut Ui, label: &str, icon: Icon, help: &str) {
    let response = unavailable(ui, label, icon, None, help);
    submenu_arrow(
        ui,
        &response,
        ui.visuals().disable(ui.visuals().text_color()),
    );
}

fn submenu_arrow(ui: &Ui, response: &egui::Response, ink: egui::Color32) {
    let center = response.rect.right_center() - egui::vec2(8.0, 0.0);
    ui.painter().add(egui::epaint::PathShape::line(
        vec![
            center + egui::vec2(-2.0, -4.0),
            center + egui::vec2(2.0, 0.0),
            center + egui::vec2(-2.0, 4.0),
        ],
        egui::Stroke::new(1.25, ink),
    ));
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
    let height = super::popup_layout::height(ui, 80.0, 640.0);
    egui::ScrollArea::vertical()
        .id_salt("classic-view-menu")
        .max_height(height)
        .show(ui, |ui| {
            if variant == ClassicVariant::V11 {
                submenu(ui, "Zoom", Icon::ZoomIn, |ui| {
                    zoom_menu(ui, variant, context, actions)
                });
            } else {
                ui.menu_button(t!(ui.ctx(), "Zoom"), |ui| {
                    zoom_menu(ui, variant, context, actions)
                });
            }
            if view_item(
                ui,
                Action::View(paintcraft_ui::actions::ViewAction::Rulers),
                context,
                variant,
            ) {
                actions.push(Action::View(paintcraft_ui::actions::ViewAction::Rulers));
                ui.close();
            }
            if view_item(
                ui,
                Action::View(paintcraft_ui::actions::ViewAction::PixelGrid),
                context,
                variant,
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
                if view_item(ui, Action::View(action), context, variant) {
                    actions.push(Action::View(action));
                    ui.close();
                }
            }
            ui.separator();
            for action in [Action::CanvasSize, Action::CanvasDefaults] {
                if view_item(ui, action, context, variant) {
                    actions.push(action);
                    ui.close();
                }
            }
            ui.separator();
            let change_skin = if variant == ClassicVariant::V11 {
                paintcraft_ui::actions::menu_item_with_icon_style(
                    ui,
                    Action::Interface,
                    context,
                    "Change Classic skin…",
                    22.0,
                    MenuIconStyle::ClassicOutline,
                )
            } else {
                ui.button(t!(ui.ctx(), "Change Classic skin…")).clicked()
            };
            if change_skin {
                actions.push(Action::Interface);
                ui.close();
            }
        });
}

fn view_item(ui: &mut Ui, action: Action, context: Context, variant: ClassicVariant) -> bool {
    paintcraft_ui::actions::menu_item_with_icon_style(
        ui,
        action,
        context,
        action.descriptor_for(context.frontend).label,
        if variant == ClassicVariant::V11 {
            22.0
        } else {
            16.0
        },
        if variant == ClassicVariant::V11 {
            if matches!(
                action,
                Action::View(
                    paintcraft_ui::actions::ViewAction::FullScreen
                        | paintcraft_ui::actions::ViewAction::Thumbnail
                )
            ) {
                MenuIconStyle::ClassicOutlineToggle
            } else {
                MenuIconStyle::ClassicOutline
            }
        } else {
            MenuIconStyle::Colorful
        },
    )
}

fn zoom_menu(ui: &mut Ui, variant: ClassicVariant, context: Context, actions: &mut Vec<Action>) {
    if variant == ClassicVariant::V11 {
        layout(ui);
    }
    for &action in paintcraft_ui::actions::VIEW {
        if action == paintcraft_ui::actions::ViewAction::PixelGrid {
            continue;
        }
        if view_item(ui, Action::View(action), context, variant) {
            actions.push(Action::View(action));
            ui.close();
        }
    }
}

/// egui's submenu hover heuristic uses the full layout rect, including scrolled
/// off-screen rows. Restrict its anchor to the visible portion of the entry.
fn submenu(ui: &mut Ui, label: &str, icon: Icon, contents: impl FnOnce(&mut Ui)) {
    let localized_label = t!(ui.ctx(), label);
    let label = localized_label.as_str();
    let id = ui.next_auto_id();
    let open = egui::containers::menu::MenuState::from_ui(ui, |state, _| {
        state.open_item == Some(egui::containers::menu::SubMenu::id_from_widget_id(id))
    });
    let inactive = ui.visuals().widgets.inactive;
    if open {
        ui.style_mut().visuals.widgets.inactive = ui.visuals().widgets.open;
    }
    let response = ui.add(
        Button::image_and_text(
            paintcraft_ui::icons::classic_menu_image(
                ui.ctx(),
                icon,
                22.0,
                ui.visuals().strong_text_color(),
            ),
            label,
        )
        .right_text("    "),
    );
    ui.style_mut().visuals.widgets.inactive = inactive;
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    submenu_arrow(ui, &response, ui.visuals().text_color());
    let dismissed_id = response.id.with("submenu-keyboard-dismissed");
    let child_id = egui::containers::menu::SubMenu::id_from_widget_id(response.id);
    let child_open = egui::containers::menu::MenuState::from_ui(ui, |state, _| {
        state.open_item == Some(child_id)
    });
    if child_open
        && ui.input_mut(|input| {
            input.consume_key(Modifiers::NONE, Key::Escape)
                || input.consume_key(Modifiers::NONE, Key::ArrowLeft)
        })
    {
        egui::containers::menu::MenuState::from_ui(ui, |state, _| state.open_item = None);
        ui.data_mut(|data| data.insert_temp(dismissed_id, true));
        response.request_focus();
        ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        return;
    }
    if ui
        .data(|data| data.get_temp::<bool>(dismissed_id))
        .unwrap_or(false)
    {
        let focused = response.has_focus();
        let resume = response.clicked()
            || ui.input(|input| {
                input.pointer.delta() != egui::Vec2::ZERO
                    || (focused && input.key_pressed(Key::ArrowRight))
            });
        if !resume {
            return;
        }
        ui.data_mut(|data| data.remove::<bool>(dismissed_id));
    }
    let mut anchor = response;
    anchor.rect = anchor.rect.intersect(ui.clip_rect());
    if anchor.rect.is_positive() {
        egui::containers::menu::SubMenu::new().show(ui, &anchor, contents);
    }
}
