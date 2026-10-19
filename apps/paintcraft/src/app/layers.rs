//! Studio layer actions reuse typed engine transactions and stable layer IDs.
use super::*;
use paintcraft_ui::actions::LayerAction;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PropertiesDraft {
    tab: Uuid,
    layer: Uuid,
    revision: (Uuid, u64),
    values: paintcraft_studio::LayerProperties,
}

impl App {
    pub(super) fn dispatch_layer_action(&mut self, ctx: &egui::Context, action: LayerAction) {
        self.resolve_tab_input(ctx);
        let command = if action == LayerAction::Add {
            Some(Action::AddLayer)
        } else {
            let Some(id) = self.editor.target() else {
                return;
            };
            let doc = self.editor.document();
            let Some(index) = doc.roots.iter().position(|r| *r == id) else {
                return;
            };
            let top = doc.roots.len() - 1;
            match action {
                LayerAction::Properties => {
                    let layer = &doc.layers[&id];
                    self.confirm = Some(Confirm::LayerProperties(PropertiesDraft {
                        tab: self.active_tab_id(),
                        layer: id,
                        revision: self.editor.render_key(),
                        values: paintcraft_studio::LayerProperties {
                            name: layer.name.clone(),
                            visible: layer.visible,
                            opacity: layer.opacity,
                        },
                    }));
                    None
                }
                LayerAction::Duplicate => Some(Action::DuplicateLayer { layer: id }),
                LayerAction::Delete => Some(Action::DeleteLayer { layer: id }),
                LayerAction::ToggleVisibility => Some(Action::SetVisible {
                    layer: id,
                    visible: !doc.layers[&id].visible,
                }),
                LayerAction::MoveTop
                | LayerAction::MoveUp
                | LayerAction::MoveDown
                | LayerAction::MoveBottom => {
                    let to_index = match action {
                        LayerAction::MoveTop => top,
                        LayerAction::MoveUp => (index + 1).min(top),
                        LayerAction::MoveDown => index.saturating_sub(1),
                        _ => 0,
                    };
                    Some(Action::MoveLayer {
                        layer: id,
                        to_index,
                    })
                }
                LayerAction::GoTop
                | LayerAction::GoAbove
                | LayerAction::GoBelow
                | LayerAction::GoBottom => {
                    let next = match action {
                        LayerAction::GoTop => top,
                        LayerAction::GoAbove => (index + 1).min(top),
                        LayerAction::GoBelow => index.saturating_sub(1),
                        _ => 0,
                    };
                    let next_id = doc.roots[next];
                    self.editor.set_target(next_id);
                    self.name_edit = None;
                    self.opacity_edit = None;
                    None
                }
                LayerAction::Add => unreachable!(),
            }
        };
        if let Some(command) = command {
            self.request(
                ctx,
                paintcraft_ui::actions::Action::Layer(action).activation_code(),
                command,
            );
        }
    }
}

impl App {
    pub(crate) fn layer_properties_dialog(
        &mut self,
        ctx: &egui::Context,
        mut draft: PropertiesDraft,
    ) {
        if draft.tab != self.active_tab_id()
            || draft.revision != self.editor.render_key()
            || self.editor.document().layer(&draft.layer).is_none()
        {
            self.confirm = None;
            self.editor
                .set_status("Layer properties were not applied: the source document changed.");
            return;
        }
        match paintcraft_studio::layer_properties_dialog(ctx, &mut draft.values) {
            paintcraft_studio::PropertiesResponse::Pending => {
                self.confirm = Some(Confirm::LayerProperties(draft))
            }
            paintcraft_studio::PropertiesResponse::Cancel => self.confirm = None,
            paintcraft_studio::PropertiesResponse::Apply => {
                self.confirm = None;
                self.request(
                    ctx,
                    paintcraft_ui::actions::Action::Layer(LayerAction::Properties)
                        .activation_code(),
                    Action::SetLayerProperties {
                        layer: draft.layer,
                        name: draft.values.name,
                        visible: draft.values.visible,
                        opacity: draft.values.opacity,
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_reject_stale_document_revisions_without_applying_to_another_layer() {
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        app.dispatch_layer_action(&ctx, LayerAction::Properties);
        let Some(Confirm::LayerProperties(mut draft)) = app.confirm.clone() else {
            panic!("properties draft");
        };
        draft.values.name = "Wrong revision".into();
        app.editor.request(987, Action::AddLayer);
        app.editor.run_pending();
        let before = app.editor.document_snapshot();
        app.layer_properties_dialog(&ctx, draft);
        assert!(app.confirm.is_none());
        assert!(app.editor.document().same_semantics(&before));
        app.dispatch_layer_action(&ctx, LayerAction::Properties);
        let Some(Confirm::LayerProperties(mut draft)) = app.confirm.clone() else {
            panic!("properties draft");
        };
        draft.tab = Uuid::new_v4();
        app.layer_properties_dialog(&ctx, draft);
        assert!(app.confirm.is_none());
        assert!(app.editor.document().same_semantics(&before));
    }
}
