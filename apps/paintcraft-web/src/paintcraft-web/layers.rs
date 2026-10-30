//! Classic layer intentions operate on the browser's own session.
use crate::{app::WebApp, editor::layer_ref};
use eframe::egui;
use paintcraft_classic::layers::{self, Action};
use paintcraft_core::types::json::Value;

impl WebApp {
    pub(crate) fn layers(&mut self, ui: &mut egui::Ui) {
        if !self.workspace.layers {
            return;
        }
        let blocked = self.blocked() || self.paste.is_placing();
        let editor = &self.editor;
        let classic = self.frontend == paintcraft_ui::actions::Frontend::Classic;
        let draw = |ui: &mut egui::Ui| {
            ui.add_enabled_ui(!blocked, |ui| {
                if classic {
                    layers::style(ui);
                }
                let doc = editor.session.document();
                let rows: Vec<_> = doc
                    .roots
                    .iter()
                    .rev()
                    .map(|id| &doc.layers[id])
                    .map(|layer| layers::Layer {
                        id: layer.id,
                        name: layer.name.as_str(),
                        visible: layer.visible,
                        preview: layers::Preview::Unavailable,
                    })
                    .collect();
                let actions = layers::panel(ui, &rows, editor.target, &mut Vec::new());
                let filter_actions = if !classic {
                    ui.separator();
                    let filters: Vec<_> = editor
                        .target
                        .and_then(|id| doc.layer(&id))
                        .map(|layer| {
                            layer
                                .filters
                                .iter()
                                .map(|f| paintcraft_studio::filters::Row {
                                    id: f.id(),
                                    enabled: f.enabled(),
                                    descriptor: f.descriptor(),
                                    mask: f.mask().map(|m| {
                                        let stats = m.coverage().stats();
                                        paintcraft_studio::filters::MaskSummary {
                                            enabled: m.enabled(),
                                            runs: stats.runs,
                                        }
                                    }),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    paintcraft_studio::filters::panel(
                        ui,
                        &filters,
                        editor.target.is_some() && filters.len() < 256,
                        !matches!(
                            editor.session.selection(),
                            paintcraft_editor::execute::Selection::Inactive
                        ),
                    )
                } else {
                    Vec::new()
                };
                (actions, filter_actions)
            })
            .inner
        };
        let (actions, filter_actions) = if self.frontend == paintcraft_ui::actions::Frontend::Studio
        {
            let mut actions = (Vec::new(), Vec::new());
            paintcraft_studio::layers_window(
                ui.ctx(),
                &mut self.studio.layers,
                self.studio.canvas_top,
                |ui| {
                    ui.set_max_height(300.0);
                    actions = draw(ui);
                },
            );
            actions
        } else {
            egui::Panel::right("web-layers")
                .default_size(240.0)
                .show(ui, draw)
                .inner
        };
        for intent in filter_actions {
            self.begin_studio_filter(ui.ctx(), intent);
        }
        for action in actions {
            if !matches!(action, Action::Close) && !self.finish_text() {
                continue;
            }
            self.editor.tools.cancel_gesture();
            let op = match action {
                Action::Close => {
                    self.workspace.layers = false;
                    if self.frontend == paintcraft_ui::actions::Frontend::Studio {
                        self.studio.layers = false;
                    }
                    continue;
                }
                Action::Select(layer) => {
                    self.editor.target = Some(layer);
                    continue;
                }
                Action::Add => {
                    let (w, h) = self.editor.session.document().canvas;
                    Value::object([
                        ("op", Value::str("layer.add")),
                        ("ref", Value::str("layer")),
                        (
                            "name",
                            Value::str(format!(
                                "Layer {}",
                                self.editor.session.document().roots.len() + 1
                            )),
                        ),
                        ("width", Value::Int(w.into())),
                        ("height", Value::Int(h.into())),
                        (
                            "origin",
                            Value::object([("x", Value::Int(0)), ("y", Value::Int(0))]),
                        ),
                        ("default_pixel", Value::str("#00000000")),
                    ])
                }
                Action::SetVisible { layer, visible } => Value::object([
                    ("op", Value::str("layer.set_properties")),
                    ("layer_ref", layer_ref(layer)),
                    ("visible", Value::Bool(visible)),
                ]),
                Action::Delete => {
                    let Some(layer) = self.editor.target else {
                        continue;
                    };
                    Value::object([
                        ("op", Value::str("layer.delete")),
                        ("layer_ref", layer_ref(layer)),
                    ])
                }
                Action::MoveUp | Action::MoveDown => {
                    let Some(layer) = self.editor.target else {
                        continue;
                    };
                    let doc = self.editor.session.document();
                    let Some(index) = doc.roots.iter().position(|id| *id == layer) else {
                        continue;
                    };
                    let next = if matches!(action, Action::MoveUp) {
                        (index + 1).min(doc.roots.len() - 1)
                    } else {
                        index.saturating_sub(1)
                    };
                    Value::object([
                        ("op", Value::str("layer.reorder")),
                        ("layer_ref", layer_ref(layer)),
                        ("to_index", Value::Int(next as i64)),
                    ])
                }
            };
            match self.editor.commit(vec![op]) {
                Ok(()) => self.message = "Layers changed - unsaved".into(),
                Err(error) => self.message = format!("Layer change failed: {error}"),
            }
        }
    }
}
