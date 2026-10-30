//! One immutable source is pinned throughout a floating selection draft.
use super::*;
use paintcraft_editor::session::Ticket;
use paintcraft_input::PointerButton;
use paintcraft_tools::{FloatingOptions, FloatingSelection};

#[derive(Debug)]
pub(super) struct Source {
    ticket: Ticket,
    document: Arc<Document>,
    target: Uuid,
    bounds: paintcraft_core::types::Rect,
}
impl Editor {
    /// Queue one activation, deduplicating before resolving private drafts.
    pub fn request(&mut self, token: Token, action: Action) -> bool {
        if self.outbox.has_seen(token) {
            return false;
        }
        if let Action::Filter(edit) = &action
            && let Err(error) = self.validate_filter_edit(edit)
        {
            self.status = error.to_string();
            return false;
        }
        if self.filter_edit_open
            && !matches!(&action, Action::Filter(edit) if self.filter_request_allowed(token, edit))
        {
            return false;
        }
        if self.has_image_paste() {
            if matches!(action, Action::Undo) {
                self.cancel_image_paste();
            } else {
                self.status = "Finish placing the pasted image before editing or saving.".into();
            }
            self.outbox.remember(token);
            return false;
        }
        if !self.before_floating_action(&action) {
            self.outbox.remember(token);
            return false;
        }
        if let Action::Stroke(stroke) = &action
            && !self.admit_paint(stroke)
        {
            return false;
        }
        // Typed text is part of the document the user is about to undo, save or change.
        if !matches!(action, Action::Text { .. } | Action::PickColor(_)) {
            self.commit_text();
        }
        let accepted = self
            .outbox
            .push(token, action, self.native_activation.clone());
        if accepted {
            self.preview_queue_revision.advance();
        }
        accepted
    }

    pub(crate) fn set_floating_commit_on_release(&mut self, value: bool) {
        self.floating_commit_on_release = value;
    }
    pub fn has_floating_selection(&self) -> bool {
        self.tools.floating_selection().is_some()
    }
    pub(super) fn floating_source_current(&self) -> bool {
        self.floating_source.as_ref().is_some_and(|source| {
            source.ticket == self.session.ticket()
                && Arc::ptr_eq(&source.document, &self.document_snapshot())
                && self.target == Some(source.target)
                && self.tools.tool == Tool::Select
                && self
                    .document()
                    .layers
                    .get(&source.target)
                    .is_some_and(|layer| layer.visible)
        })
    }
    pub(super) fn guard_floating_source(&mut self) {
        if self.floating_source.is_some()
            && (!self.floating_source_current() || !self.has_floating_selection())
        {
            self.tools.cancel_gesture();
            self.floating_source = None;
        }
    }
    pub(super) fn floating_preview(&self) -> Option<Preview> {
        if self.has_floating_selection() && !self.floating_source_current() {
            None
        } else {
            self.tools.preview()
        }
    }
    /// The field used for overlays is disposable and never replaces session coverage.
    pub fn display_selection(&self) -> Selection {
        if self.image_paste_revision().is_some() {
            return self
                .image_paste_bounds()
                .map_or(Selection::Empty, Selection::Rect);
        }
        if let Some(floating) = self
            .tools
            .floating_selection()
            .filter(|_| self.floating_source_current())
            && let Some(source) = &self.floating_source
        {
            return self
                .moved_selection_field(
                    floating.source_selection(),
                    source.bounds,
                    floating.offset(),
                )
                .unwrap_or(Selection::Inactive);
        }
        self.pending_move_selection()
            .unwrap_or_else(|| self.selection())
    }
    pub(super) fn begin_floating(&mut self, at: (i64, i64), button: PointerButton) -> bool {
        self.guard_floating_source();
        if self.tool != Tool::Select || button != PointerButton::Primary {
            return false;
        }
        let Some(layer) = self.target.filter(|id| {
            self.document()
                .layers
                .get(id)
                .is_some_and(|layer| layer.visible)
        }) else {
            return false;
        };
        let selection = self.selection();
        let bounds = if let Some(source) = &self.floating_source {
            source.bounds
        } else {
            if !FloatingSelection::hit_test(&selection, at) || self.has_pending_edits() {
                return false;
            }
            let canvas = self.document().canvas_rect();
            match &selection {
                Selection::Rect(rect) => match rect.intersect(&canvas) {
                    Some(r) => r,
                    None => return false,
                },
                Selection::Mask(mask) => match mask.bounds_checked(
                    canvas,
                    paintcraft_raster::selection::BOUNDS_CACHE_WORK,
                    &|| Ok(()),
                ) {
                    Ok(Some(r)) => r,
                    Ok(None) => return false,
                    Err(_) => {
                        self.set_status("Selection bounds unavailable");
                        return true;
                    }
                },
                _ => return false,
            }
        };
        let options = FloatingOptions {
            layer,
            selection,
            bounds,
            clear: self.options.eraser_color,
            transparent_key: self.options.transparent_selection.then_some(self.secondary),
            token: 0,
            commit_on_release: self.floating_commit_on_release,
        };
        let existed = self.has_floating_selection();
        match self.tools.begin_selection_move(options, at, button) {
            Ok(true) => {
                if self.floating_source.is_none() {
                    self.floating_source = Some(Source {
                        ticket: self.session.ticket(),
                        document: self.document_snapshot(),
                        target: layer,
                        bounds,
                    });
                }
                true
            }
            Ok(false) if existed => {
                self.finish_selection_move();
                true
            }
            Ok(false) => false,
            Err(error) => {
                self.cancel_gesture();
                self.status = error.to_string();
                true
            }
        }
    }
    pub fn finish_selection_move(&mut self) {
        if !self.has_floating_selection() {
            self.floating_source = None;
            return;
        }
        if !self.floating_source_current() {
            self.cancel_gesture();
            return;
        }
        if let Some((token, action)) = self.tools.finish_selection_move() {
            self.request(token, action);
        } else if !self.has_floating_selection() {
            self.floating_source = None;
        }
    }
    /// Called before accepting another action. Undo cancels a private draft first.
    pub(super) fn before_floating_action(&mut self, action: &Action) -> bool {
        if matches!(action, Action::MoveSelection { .. }) {
            if self.floating_source.is_some() {
                let current = self.floating_source_current();
                self.floating_source = None;
                if !current {
                    self.tools.cancel_gesture();
                    return false;
                }
            }
        } else if self.has_floating_selection() {
            if matches!(action, Action::Undo | Action::Redo) {
                self.cancel_gesture();
                return false;
            }
            self.finish_selection_move();
        }
        true
    }
    pub(super) fn move_selection(
        &mut self,
        layer: Uuid,
        offset: (i64, i64),
        clear: Rgba,
        key: Option<Rgba>,
    ) {
        if self.editing.is_some() {
            self.start_tool_plan(
                Preview::MoveSelection {
                    layer,
                    offset,
                    clear,
                    transparent_key: key,
                },
                "Move selection",
            );
        } else {
            match raster_commands::move_selection(layer, offset, clear, key) {
                Ok(operation) => self.draw(vec![operation], "Move selection"),
                Err(error) => self.status = error.to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests;
