//! Studio filter drafts are display-only until their captured FIFO activation.
use super::*;
use paintcraft_commands::{FilterOp, FilterRef, LayerRef};
use paintcraft_core::types::filters::Descriptor;
use paintcraft_studio::filters::{self, Action as Intent, ParameterResponse, Parameters};
use paintcraft_tools::FilterEdit;
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
    base: FilterEdit,
    parameters: Parameters,
    attempt: Option<Uuid>,
}
impl App {
    pub(super) fn studio_filter_panel(&mut self, ui: &mut egui::Ui) {
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
            .filters
            .iter()
            .map(|f| filters::Row {
                id: f.id(),
                enabled: f.enabled(),
                descriptor: f.descriptor(),
            })
            .collect();
        let actions = filters::panel(ui, &rows, rows.len() < 256);
        for action in actions {
            self.begin_studio_filter(ui.ctx(), action);
        }
    }
    pub(super) fn begin_studio_filter(&mut self, ctx: &egui::Context, intent: Intent<Uuid>) {
        if self.filter_editing.is_some() || self.confirm.is_some() || self.document_count() == 0 {
            return;
        }
        if let Err(error) = self.editor.begin_filter_edit() {
            self.editor
                .set_status(format!("Filter not opened: {error}"));
            return;
        }
        self.resolve_tab_input(ctx);
        let Some(layer) = self.editor.target() else {
            return;
        };
        self.filter_editing = Some(State {
            tab: self.active_tab_id(),
            owner: self.editor.session_owner_id(),
            layer,
            intent,
            draft: None,
        });
        self.confirm = Some(Confirm::FilterParameters);
        self.pump_filter_edit(ctx);
    }
    pub(super) fn filter_navigation_ready(&self) -> bool {
        self.confirm == Some(Confirm::FilterParameters)
            && self
                .filter_editing
                .as_ref()
                .is_some_and(|state| state.draft.is_some() && self.filter_state_current(state))
    }
    pub(super) fn filter_waiting_prefix(&self) -> bool {
        self.filter_editing
            .as_ref()
            .is_some_and(|state| state.draft.is_none())
    }
    fn retire_filter_owner(&mut self, owner: Uuid) {
        let retire = |editor: &mut Editor| {
            if editor.session_owner_id() == owner {
                editor.cancel_active_filter();
                editor.set_filter_edit_open(false);
            }
        };
        retire(&mut self.editor);
        for tab in &mut self.tabs {
            if let Some(state) = &mut tab.state {
                retire(&mut state.editor);
            }
        }
        if let Some(workspace) = &mut self.other_workspace {
            retire(&mut workspace.active.editor);
            for tab in &mut workspace.tabs {
                if let Some(state) = &mut tab.state {
                    retire(&mut state.editor);
                }
            }
        }
    }
    pub(super) fn cancel_filter_edit(&mut self) {
        if let Some(state) = self.filter_editing.take() {
            self.retire_filter_owner(state.owner);
        }
        self.editor.cancel_active_filter();
        self.editor.set_filter_edit_open(false);
        if self.confirm == Some(Confirm::FilterParameters) {
            self.confirm = None;
        }
    }
    fn filter_state_current(&self, state: &State) -> bool {
        state.tab == self.active_tab_id()
            && state.owner == self.editor.session_owner_id()
            && self.experience() == Experience::Studio
            && self.editor.target() == Some(state.layer)
            && self.editor.document().layer(&state.layer).is_some()
            && state
                .draft
                .as_ref()
                .is_none_or(|d| self.editor.filter_base_current(&d.base))
    }
    pub(super) fn pump_filter_edit(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.filter_editing.take() else {
            return;
        };
        if let Some(draft) = &mut state.draft
            && let Some(id) = draft.attempt
        {
            match self.editor.filter_feedback(id) {
                Some(controller::FilterFeedback::Published)
                    if state.owner == self.editor.session_owner_id() =>
                {
                    self.editor.set_filter_edit_open(false);
                    self.confirm = None;
                    return;
                }
                Some(controller::FilterFeedback::Failed) => draft.attempt = None,
                _ => {}
            }
        }
        if !self.filter_state_current(&state) || self.confirm != Some(Confirm::FilterParameters) {
            self.retire_filter_owner(state.owner);
            if self.confirm == Some(Confirm::FilterParameters) {
                self.confirm = None;
            }
            return;
        }
        if state.draft.is_none() {
            // Never capture an old base while text or an accepted prefix is pending.
            if self.editor.failed_text_retained() {
                self.confirm = None;
                return;
            }
            if self.editor.has_pending_edits()
                || self.editor.is_editing_text()
                || !self.deferred_files.is_empty()
            {
                self.filter_editing = Some(state);
                ctx.request_repaint();
                return;
            }
            let operation = operation(&self.editor, state.layer, state.intent);
            let Some(operation) = operation else {
                self.confirm = None;
                return;
            };
            let parameters = match &operation {
                FilterOp::Add { descriptor, .. } | FilterOp::Update { descriptor, .. } => {
                    Some(Parameters::from_descriptor(*descriptor))
                }
                _ => None,
            };
            let edit = self.editor.filter_capture(operation);
            if let Some(parameters) = parameters {
                self.editor.set_filter_edit_open(true);
                self.editor.set_filter_preview(Some(edit.clone()));
                state.draft = Some(Draft {
                    base: edit,
                    parameters,
                    attempt: None,
                });
            } else {
                self.confirm = None;
                self.editor
                    .request(token(ctx, 0x46494c544552), Action::Filter(edit));
                return;
            }
        }
        self.filter_editing = Some(state);
    }
    pub(super) fn studio_filter_dialog(&mut self, ctx: &egui::Context) {
        self.pump_filter_edit(ctx);
        let Some(mut state) = self.filter_editing.take() else {
            return;
        };
        let Some(draft) = state.draft.as_mut() else {
            let response =
                egui::Modal::new(egui::Id::unique("filter-prefix-wait")).show(ctx, |ui| {
                    ui.spinner();
                    ui.button(t!(ui.ctx(), "Cancel")).clicked()
                });
            if response.inner || response.should_close() {
                self.confirm = None;
                self.editor.set_filter_edit_open(false);
            } else {
                self.filter_editing = Some(state);
            }
            return;
        };
        if draft.attempt.is_some() {
            let mut open = true;
            let response = egui::Window::new(t!(ctx, "Filters"))
                .id(egui::Id::unique("filter-apply-wait"))
                .open(&mut open)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.spinner();
                    ui.button(t!(ui.ctx(), "Cancel")).clicked()
                });
            let cancel = !open
                || response.is_some_and(|r| r.inner == Some(true))
                || ctx.input(|i| i.key_pressed(egui::Key::Escape));
            self.filter_editing = Some(state);
            if cancel {
                self.cancel_filter_edit();
            }
            return;
        }
        match filters::parameter_dialog(ctx, &mut draft.parameters) {
            ParameterResponse::Pending => self.filter_editing = Some(state),
            ParameterResponse::Preview(descriptor) => {
                let preview = descriptor.map(|descriptor| edited(&draft.base, descriptor));
                self.editor.set_filter_preview(preview);
                self.filter_editing = Some(state);
            }
            ParameterResponse::Cancel => self.cancel_filter_edit(),
            ParameterResponse::Apply(descriptor) => {
                let edit = edited(&draft.base, descriptor);
                match self
                    .editor
                    .queue_filter_apply(token(ctx, 0x46494c544552), edit)
                {
                    Ok(id) => draft.attempt = Some(id),
                    Err(error) => self
                        .editor
                        .set_status(format!("Filter not applied: {error}")),
                }
                self.filter_editing = Some(state);
            }
        }
    }
}
fn edited(base: &FilterEdit, descriptor: Descriptor) -> FilterEdit {
    let mut edit = base.clone();
    match &mut edit.operation {
        FilterOp::Add { descriptor: d, .. } | FilterOp::Update { descriptor: d, .. } => {
            *d = descriptor
        }
        _ => unreachable!("only parameter operations open a draft"),
    }
    edit
}
fn operation(editor: &Editor, layer: Uuid, intent: Intent<Uuid>) -> Option<FilterOp> {
    let source = editor.document().layer(&layer)?;
    let layer = LayerRef::Id(layer);
    let reference = |id| {
        source
            .filters
            .iter()
            .find(|f| f.id() == id)
            .map(|_| FilterRef::Id(id))
    };
    Some(match intent {
        Intent::Add(kind) if source.filters.len() < 256 => FilterOp::Add {
            layer,
            alias: "studio_filter".into(),
            descriptor: Parameters::new(kind).descriptor().ok()?,
            enabled: true,
            to_index: None,
        },
        Intent::Edit(id) => FilterOp::Update {
            layer,
            filter: reference(id)?,
            descriptor: source.filters.iter().find(|f| f.id() == id)?.descriptor(),
        },
        Intent::SetEnabled { id, enabled } => FilterOp::SetEnabled {
            layer,
            filter: reference(id)?,
            enabled,
        },
        Intent::Reorder { id, to_index } if to_index < source.filters.len() => FilterOp::Reorder {
            layer,
            filter: reference(id)?,
            to_index,
        },
        Intent::Duplicate(id) if source.filters.len() < 256 => FilterOp::Duplicate {
            layer,
            filter: reference(id)?,
            alias: "studio_filter".into(),
            to_index: None,
        },
        Intent::Remove(id) => FilterOp::Remove {
            layer,
            filter: reference(id)?,
        },
        _ => return None,
    })
}

#[cfg(test)]
#[path = "filter_editing/navigation_tests.rs"]
mod navigation_tests;
#[cfg(test)]
#[path = "filter_editing/tests.rs"]
mod tests;
