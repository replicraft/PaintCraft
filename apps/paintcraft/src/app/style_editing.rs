//! Native Studio drafts stay private until their exact captured FIFO receipt.
use super::*;
use paintcraft_commands::{LayerRef, StyleOp, StyleRef};
use paintcraft_core::types::styles::StyleDescriptor;
use paintcraft_studio::styles::{self, Intent, Parameters, Response};
use paintcraft_tools::StyleEdit;
use paintcraft_ui::t;

#[derive(Debug)]
pub(super) struct State {
    tab: Uuid,
    owner: Uuid,
    layer: Uuid,
    intent: Intent<Uuid>,
    draft: Option<Draft>,
}
#[derive(Debug)]
struct Draft {
    base: StyleEdit,
    parameters: Parameters,
    enabled: bool,
    attempt: Option<Uuid>,
}
impl App {
    pub(super) fn studio_style_panel(&mut self, ui: &mut egui::Ui) {
        if self.experience() != Experience::Studio || self.document_count() == 0 {
            return;
        }
        let Some(layer) = self
            .editor
            .target()
            .and_then(|id| self.editor.document().layer(&id))
        else {
            return;
        };
        let rows: Vec<_> = layer
            .styles
            .iter()
            .map(|s| styles::Row {
                id: s.id(),
                enabled: s.enabled(),
                descriptor: s.descriptor(),
            })
            .collect();
        let available = self.editor.style_worker_available();
        let actions = styles::panel(ui, &rows, rows.len() < 256, available);
        if !available {
            ui.label(t!(ui.ctx(), "Layer styles require the editing worker"));
        }
        if ui.ctx().current_pass_index() != 0 {
            return;
        }
        for action in actions {
            self.begin_studio_style(ui.ctx(), action);
        }
    }
    pub(super) fn begin_studio_style(&mut self, ctx: &egui::Context, intent: Intent<Uuid>) {
        if self.experience() != Experience::Studio
            || self.style_editing.is_some()
            || self.filter_editing.is_some()
            || self.confirm.is_some()
            || self.document_count() == 0
            || self.ime_composing
            || self.editor.gesture_in_progress()
        {
            return;
        }
        if let Err(error) = self.editor.begin_style_edit() {
            self.editor.set_status(format!("Style not opened: {error}"));
            return;
        }
        self.resolve_tab_input(ctx);
        let Some(layer) = self.editor.target() else {
            return;
        };
        self.style_editing = Some(State {
            tab: self.active_tab_id(),
            owner: self.editor.session_owner_id(),
            layer,
            intent,
            draft: None,
        });
        self.confirm = Some(Confirm::StyleParameters);
        self.pump_style_edit(ctx);
    }
    pub(super) fn parameter_navigation_ready(&self) -> bool {
        self.filter_navigation_ready() || self.style_navigation_ready()
    }
    pub(super) fn style_navigation_ready(&self) -> bool {
        self.confirm == Some(Confirm::StyleParameters)
            && self
                .style_editing
                .as_ref()
                .is_some_and(|s| s.draft.is_some() && self.style_state_current(s))
    }
    pub(super) fn style_waiting_prefix(&self) -> bool {
        self.style_editing
            .as_ref()
            .is_some_and(|s| s.draft.is_none())
    }
    fn retire_style_owner(&mut self, owner: Uuid, attempt: Option<Uuid>) {
        let retire = |editor: &mut Editor| {
            if editor.session_owner_id() == owner {
                if let Some(id) = attempt {
                    editor.cancel_style_attempt(id);
                }
                editor.set_style_edit_open(false);
            }
        };
        retire(&mut self.editor);
        for tab in &mut self.tabs {
            if let Some(s) = &mut tab.state {
                retire(&mut s.editor);
            }
        }
        for workspace in self.parked_workspaces.values_mut() {
            retire(&mut workspace.active.editor);
            for tab in &mut workspace.tabs {
                if let Some(s) = &mut tab.state {
                    retire(&mut s.editor);
                }
            }
        }
    }
    pub(super) fn cancel_style_edit(&mut self) {
        if let Some(state) = self.style_editing.take() {
            self.retire_style_owner(state.owner, state.draft.as_ref().and_then(|d| d.attempt));
        }
        if self.confirm == Some(Confirm::StyleParameters) {
            self.confirm = None;
        }
    }
    fn style_state_current(&self, state: &State) -> bool {
        state.tab == self.active_tab_id()
            && state.owner == self.editor.session_owner_id()
            && self.experience() == Experience::Studio
            && self.editor.target() == Some(state.layer)
            && self.editor.document().layer(&state.layer).is_some()
            && state
                .draft
                .as_ref()
                .is_none_or(|d| self.editor.style_base_current(&d.base))
    }
    pub(super) fn pump_style_edit(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.style_editing.take() else {
            return;
        };
        if let Some(draft) = &mut state.draft
            && let Some(id) = draft.attempt
        {
            match self.editor.style_feedback(id) {
                Some(controller::StyleFeedback::Accepted)
                    if state.owner == self.editor.session_owner_id() =>
                {
                    self.editor.set_style_edit_open(false);
                    self.confirm = None;
                    return;
                }
                Some(controller::StyleFeedback::Failed) => draft.attempt = None,
                _ => {}
            }
        }
        if !self.style_state_current(&state) || self.confirm != Some(Confirm::StyleParameters) {
            self.retire_style_owner(state.owner, state.draft.as_ref().and_then(|d| d.attempt));
            if self.confirm == Some(Confirm::StyleParameters) {
                self.confirm = None;
            }
            return;
        }
        if state.draft.is_none() {
            if self.editor.failed_text_retained() {
                self.confirm = None;
                return;
            }
            if self.editor.has_pending_edits()
                || self.editor.is_editing_text()
                || !self.deferred_files.is_empty()
            {
                self.style_editing = Some(state);
                ctx.request_repaint();
                return;
            }
            let Some(operation) = operation(&self.editor, state.layer, state.intent) else {
                self.confirm = None;
                self.editor
                    .set_status("Style not applied: its document changed");
                return;
            };
            let values = match &operation {
                StyleOp::Add {
                    descriptor,
                    enabled,
                    ..
                } => Some((Parameters::from_descriptor(*descriptor), *enabled)),
                StyleOp::Update {
                    descriptor,
                    style: StyleRef::Id(id),
                    ..
                } => self
                    .editor
                    .document()
                    .layer(&state.layer)
                    .and_then(|l| l.styles.iter().find(|s| s.id() == *id))
                    .map(|s| (Parameters::from_descriptor(*descriptor), s.enabled())),
                _ => None,
            };
            let edit = self.editor.style_capture(operation);
            if let Some((parameters, enabled)) = values {
                self.editor.set_style_edit_open(true);
                self.editor.set_style_preview(Some(edit.clone()));
                state.draft = Some(Draft {
                    base: edit,
                    parameters,
                    enabled,
                    attempt: None,
                });
            } else {
                self.confirm = None;
                if let Err(error) = self
                    .editor
                    .queue_style_apply(token(ctx, 0x5354594c45), edit)
                {
                    self.editor
                        .set_status(format!("Style not applied: {error}"));
                }
                return;
            }
        }
        self.style_editing = Some(state);
    }
    pub(super) fn studio_style_dialog(&mut self, ctx: &egui::Context) {
        self.pump_style_edit(ctx);
        let Some(mut state) = self.style_editing.take() else {
            return;
        };
        let Some(draft) = &mut state.draft else {
            let response =
                egui::Modal::new(egui::Id::unique("style-prefix-wait")).show(ctx, |ui| {
                    ui.spinner();
                    ui.button(t!(ctx, "Cancel")).clicked()
                });
            self.style_editing = Some(state);
            if response.inner || response.should_close() {
                self.cancel_style_edit();
            }
            return;
        };
        if draft.attempt.is_some() {
            let response = egui::Modal::new(egui::Id::unique("style-apply-wait")).show(ctx, |ui| {
                ui.heading(t!(ctx, "Layer styles"));
                ui.spinner();
                ui.button(t!(ctx, "Cancel")).clicked()
            });
            self.style_editing = Some(state);
            if response.inner || response.should_close() {
                self.cancel_style_edit();
            }
            return;
        }
        match styles::parameter_dialog(ctx, &mut draft.parameters, draft.enabled) {
            Response::Pending => self.style_editing = Some(state),
            Response::Preview(descriptor) => {
                self.editor
                    .set_style_preview(descriptor.map(|d| edited(&draft.base, d)));
                self.style_editing = Some(state);
            }
            Response::Cancel => {
                self.style_editing = Some(state);
                self.cancel_style_edit();
            }
            Response::Apply(descriptor) => {
                match self
                    .editor
                    .queue_style_apply(token(ctx, 0x5354594c45), edited(&draft.base, descriptor))
                {
                    Ok(id) => draft.attempt = Some(id),
                    Err(error) => self
                        .editor
                        .set_status(format!("Style not applied: {error}")),
                }
                self.style_editing = Some(state);
            }
        }
    }
}
fn edited(base: &StyleEdit, descriptor: StyleDescriptor) -> StyleEdit {
    let mut edit = base.clone();
    match &mut edit.operation {
        StyleOp::Add {
            descriptor: value, ..
        }
        | StyleOp::Update {
            descriptor: value, ..
        } => *value = descriptor,
        _ => unreachable!("only style parameter operations open this draft"),
    }
    edit
}
fn operation(editor: &Editor, layer: Uuid, intent: Intent<Uuid>) -> Option<StyleOp> {
    let source = editor.document().layer(&layer)?;
    let layer = LayerRef::Id(layer);
    let reference = |id| {
        source
            .styles
            .iter()
            .find(|s| s.id() == id)
            .map(|_| StyleRef::Id(id))
    };
    Some(match intent {
        Intent::Add if source.styles.len() < 256 => {
            let c = editor.color.0;
            StyleOp::Add {
                layer,
                alias: "studio_style".into(),
                descriptor: Parameters::new([c[0], c[1], c[2]]).descriptor(),
                enabled: true,
                to_index: None,
            }
        }
        Intent::Edit(id) => StyleOp::Update {
            layer,
            style: reference(id)?,
            descriptor: source.styles.iter().find(|s| s.id() == id)?.descriptor(),
        },
        Intent::SetEnabled { id, enabled } => StyleOp::SetEnabled {
            layer,
            style: reference(id)?,
            enabled,
        },
        Intent::Move { id, to_index } if to_index < source.styles.len() => StyleOp::Move {
            layer,
            style: reference(id)?,
            to_index,
        },
        Intent::Duplicate(id) if source.styles.len() < 256 => StyleOp::Duplicate {
            layer,
            style: reference(id)?,
            alias: "studio_style".into(),
            to_index: None,
        },
        Intent::Remove(id) => StyleOp::Remove {
            layer,
            style: reference(id)?,
        },
        _ => return None,
    })
}

#[cfg(test)]
#[path = "style_editing/tests.rs"]
mod tests;
