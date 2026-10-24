//! Host integration for frontend layer controls and committed property edits.
use super::*;
use paintcraft_ui::t;

impl App {
    fn classic_layer_list(&mut self, ui: &mut egui::Ui) {
        use paintcraft_classic::layers::{Action as Intent, Layer};
        let document = self.editor.document_snapshot();
        self.layer_thumbnails
            .begin(ui.ctx(), &document, &self.worker_budget);
        let rows = self.editor.layers();
        let view: Vec<_> = rows
            .iter()
            .map(|row| Layer {
                id: row.id,
                name: &row.name,
                visible: row.visible,
                preview: self.layer_thumbnails.preview(row.id),
            })
            .collect();
        let target = self.editor.target();
        let ctx = ui.ctx().clone();
        let mut visible_previews = Vec::new();
        let actions = paintcraft_classic::layers::panel(ui, &view, target, &mut visible_previews);
        self.layer_thumbnails
            .request(&ctx, document, &visible_previews, &self.worker_budget);
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
            if self.experience() == Experience::Classic {
                ui.scope(|ui| {
                    paintcraft_classic::layers::style(ui);
                    self.layer_controls(ui);
                });
            } else {
                self.layer_controls(ui);
            }
        });
    }

    fn layer_controls(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let classic = self.experience() == Experience::Classic;
        let property_height = paintcraft_classic::layers::property_height(ui);
        if self.document_count() == 0 {
            ui.label(t!(ui.ctx(), "No document open"));
            return;
        }
        // Finish a focused property's draft against the layer it belonged to
        // before this frame's list click changes the drawing target.
        let target = self.editor.target();
        if self.experience() == Experience::Classic {
            self.classic_layer_list(ui);
        } else {
            ui.horizontal(|ui| {
                ui.heading(t!(ui.ctx(), "Layers"));
                if ui.button(t!(ui.ctx(), "Add layer")).clicked() {
                    self.request(&ctx, code::ADD_LAYER, Action::AddLayer);
                }
            });
            for row in self.editor.layers() {
                ui.horizontal(|ui| {
                    let mut visible = row.visible;
                    let toggle = ui
                        .checkbox(&mut visible, t!(ui.ctx(), ""))
                        .on_hover_text(t!(ui.ctx(), "Visible"));
                    let accessible =
                        t!(ui.ctx(), "Show { $name }", &[("name", row.name.to_owned())]);
                    toggle.widget_info(|| {
                        egui::WidgetInfo::selected(egui::Role::CheckBox, true, visible, &accessible)
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
            ui.label(t!(ui.ctx(), "Drawing on: (no layer)"));
            return;
        };
        let Some((name, opacity, index)) = self.editor.layer_properties(t) else {
            return;
        };
        let drawing_on = t!(ui.ctx(), "Drawing on: { $name }", &[("name", name.clone())]);
        if self.experience() == Experience::Classic {
            ui.add(
                egui::Label::new(drawing_on)
                    .truncate()
                    .show_tooltip_when_elided(false),
            )
            .on_hover_text(&name);
        } else {
            ui.label(drawing_on);
        }
        let top = self.editor.document().roots.len() - 1;

        // Name: committed on focus loss; Escape discards the local draft.
        let buf = match &mut self.name_edit {
            Some(b) if b.0 == t => b,
            other => other.insert((t, name.clone())),
        };
        ui.horizontal(|ui| {
            let label = if classic {
                paintcraft_classic::layers::property_label(ui, "Layer name")
            } else {
                ui.label(t!(ui.ctx(), "Layer name"))
            };
            let field_id = ui.scope_id().with(("layer-name", t));
            let focused = ui.memory(|memory| memory.has_focus(field_id));
            let cancel = focused
                && ui.ctx().current_pass_index() == 0
                && ui
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if cancel {
                buf.1.clone_from(&name);
            }
            let editor = egui::TextEdit::singleline(&mut buf.1)
                .id(field_id)
                .event_filter(egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                    ..Default::default()
                });
            let editor = if classic {
                editor
                    .min_size(Vec2::new(0.0, property_height))
                    .align(egui::Align2::LEFT_CENTER)
            } else {
                editor
            };
            let r = ui.add(editor).labelled_by(label.id);
            if cancel {
                r.surrender_focus();
            } else if r.lost_focus() {
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
            let label = if classic {
                paintcraft_classic::layers::property_label(ui, "Opacity")
            } else {
                ui.label(t!(ui.ctx(), "Opacity"))
            };
            let field_id = ui.next_auto_id();
            let focused = ui.memory(|memory| memory.has_focus(field_id));
            let cancel_id = field_id.with("opacity-cancel-frame");
            let frame = ui.ctx().cumulative_frame_nr();
            if focused && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                ui.data_mut(|data| data.insert_temp(cancel_id, frame));
            }
            let cancel = ui.data(|data| data.get_temp::<u64>(cancel_id)) == Some(frame);
            if cancel {
                obuf.1 = opacity;
                ui.memory_mut(|memory| memory.surrender_focus(field_id));
            }
            let editor = egui::DragValue::new(&mut obuf.1)
                .range(0..=255)
                .update_while_editing(false);
            let editor = if classic {
                editor.min_size(Vec2::new(ui.available_width(), property_height))
            } else {
                editor
            };
            let r = ui.add(editor).labelled_by(label.id);
            if cancel {
                obuf.1 = opacity;
                r.surrender_focus();
                // Its private edit buffer may be parsed on a later pass after
                // Escape is consumed; suppress publication for the whole frame.
            } else if r.drag_stopped() || r.lost_focus() {
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
            if r.has_focus() {
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        r.id,
                        egui::EventFilter {
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            escape: true,
                            ..Default::default()
                        },
                    )
                });
            }
        });

        if self.experience() == Experience::Classic {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(index < top, egui::Button::new(t!(ui.ctx(), "Move up")))
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
                .add_enabled(index > 0, egui::Button::new(t!(ui.ctx(), "Move down")))
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
                        egui::Button::new(t!(ui.ctx(), "Duplicate layer")),
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
            if self.experience() == Experience::Studio
                && ui.button(t!(ui.ctx(), "Layer properties…")).clicked()
            {
                self.dispatch_chrome(
                    &ctx,
                    vec![paintcraft_ui::actions::Action::Layer(
                        paintcraft_ui::actions::LayerAction::Properties,
                    )],
                );
            }
            if ui.button(t!(ui.ctx(), "Delete layer")).clicked() {
                self.request(&ctx, code::DELETE, Action::DeleteLayer { layer: t });
            }
        });
    }
}
