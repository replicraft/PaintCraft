//! Native menus route into the same action outbox as the frontend UI.
use super::*;
use paintcraft_platform::native_menu::{Entry, NativeMenu, Section};
use paintcraft_ui::actions::{Action as Chrome, Frontend, ViewAction};

const QUIT: u64 = 9000;

fn actions() -> Vec<Chrome> {
    paintcraft_ui::actions::FILE
        .iter()
        .chain(paintcraft_ui::actions::EDIT)
        .copied()
        .chain(
            paintcraft_ui::actions::VIEW
                .iter()
                .chain(paintcraft_ui::actions::WORKSPACE_VIEW)
                .copied()
                .map(Chrome::View),
        )
        .chain([Chrome::Settings])
        .collect()
}

fn shortcut(action: Chrome, frontend: Frontend) -> Option<&'static str> {
    Some(match action {
        Chrome::New => "meta+KeyN",
        Chrome::Open => "meta+KeyO",
        Chrome::Save => "meta+KeyS",
        Chrome::SaveAs => "meta+shift+KeyS",
        Chrome::Close => "meta+KeyW",
        Chrome::Undo => "meta+KeyZ",
        Chrome::Redo => "meta+shift+KeyZ",
        Chrome::SelectAll => "meta+KeyA",
        Chrome::Deselect => "meta+KeyD",
        Chrome::View(ViewAction::ZoomIn) => "meta+Equal",
        Chrome::View(ViewAction::ZoomOut) => "meta+Minus",
        Chrome::View(ViewAction::ActualSize) => "meta+Digit0",
        Chrome::View(ViewAction::FitWindow) => "meta+KeyB",
        Chrome::View(ViewAction::FitSelection) => "meta+shift+KeyB",
        Chrome::View(ViewAction::FullScreen) => "F11",
        Chrome::View(ViewAction::Rulers) if frontend == Frontend::Classic => "meta+KeyR",
        Chrome::View(ViewAction::PixelGrid) if frontend == Frontend::Classic => "meta+KeyG",
        _ => return None,
    })
}

impl App {
    pub(super) fn initialize_native_menu(&mut self, ctx: &egui::Context) {
        let context = self.action_context();
        let section = |title: &str, actions: Vec<Chrome>| Section {
            title: title.into(),
            entries: actions
                .into_iter()
                .map(|action| Entry {
                    id: action.activation_code(),
                    text: action.descriptor_for(context.frontend).label.into(),
                    shortcut: shortcut(action, context.frontend),
                    checked: action.checked(context),
                })
                .collect(),
        };
        let wake = ctx.clone();
        match NativeMenu::new(
            vec![
                section("File", paintcraft_ui::actions::FILE.to_vec()),
                section("Edit", paintcraft_ui::actions::EDIT.to_vec()),
                section(
                    "View",
                    paintcraft_ui::actions::VIEW
                        .iter()
                        .chain(paintcraft_ui::actions::WORKSPACE_VIEW)
                        .copied()
                        .map(Chrome::View)
                        .collect(),
                ),
            ],
            Chrome::Settings.activation_code(),
            QUIT,
            move || wake.request_repaint(),
        ) {
            Ok(menu) => self.native_menu = Some(menu),
            Err(e) => {
                eprintln!("Native menu unavailable: {e}");
                self.editor
                    .set_status(format!("Native menu unavailable: {e}"));
            }
        }
    }

    pub(super) fn poll_native_menu(&mut self, ctx: &egui::Context) {
        let context = self.action_context();
        let Some(menu) = &mut self.native_menu else {
            return;
        };
        let text_focus = ctx.egui_wants_keyboard_input();
        for action in actions() {
            if let Err(error) = menu.update_presentation(
                action.activation_code(),
                action.descriptor_for(context.frontend).label,
                shortcut(action, context.frontend),
            ) {
                self.editor
                    .set_status(format!("Native menu update failed: {error}"));
            }
            menu.update(
                action.activation_code(),
                action.enabled(context)
                    || (text_focus
                        && matches!(action, Chrome::Undo | Chrome::Redo | Chrome::SelectAll)),
                action.checked(context),
            );
        }
        menu.update(QUIT, !context.blocked, None);
        let events = menu.poll();
        for event in events {
            if event == QUIT {
                if !self.action_context().blocked {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            } else if let Some(action) = actions()
                .into_iter()
                .find(|action| action.activation_code() == event)
            {
                if text_focus && matches!(action, Chrome::Undo | Chrome::Redo | Chrome::SelectAll) {
                    if let Some(shortcut) = action.descriptor().shortcut {
                        ctx.input_mut(|input| {
                            input.events.push(egui::Event::Key {
                                key: shortcut.logical_key,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: shortcut.modifiers,
                            })
                        });
                    }
                } else {
                    self.dispatch_chrome(ctx, vec![action]);
                }
            }
        }
    }
}
