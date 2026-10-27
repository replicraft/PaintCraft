//! Browser-only first-visit presentation; dismissal never contains document data.
use super::*;

pub(super) const DISMISSED_KEY: &str = "showWelcomeScreen";
#[derive(Debug, Default)]
pub(super) struct State {
    open: bool,
}
impl State {
    pub(super) fn load() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self::from_lookup(
                storage().and_then(|storage| storage.get_item(DISMISSED_KEY).map_err(|_| ())),
            )
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Native library consumers have no browser startup or localStorage.
            // Host UI tests explicitly construct the same first-visit state.
            Self::default()
        }
    }
    #[cfg(any(target_arch = "wasm32", test))]
    fn from_lookup(value: Result<Option<String>, ()>) -> Self {
        Self {
            open: !matches!(value, Ok(Some(_))),
        }
    }
    pub(super) fn open(&self) -> bool {
        self.open
    }
    fn dismiss_with(&mut self, persist: impl FnOnce(&str, &str) -> Result<(), ()>) {
        // Storage restrictions must not trap a visitor in the dialog this session.
        let _ = persist(DISMISSED_KEY, "false");
        self.open = false;
    }
    fn dismiss(&mut self) {
        self.dismiss_with(|key, value| {
            #[cfg(target_arch = "wasm32")]
            {
                storage()?.set_item(key, value).map_err(|_| ())
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = (key, value);
                Ok(())
            }
        });
    }
}
#[cfg(target_arch = "wasm32")]
fn storage() -> Result<web_sys::Storage, ()> {
    web_sys::window()
        .ok_or(())?
        .local_storage()
        .map_err(|_| ())?
        .ok_or(())
}
impl WebApp {
    pub(super) fn show_welcome(&mut self, ctx: &egui::Context) {
        if !self.welcome.open() {
            return;
        }
        let close_label = t!(ctx, "Close welcome dialog");
        let response = egui::Modal::new(egui::Id::unique("web-welcome")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 48.0).clamp(120.0, 480.0));
            let top_close = ui
                .horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        let close = ui
                            .add(
                                egui::Button::new("×")
                                    .frame(false)
                                    .min_size(egui::vec2(32.0, 32.0)),
                            )
                            .on_hover_text(&close_label);
                        close.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Button, true, &close_label)
                        });
                        ui.add_sized(
                            egui::vec2(ui.available_width(), 0.0),
                            egui::Label::new(
                                egui::RichText::new(t!(ctx, "Welcome to PaintCraft"))
                                    .heading()
                                    .strong(),
                            )
                            .wrap(),
                        );
                        close.clicked()
                    })
                    .inner
                })
                .inner;
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 170.0).max(48.0))
                .show(ui, |ui| {
                    for feature in [
                        "Draw and paint with brushes, shapes, and editable text.",
                        "Try Classic skins or the Studio interface, with layers and undo.",
                        "Open pictures and save your editable work as a PaintCraft project.",
                    ] {
                        ui.horizontal_top(|ui| {
                            ui.label("•");
                            ui.add(egui::Label::new(t!(ctx, feature)).wrap());
                        });
                        ui.add_space(6.0);
                    }
                    ui.add_space(8.0);
                    ui.label(t!(
                        ctx,
                        "Click Interface in the top right to try the different interfaces"
                    ));
                });
            ui.add_space(12.0);
            let bottom_close = ui
                .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.button(t!(ctx, "Close")).clicked()
                })
                .inner;
            top_close || bottom_close
        });
        if ctx.current_pass_index() == 0 {
            let escape =
                ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if response.inner || escape {
                self.welcome.dismiss();
            }
        }
    }
}
#[cfg(test)]
mod tests;
