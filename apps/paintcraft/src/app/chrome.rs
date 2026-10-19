//! Desktop chrome integration.

use super::*;

impl App {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut controls = paintcraft_classic::Controls {
            tool: self.editor.tool,
            options: self.editor.options,
            primary: self.editor.color,
            secondary: self.editor.secondary,
        };
        let actions = paintcraft_classic::chrome(ui, self.variant(), &mut controls);
        for action in actions {
            self.chrome_outbox
                .push(token(&ctx, action.activation_code()), action);
        }
        for action in self.chrome_outbox.drain() {
            use paintcraft_classic::Action as Chrome;
            match action {
                Chrome::New => {
                    if self.editor.is_dirty() {
                        self.confirm = Some(Confirm::DiscardForNew);
                    } else {
                        self.new_document();
                    }
                }
                Chrome::Open => self.open_requested(&ctx),
                Chrome::Save => self.save_requested(&ctx),
                Chrome::SaveAs => self.request_dialog(&ctx, Kind::SaveAs),
                Chrome::Export => self.export_requested(&ctx),
                Chrome::Undo => self.request(&ctx, code::UNDO, Action::Undo),
                Chrome::Redo => self.request(&ctx, code::REDO, Action::Redo),
                Chrome::Interface => self.picking = Some(self.variant()),
                Chrome::SelectTool(tool) => self.editor.select_tool(tool),
                Chrome::SelectAll => {
                    let (w, h) = self.editor.document().canvas;
                    self.request(
                        &ctx,
                        code::SELECT_ALL,
                        Action::Select {
                            corners: Some(((0, 0), (i64::from(w) - 1, i64::from(h) - 1))),
                        },
                    );
                }
                Chrome::Deselect => {
                    self.request(&ctx, code::DESELECT, Action::Select { corners: None })
                }
            }
        }
        self.editor.options = controls.options;
        self.editor.color = controls.primary;
        self.editor.secondary = controls.secondary;
    }

    pub(super) fn layer_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
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

        ui.horizontal(|ui| {
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
            if ui.button("Delete layer").clicked() {
                self.request(&ctx, code::DELETE, Action::DeleteLayer { layer: t });
            }
        });
    }

    pub(super) fn confirm_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(confirm) = self.confirm.clone() else {
            return;
        };
        let ctx = ui.ctx().clone();
        let (title, text, go) = match &confirm {
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
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(&ctx, |ui| {
                ui.label(text);
                ui.horizontal(|ui| {
                    if ui.button(go).clicked() {
                        match confirm {
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
                            Confirm::ReplaceFile(p) => self.do_save(&ctx, p),
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
