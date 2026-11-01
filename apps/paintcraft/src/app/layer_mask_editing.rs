//! Four immediate Studio actions wait for the accepted prefix, then freeze a ticket.
use super::*;
use paintcraft_commands::{LayerMaskOp, LayerRef};
use paintcraft_studio::layer_masks::{self, Action as Intent, SelectionState, Summary};
use paintcraft_ui::t;
#[derive(Debug)]
pub(super) struct State {
    tab: Uuid,
    owner: Uuid,
    layer: Uuid,
    intent: Intent,
}
impl App {
    fn layer_mask_private_input(&self) -> bool {
        self.ime_composing
            || self.editor.gesture_in_progress()
            || self.editor.is_editing_text()
            || self.editor.has_image_paste()
            || self.editor.has_floating_selection()
            || self.editor.is_constructing_shape()
            || self.filter_editing.is_some()
    }
    pub(super) fn studio_layer_mask_panel(&mut self, ui: &mut egui::Ui) {
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
        let mask = layer.mask.as_ref().map(|m| {
            let s = m.coverage().stats();
            Summary {
                enabled: m.enabled(),
                domain: m.coverage().domain(),
                bands: s.bands,
                runs: s.runs,
            }
        });
        let selection = match self.editor.selection() {
            Selection::Inactive => SelectionState::Inactive,
            Selection::Empty => SelectionState::Empty,
            _ => SelectionState::Active,
        };
        let available = self.editor.layer_mask_worker_available()
            && self.confirm.is_none()
            && self.layer_mask_editing.is_none()
            && !self.layer_mask_private_input();
        for action in layer_masks::panel(
            ui,
            layer_masks::Context {
                available,
                selection,
                mask,
            },
        ) {
            self.begin_layer_mask_action(ui.ctx(), action);
        }
        ui.separator();
    }
    pub(super) fn begin_layer_mask_action(&mut self, ctx: &egui::Context, intent: Intent) {
        if self.experience() != Experience::Studio
            || self.confirm.is_some()
            || self.layer_mask_editing.is_some()
            || self.document_count() == 0
            || self.layer_mask_private_input()
        {
            return;
        }
        if !self.editor.layer_mask_worker_available() {
            self.editor
                .set_status("Edit failed: Layer masks require the editing worker");
            return;
        }
        let Some(layer) = self.editor.target() else {
            return;
        };
        self.layer_mask_editing = Some(State {
            tab: self.active_tab_id(),
            owner: self.editor.session_owner_id(),
            layer,
            intent,
        });
        self.confirm = Some(Confirm::LayerMask);
        self.pump_layer_mask_action(ctx);
    }
    pub(super) fn pump_layer_mask_action(&mut self, ctx: &egui::Context) {
        let Some(state) = self.layer_mask_editing.take() else {
            return;
        };
        if self.confirm != Some(Confirm::LayerMask)
            || state.tab != self.active_tab_id()
            || state.owner != self.editor.session_owner_id()
            || self.experience() != Experience::Studio
            || self.editor.target() != Some(state.layer)
            || self.editor.document().layer(&state.layer).is_none()
            || self.layer_mask_private_input()
            || self.editor.failed_text_retained()
        {
            if self.confirm == Some(Confirm::LayerMask) {
                self.confirm = None;
            }
            return;
        }
        if self.editor.has_pending_edits() || !self.deferred_files.is_empty() {
            self.layer_mask_editing = Some(state);
            ctx.request_repaint();
            return;
        }
        let layer = LayerRef::Id(state.layer);
        let operation = match state.intent {
            Intent::UseSelection => LayerMaskOp::FromSelection {
                layer,
                enabled: true,
            },
            Intent::SetEnabled(enabled) => LayerMaskOp::SetEnabled { layer, enabled },
            Intent::Invert => LayerMaskOp::Invert { layer },
            Intent::Remove => LayerMaskOp::Remove { layer },
        };
        let edit = self.editor.layer_mask_capture(operation);
        self.confirm = None;
        self.editor
            .request(token(ctx, 0x4c415945524d4153), Action::LayerMask(edit));
    }
    pub(super) fn layer_mask_wait_dialog(&mut self, ctx: &egui::Context) {
        self.pump_layer_mask_action(ctx);
        if self.layer_mask_editing.is_none() {
            return;
        }
        let modal = egui::Modal::new(egui::Id::unique("layer-mask-prefix-wait")).show(ctx, |ui| {
            ui.spinner();
            ui.button(t!(ui.ctx(), "Cancel")).clicked()
        });
        if modal.inner || modal.should_close() {
            self.layer_mask_editing = None;
            self.confirm = None;
        }
    }
}
#[cfg(test)]
#[path = "layer_mask_editing/tests.rs"]
mod tests;
