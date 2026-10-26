//! Browser frontend layout; editor actions remain in the application host.
use super::*;

impl WebApp {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.refresh_appearance(&ctx);
        crate::local_fonts::refresh(&ctx, &self.text.style.family, &mut self.text.families);
        let font_generation = crate::local_fonts::generation();
        if self.text.font_generation != font_generation {
            self.text.font_generation = font_generation;
            self.canvas.invalidate();
        }
        if ctx.current_pass_index() == 0 {
            if self.fullscreen.refresh(&ctx) {
                self.message = crate::fullscreen::UNAVAILABLE.into();
            }
            self.poll_files(&ctx);
            self.poll_copy(&ctx);
            self.poll_paste(&ctx);
            self.poll_print();
        }
        self.text.widget_owned_input =
            ctx.egui_wants_keyboard_input() || egui::Popup::is_any_open(&ctx);
        self.studio.sampled_brush = self.editor.adopted_brush.as_ref().map(|adopted| {
            (
                self.editor.tools.sampled_name.clone().unwrap_or_default(),
                (adopted.tip().width(), adopted.tip().height()),
            )
        });
        let context = self.context();
        let chrome = egui::Panel::top("classic-chrome").show(ui, |ui| {
            let actions = if self.frontend == Frontend::Studio {
                let mut actions = Vec::new();
                ui.horizontal(|ui| {
                    ui.strong(t!(ui.ctx(), "PaintCraft Studio"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if paintcraft_ui::widgets::classic_menu_button(
                            ui,
                            "Settings…",
                            paintcraft_ui::icons::Icon::Settings,
                            false,
                            "Settings…",
                        )
                        .clicked()
                        {
                            actions.push(Action::Settings);
                        }
                        actions.extend(paintcraft_ui::chrome::interface_changer(ui, context));
                    });
                });
                actions.extend(paintcraft_studio::toolbar(
                    ui,
                    &mut self.controls,
                    &mut self.studio,
                    context,
                ));
                self.studio.canvas_top = ui.min_rect().bottom();
                actions
            } else {
                paintcraft_classic::chrome_with_text(
                    ui,
                    self.variant,
                    &mut self.controls,
                    &mut self.workspace,
                    context,
                    Some(paintcraft_classic::TextControls {
                        options: &mut self.text.style,
                        families: &self.text.families,
                        search: &mut self.text.search,
                        loading: false,
                    }),
                )
            };
            for action in actions {
                self.action(&ctx, action);
            }
        });
        self.paste_placement_bar(ui);
        let status_rect =
            (self.frontend == Frontend::Studio || self.preferences.status_bar).then(|| {
                egui::Panel::bottom("classic-status")
                    .frame(paintcraft_classic::status::frame(self.variant, ctx.theme()))
                    .show(ui, |ui| {
                        let size = egui::vec2(
                            ui.available_width(),
                            paintcraft_classic::status::height_for_skin(ui, self.variant, true),
                        );
                        ui.allocate_space(size).1
                    })
                    .inner
            });
        self.text_chrome(ui);
        if self.frontend == Frontend::Classic {
            let mut bounds = chrome.response.rect;
            bounds.max.y = bounds.max.y.max(ui.available_rect_before_wrap().top());
            self.workspace.toolbar.remember_bounds(bounds);
        }
        if self.frontend == Frontend::Studio {
            self.studio_windows(&ctx);
        }
        self.layers(ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(paintcraft_classic::workspace_color(
                self.variant,
                ctx.theme(),
            )))
            .show(ui, |ui| {
                self.canvas(ui);
            });
        if let Some(status_rect) = status_rect {
            ui.scope_builder(egui::UiBuilder::new().max_rect(status_rect), |ui| {
                let selection = self
                    .editor
                    .tools
                    .selection_preview_size(self.editor.session.document().canvas_rect())
                    .or_else(|| self.selected_dimensions());
                let actions = paintcraft_classic::status::contents_for_skin(
                    ui,
                    self.variant,
                    paintcraft_classic::status::Status {
                        message: &tr_status(ui.ctx(), &self.message),
                        recovery: None,
                        canvas: Some(self.editor.session.document().canvas),
                        pointer: self.canvas.pointer,
                        selection,
                    },
                    self.context(),
                    egui::Id::unique("web-zoom"),
                );
                for action in actions {
                    self.action(&ctx, action);
                }
            });
        }
        self.show_thumbnail(&ctx);
        self.dialogs(&ctx);
        self.show_form(&ctx);
        self.print_progress(&ctx);
        self.paste_dialog(&ctx);
    }
}
