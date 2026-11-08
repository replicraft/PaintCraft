//! Studio layer actions reuse typed engine transactions and stable layer IDs.
use super::*;
use paintcraft_ui::actions::LayerAction;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PropertiesDraft {
    tab: Uuid,
    owner: Uuid,
    experience: Experience,
    layer: Uuid,
    ready: Option<ReadyProperties>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReadyProperties {
    capture: controller::PropertiesCapture,
    values: paintcraft_studio::LayerProperties,
    desired_mode: paintcraft_core::types::LayerBlendMode,
}

impl App {
    pub(super) fn dispatch_layer_action(&mut self, ctx: &egui::Context, action: LayerAction) {
        if self.document_count() == 0 {
            return;
        }
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
                    let mut draft = PropertiesDraft {
                        tab: self.active_tab_id(),
                        owner: self.editor.properties_owner(),
                        experience: self.experience(),
                        layer: id,
                        ready: None,
                    };
                    // Resolve accepted text/edits first; never freeze pre-prefix values.
                    if !self.editor.has_pending_edits()
                        && !self.editor.is_editing_text()
                        && self.deferred_files.is_empty()
                    {
                        draft.ready = self.capture_properties_values(id).ok();
                    }
                    self.confirm = Some(Confirm::LayerProperties(draft));
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
    fn capture_properties_values(
        &self,
        layer: Uuid,
    ) -> paintcraft_core::types::Result<ReadyProperties> {
        let capture = self.editor.capture_properties(layer)?;
        let layer = &self.editor.document().layers[&layer];
        Ok(ReadyProperties {
            capture,
            values: paintcraft_studio::LayerProperties {
                name: layer.name.clone(),
                visible: layer.visible,
                opacity: layer.opacity,
            },
            desired_mode: layer.blend_mode,
        })
    }
    pub(crate) fn layer_properties_dialog(
        &mut self,
        ctx: &egui::Context,
        mut draft: PropertiesDraft,
    ) {
        if self.document_count() == 0 {
            self.confirm = None;
            return;
        }
        if draft.tab != self.active_tab_id()
            || draft.owner != self.editor.properties_owner()
            || draft.experience != self.experience()
            || self.editor.target() != Some(draft.layer)
            || self.editor.document().layer(&draft.layer).is_none()
            || draft
                .ready
                .as_ref()
                .is_some_and(|r| !self.editor.properties_capture_current(&r.capture))
        {
            self.confirm = None;
            self.editor
                .set_status("Layer properties were not applied: the source document changed.");
            return;
        }
        if draft.ready.is_none() {
            if self.editor.failed_text_retained() {
                self.confirm = None;
                return;
            }
            if self.editor.has_pending_edits()
                || self.editor.is_editing_text()
                || !self.deferred_files.is_empty()
            {
                let response =
                    egui::Modal::new(egui::Id::unique("layer-properties-prefix")).show(ctx, |ui| {
                        ui.spinner();
                        ui.button(paintcraft_ui::t!(ui.ctx(), "Cancel")).clicked()
                    });
                if response.inner || response.should_close() {
                    self.confirm = None;
                } else {
                    self.confirm = Some(Confirm::LayerProperties(draft));
                    ctx.request_repaint();
                }
                return;
            }
            match self.capture_properties_values(draft.layer) {
                Ok(ready) => draft.ready = Some(ready),
                Err(error) => {
                    self.confirm = None;
                    self.editor.set_status(format!("Edit failed: {error}"));
                    return;
                }
            }
        }
        let ready = draft
            .ready
            .as_mut()
            .expect("captured after accepted prefix");
        let response = if self.experience() == Experience::Studio {
            paintcraft_studio::layer_properties_with_blend_dialog(
                ctx,
                &mut ready.values,
                &mut ready.desired_mode,
            )
        } else {
            paintcraft_studio::layer_properties_dialog(ctx, &mut ready.values)
        };
        match response {
            paintcraft_studio::PropertiesResponse::Pending => {
                self.confirm = Some(Confirm::LayerProperties(draft))
            }
            paintcraft_studio::PropertiesResponse::Cancel => self.confirm = None,
            paintcraft_studio::PropertiesResponse::Apply => {
                self.confirm = None;
                let ready = draft.ready.take().expect("complete owned properties");
                let activation = paintcraft_ui::actions::Action::Layer(LayerAction::Properties)
                    .activation_code();
                if self.experience() == Experience::Studio {
                    let request = controller::PropertiesRequest {
                        capture: ready.capture,
                        desired_mode: ready.desired_mode,
                        values: ready.values,
                    };
                    self.editor
                        .request_layer_properties(token(ctx, activation), request);
                } else {
                    self.request(
                        ctx,
                        activation,
                        Action::SetLayerProperties {
                            layer: draft.layer,
                            name: ready.values.name,
                            visible: ready.values.visible,
                            opacity: ready.values.opacity,
                        },
                    );
                }
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
        draft.ready.as_mut().unwrap().values.name = "Wrong revision".into();
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

#[cfg(test)]
#[path = "layers/blend_tests.rs"]
mod blend_tests;
