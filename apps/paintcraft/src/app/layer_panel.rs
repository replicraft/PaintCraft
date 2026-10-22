//! Host integration for frontend layer controls and committed property edits.
use super::*;

impl App {
    fn classic_layer_list(&mut self, ui: &mut egui::Ui) {
        use paintcraft_classic::layers::{Action as Intent, Layer};
        let rows = self.editor.layers();
        let view: Vec<_> = rows
            .iter()
            .map(|row| Layer {
                id: row.id,
                name: &row.name,
                visible: row.visible,
            })
            .collect();
        let target = self.editor.target();
        let ctx = ui.ctx().clone();
        let actions = paintcraft_classic::layers::panel(ui, &view, target);
        if ctx.current_pass_index() != 0 {
            return;
        }
        for action in actions {
            match action {
                Intent::Close => self.classic.layers = false,
                Intent::Add => self.request(&ctx, code::ADD_LAYER, Action::AddLayer),
                Intent::Select(id) => {
                    self.editor.set_target(id);
                }
                Intent::SetVisible { layer, visible } => {
                    self.editor.request(
                        token(&ctx, code::VISIBLE ^ layer.as_u128() as u64),
                        Action::SetVisible { layer, visible },
                    );
                }
                Intent::MoveUp | Intent::MoveDown => {
                    if let Some(layer) = target
                        && let Some((_, _, index)) = self.editor.layer_properties(layer)
                    {
                        let to_index = if action == Intent::MoveUp {
                            index + 1
                        } else {
                            index.saturating_sub(1)
                        };
                        self.request(&ctx, code::MOVE, Action::MoveLayer { layer, to_index });
                    }
                }
                Intent::Delete => {
                    if let Some(layer) = target {
                        self.request(&ctx, code::DELETE, Action::DeleteLayer { layer });
                    }
                }
            }
        }
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
        // Finish a focused property's draft against the layer it belonged to
        // before this frame's list click changes the drawing target.
        let target = self.editor.target();
        if self.experience() == Experience::Classic {
            self.classic_layer_list(ui);
        } else {
            ui.horizontal(|ui| {
                ui.heading("Layers");
                if ui.button("Add layer").clicked() {
                    self.request(&ctx, code::ADD_LAYER, Action::AddLayer);
                }
            });
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

        if self.experience() == Experience::Classic {
            return;
        }
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
}
