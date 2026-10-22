//! Desktop chrome integration.

use super::*;

impl App {
    pub(super) fn action_context(&self) -> paintcraft_ui::actions::Context {
        use paintcraft_ui::actions::{Context, Frontend};
        let pending_text = self.editor.is_editing_text();
        let layer_index = self
            .editor
            .target()
            .and_then(|id| self.editor.document().roots.iter().position(|r| *r == id));
        let layer_visible = self
            .editor
            .target()
            .and_then(|id| self.editor.document().layer(&id))
            .is_some_and(|l| l.visible);
        Context {
            frontend: match self.experience() {
                Experience::Classic => Frontend::Classic,
                Experience::Studio => Frontend::Studio,
            },
            document_open: self.document_count() > 0,
            can_undo: self.editor.undo_depth() > 0
                || pending_text
                || self.editor.has_pending_edits(),
            can_redo: (self.editor.redo_depth() > 0 || self.editor.has_pending_edits())
                && !pending_text,
            selection_active: self.editor.selection() != Selection::Inactive,
            tool: self.editor.tool,
            layer_index,
            layer_count: self.editor.document().roots.len(),
            layer_visible,
            zoom: self.zoom,
            pixel_grid: self.pixel_grid,
            can_fit_selection: matches!(self.editor.selection(), Selection::Rect(_)),
            status_bar: self.settings.classic_status_bar,
            rulers: self.settings.classic_rulers,
            fullscreen: self.fullscreen,
            blocked: self.switching.is_some()
                || self.closing_workspace
                || self.close_after_saves
                || self.close_after_edits
                || !self.deferred_files.is_empty()
                || self.confirm.is_some()
                || self.tab_closing.is_some()
                || self.dialogs.as_ref().is_some_and(NativeDialogs::pending),
        }
    }

    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if self.experience() == Experience::Studio {
            ui.horizontal(|ui| {
                ui.strong("PaintCraft Studio");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if paintcraft_ui::widgets::icon_button(
                        ui,
                        "Settings…",
                        paintcraft_ui::icons::Icon::Settings,
                        false,
                        false,
                    )
                    .clicked()
                    {
                        self.dispatch_chrome(&ctx, vec![paintcraft_ui::actions::Action::Settings]);
                    }
                    let actions =
                        paintcraft_ui::chrome::interface_changer(ui, self.action_context());
                    self.dispatch_chrome(&ctx, actions);
                });
            });
        }
        let mut controls = paintcraft_ui::chrome::Controls {
            tool: self.editor.tool,
            options: self.editor.options,
            primary: self.editor.color,
            secondary: self.editor.secondary,
        };
        let action_context = self.action_context();
        self.classic.custom_colors = self.settings.custom_colors;
        let actions = ui
            .push_id(("document-chrome", self.active_tab_id()), |ui| {
                match self.experience() {
                    Experience::Classic => paintcraft_classic::chrome(
                        ui,
                        self.variant(),
                        &mut controls,
                        &mut self.classic,
                        action_context,
                    ),
                    Experience::Studio => paintcraft_studio::toolbar(
                        ui,
                        &mut controls,
                        &mut self.studio,
                        action_context,
                    ),
                }
            })
            .inner;
        self.apply_chrome(&ctx, controls, actions);
        if self.editor.tool == Tool::Text {
            self.classic_text_options(ui);
            self.editor.set_text_style(self.settings.text.clone());
        }
    }

    fn apply_chrome(
        &mut self,
        ctx: &egui::Context,
        controls: paintcraft_ui::chrome::Controls,
        actions: Vec<paintcraft_ui::chrome::Action>,
    ) {
        // Controls belong to the document that produced this frame. An action
        // below may activate another document with its own tool preferences.
        self.editor.options = controls.options;
        self.editor.color = controls.primary;
        self.editor.secondary = controls.secondary;
        self.dispatch_chrome(ctx, actions);
    }

    pub(super) fn dispatch_chrome(
        &mut self,
        ctx: &egui::Context,
        actions: Vec<paintcraft_ui::chrome::Action>,
    ) {
        for action in actions {
            self.chrome_outbox
                .push(token(ctx, action.activation_code()), action);
        }
        for action in self.chrome_outbox.drain() {
            // Earlier events in a coalesced frame may have queued an edit or
            // history move. Its result determines this action's enabled state.
            self.editor.run_pending();
            if !action.enabled(self.action_context()) {
                continue;
            }
            use paintcraft_ui::chrome::Action as Chrome;
            match action {
                Chrome::New => {
                    if self.experience() == Experience::Classic
                        && (self.editor.is_dirty() || self.has_document_drafts())
                    {
                        self.confirm = Some(Confirm::DiscardForNew);
                    } else {
                        self.new_document();
                    }
                }
                Chrome::Open => self.open_requested(ctx),
                Chrome::Save => self.save_requested(ctx),
                Chrome::SaveAs => self.request_dialog(ctx, Kind::SaveAs),
                Chrome::Export => self.export_requested(ctx),
                Chrome::Undo => self.request(ctx, code::UNDO, Action::Undo),
                Chrome::Redo => self.request(ctx, code::REDO, Action::Redo),
                Chrome::View(action) => self.dispatch_view_action(ctx, action),
                Chrome::Layer(action) => self.dispatch_layer_action(ctx, action),
                Chrome::Close => self.request_close_tab(self.active_tab_id(), ctx),
                Chrome::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Chrome::Settings => self.open_settings(ctx),
                Chrome::ChooseFrontend(frontend) => self.choose_experience(match frontend {
                    paintcraft_ui::actions::Frontend::Classic => Experience::Classic,
                    paintcraft_ui::actions::Frontend::Studio => Experience::Studio,
                }),
                Chrome::Interface => self.picking = Some(self.variant()),
                Chrome::SelectTool(tool) => self.editor.select_tool(tool),
                Chrome::SelectAll => {
                    let (w, h) = self.editor.document().canvas;
                    self.request(
                        ctx,
                        code::SELECT_ALL,
                        Action::Select {
                            corners: Some(((0, 0), (i64::from(w) - 1, i64::from(h) - 1))),
                        },
                    );
                }
                Chrome::EditColors | Chrome::EditSecondaryColors => {
                    self.begin_color_dialog(ctx, action == Chrome::EditSecondaryColors)
                }
                Chrome::ClearCanvas => {
                    let color = match self.experience() {
                        Experience::Classic => self.editor.canvas_color(),
                        Experience::Studio => Rgba::TRANSPARENT,
                    };
                    self.request(ctx, code::CLEAR_CANVAS, Action::ClearCanvas { color });
                }
                Chrome::CanvasSize | Chrome::CanvasDefaults => {
                    self.begin_canvas_dialog(ctx, action == Chrome::CanvasDefaults)
                }
                Chrome::Deselect => {
                    self.request(ctx, code::DESELECT, Action::Select { corners: None })
                }
            }
        }
    }

    pub(super) fn studio_windows(&mut self, ctx: &egui::Context) {
        self.studio.canvas_top = self.canvas_viewport.map_or(150.0, |rect| rect.top());
        let mut controls = paintcraft_ui::chrome::Controls {
            tool: self.editor.tool,
            options: self.editor.options,
            primary: self.editor.color,
            secondary: self.editor.secondary,
        };
        let enabled = self.document_count() > 0 && !self.closing_workspace;
        let state_id = egui::Id::unique(("document-controls", self.active_tab_id()));
        let actions = paintcraft_studio::floating_controls(
            ctx,
            &mut controls,
            &mut self.studio,
            enabled,
            state_id,
        );
        self.apply_chrome(ctx, controls, actions);
        let mut open = self.studio.layers;
        paintcraft_studio::layers_window(ctx, &mut open, self.studio.canvas_top, |ui| {
            if self.closing_workspace {
                ui.disable();
            }
            self.layer_panel(ui)
        });
        self.studio.layers = open;
        self.studio_history(ctx);
    }

    pub(super) fn layer_panel(&mut self, ui: &mut egui::Ui) {
        ui.push_id(("document-layers", self.active_tab_id()), |ui| {
            self.layer_controls(ui)
        });
    }

    fn layer_controls(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if self.document_count() == 0 {
            ui.label("No document open");
            return;
        }
        ui.horizontal(|ui| {
            ui.heading("Layers");
            if ui.button("Add layer").clicked() {
                self.request(&ctx, code::ADD_LAYER, Action::AddLayer);
            }
        });
        let target = self.editor.target();
        for row in self.editor.layers() {
            ui.horizontal(|ui| {
                let mut visible = row.visible;
                let toggle = ui.checkbox(&mut visible, "").on_hover_text("Visible");
                toggle.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::Role::CheckBox,
                        true,
                        visible,
                        format!("Show {}", row.name),
                    )
                });
                if toggle.changed() {
                    self.editor.request(
                        token(&ctx, code::VISIBLE ^ row.id.as_u128() as u64),
                        Action::SetVisible {
                            layer: row.id,
                            visible,
                        },
                    );
                }
                if ui
                    .selectable_label(target == Some(row.id), &row.name)
                    .clicked()
                {
                    self.editor.set_target(row.id);
                }
            });
        }
        ui.separator();
        let Some(t) = target else {
            ui.label("Drawing on: (no layer)");
            return;
        };
        let Some((name, opacity, index)) = self.editor.layer_properties(t) else {
            return;
        };
        ui.label(format!("Drawing on: {name}"));
        let top = self.editor.document().roots.len() - 1;

        // Name: edited locally, applied when the field loses focus.
        let buf = match &mut self.name_edit {
            Some(b) if b.0 == t => b,
            other => other.insert((t, name.clone())),
        };
        ui.horizontal(|ui| {
            let label = ui.label("Layer name");
            let r = ui
                .add(egui::TextEdit::singleline(&mut buf.1))
                .labelled_by(label.id);
            if r.lost_focus() {
                if buf.1 != name {
                    let rename = Action::Rename {
                        layer: t,
                        name: buf.1.clone(),
                    };
                    self.editor.request(token(&ctx, code::RENAME), rename);
                }
            } else if !r.has_focus() {
                buf.1.clone_from(&name);
            }
        });

        // Opacity: a drag is one transaction, applied on release.
        let obuf = match &mut self.opacity_edit {
            Some(b) if b.0 == t => b,
            other => other.insert((t, opacity)),
        };
        ui.horizontal(|ui| {
            let label = ui.label("Opacity");
            let r = ui
                .add(egui::DragValue::new(&mut obuf.1).range(0..=255))
                .labelled_by(label.id);
            if r.drag_stopped() || r.lost_focus() {
                if obuf.1 != opacity {
                    let set = Action::SetOpacity {
                        layer: t,
                        opacity: obuf.1,
                    };
                    self.editor.request(token(&ctx, code::OPACITY), set);
                }
            } else if !r.dragged() && !r.has_focus() {
                obuf.1 = opacity;
            }
        });

        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(index < top, egui::Button::new("Move up"))
                .clicked()
            {
                self.request(
                    &ctx,
                    code::MOVE,
                    Action::MoveLayer {
                        layer: t,
                        to_index: index + 1,
                    },
                );
            }
            if ui
                .add_enabled(index > 0, egui::Button::new("Move down"))
                .clicked()
            {
                self.request(
                    &ctx,
                    code::MOVE,
                    Action::MoveLayer {
                        layer: t,
                        to_index: index - 1,
                    },
                );
            }
            if self.experience() == Experience::Studio
                && ui
                    .add_enabled(
                        paintcraft_ui::actions::Action::Layer(
                            paintcraft_ui::actions::LayerAction::Duplicate,
                        )
                        .enabled(self.action_context()),
                        egui::Button::new("Duplicate layer"),
                    )
                    .clicked()
            {
                self.dispatch_chrome(
                    &ctx,
                    vec![paintcraft_ui::actions::Action::Layer(
                        paintcraft_ui::actions::LayerAction::Duplicate,
                    )],
                );
            }
            if self.experience() == Experience::Studio && ui.button("Layer properties…").clicked()
            {
                self.dispatch_chrome(
                    &ctx,
                    vec![paintcraft_ui::actions::Action::Layer(
                        paintcraft_ui::actions::LayerAction::Properties,
                    )],
                );
            }
            if ui.button("Delete layer").clicked() {
                self.request(&ctx, code::DELETE, Action::DeleteLayer { layer: t });
            }
        });
    }

    pub(super) fn confirm_dialog(&mut self, ui: &mut egui::Ui) {
        if let Some(Confirm::SwitchInterface(target)) = self.confirm.clone() {
            self.interface_switch_dialog(ui.ctx(), target);
            return;
        }
        let Some(confirm) = self.confirm.clone() else {
            return;
        };
        let ctx = ui.ctx().clone();
        if let Confirm::Settings(draft) = confirm {
            self.settings_dialog(&ctx, draft);
            return;
        }
        if let Confirm::Colors(draft) = confirm {
            self.classic_color_dialog(&ctx, draft);
            return;
        }
        if let Confirm::Canvas(draft) = confirm {
            self.canvas_settings_dialog(&ctx, draft);
            return;
        }
        if let Confirm::LayerProperties(draft) = confirm {
            self.layer_properties_dialog(&ctx, draft);
            return;
        }
        let (title, text, go) = match &confirm {
            Confirm::Settings(_) | Confirm::SwitchInterface(_) | Confirm::Colors(_) | Confirm::Canvas(_) | Confirm::LayerProperties(_) => unreachable!("properties handled above"),
            Confirm::DiscardForNew
            | Confirm::DiscardForOpen(_)
            | Confirm::DiscardForRecovery(_)
            | Confirm::DiscardForSaveSnapshot(_) => (
                "Discard changes?",
                "The current document has unsaved changes.".to_owned(),
                "Continue",
            ),
            Confirm::ForgetSaveSnapshot(_) => (
                "Forget captured save snapshot?",
                "This discards the snapshot retained for an incomplete save. The destination may have changed, and forgetting does not establish that it was saved safely.".to_owned(),
                "Forget snapshot",
            ),
            Confirm::ReplaceFile(p) => (
                "Replace file?",
                format!(
                    "{} already exists and is not this document's file.",
                    p.display()
                ),
                "Continue",
            ),
            Confirm::ReplaceSharedFile(p) => (
                "Replace a shared file?",
                format!("Another open document uses {}. Saving here can replace its saved contents.",p.display()),
                "Continue",
            ),
            Confirm::QuitUnsaved => {
                let incomplete = self
                    .saving
                    .as_ref()
                    .is_some_and(saving::NativeSaving::failed);
                (if incomplete { "Quit with an incomplete save?" } else { "Quit without saving?" },
                if incomplete {
                    if self.saving.as_ref().is_some_and(saving::NativeSaving::has_volatile_attempts) {
                        "A requested save failed or is unconfirmed. Some captured snapshots could not be stored for restart recovery; quitting abandons those in-memory snapshots. Available crash recovery data will be kept."
                    } else {
                        "A requested save or recovery operation failed or is unconfirmed. Captured snapshots already stored for restart recovery will be kept."
                    }
                }
                else { "There are unsaved changes. Available recovery data will be kept." }.to_owned(), "Quit anyway")
            }
            Confirm::Export {
                path,
                format,
                losses,
                exists,
            } => {
                let mut t = format!("Export {} as {}.", path.display(), format.name());
                if !losses.is_empty() {
                    t += &format!("\nThis export loses: {}.", losses.join(", "));
                }
                if *exists {
                    t += "\nThe file exists and will be replaced.";
                }
                ("Export", t, "Export now")
            }
        };
        egui::Modal::new(egui::Id::unique(("confirmation", title))).show(&ctx, |ui| {
            ui.heading(title);
            ui.label(text);
            ui.horizontal(|ui| {
                if ui.button(go).clicked() {
                    match confirm {
                        Confirm::Settings(_)
                        | Confirm::SwitchInterface(_)
                        | Confirm::Colors(_)
                        | Confirm::Canvas(_)
                        | Confirm::LayerProperties(_) => {
                            unreachable!("properties handled above")
                        }
                        Confirm::DiscardForNew => self.new_document(),
                        Confirm::DiscardForOpen(p) => {
                            if let Err(e) = self.open_path(&p) {
                                self.editor.set_status(format!("Cannot open: {e}"));
                            }
                        }
                        Confirm::DiscardForRecovery(item) => self.do_recover(item),
                        Confirm::DiscardForSaveSnapshot(sequence) => {
                            self.open_saved_snapshot(sequence)
                        }
                        Confirm::ForgetSaveSnapshot(sequence) => {
                            self.forget_incomplete_save(sequence)
                        }
                        Confirm::ReplaceFile(p) | Confirm::ReplaceSharedFile(p) => {
                            self.do_save(&ctx, p)
                        }
                        Confirm::QuitUnsaved => {
                            self.allow_close = true;
                            self.close_after_requested_saves(&ctx);
                        }
                        Confirm::Export {
                            path,
                            format,
                            losses,
                            ..
                        } => self.do_export(&path, format, &losses),
                    }
                    self.confirm = None;
                }
                if ui.button("Cancel").clicked() {
                    self.confirm = None;
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_keeps_its_own_colors_instead_of_outgoing_controls() {
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        let ctx = egui::Context::default();
        let outgoing = app.active_tab_id();
        let controls = paintcraft_ui::chrome::Controls {
            tool: app.editor.tool,
            options: app.editor.options,
            primary: Rgba([12, 34, 56, 255]),
            secondary: Rgba([65, 43, 21, 255]),
        };
        let default_colors = (app.editor.color, app.editor.secondary);
        app.apply_chrome(&ctx, controls, vec![paintcraft_ui::chrome::Action::New]);
        assert_eq!(app.document_count(), 2);
        assert_eq!((app.editor.color, app.editor.secondary), default_colors);
        app.activate_tab(outgoing, &ctx);
        assert_eq!(app.editor.color, Rgba([12, 34, 56, 255]));
        assert_eq!(app.editor.secondary, Rgba([65, 43, 21, 255]));
    }
}
