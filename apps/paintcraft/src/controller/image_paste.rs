//! Frozen external-image placement; pointer releases retain the private draft.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Rect, policy::BlendMode};
use paintcraft_editor::{clipboard::PasteDraft, session::Preparation};
use paintcraft_input::PointerButton;
use paintcraft_tools::{ImagePlacement, PreviewRevision};

#[derive(Debug)]
pub(super) struct Placing {
    position: ImagePlacement,
    draft: PasteDraft,
    target: Uuid,
    key: Option<Rgba>,
    document: Arc<Document>,
    committing: bool,
}

pub(crate) struct Captured {
    source: Result<Preparation>,
    draft: PasteDraft,
    target: Uuid,
    key: Option<Rgba>,
    canvas: (u32, u32),
}
impl Captured {
    pub(crate) fn build(self, check: &dyn Fn() -> Result<()>) -> Result<Document> {
        let mut doc = self.source?.preview_paste_image_onto(
            &self.draft,
            self.target,
            BlendMode::SourceOver,
            self.key,
            paintcraft_raster::Admission::new(paintcraft_core::types::policy::STAGED_PIXELS_MAX),
            check,
        )?;
        // Growth is staged with placement. Keep the current canvas while dragging.
        doc.canvas = self.canvas;
        Ok(doc)
    }
}
impl Editor {
    fn image_paste_current(&self) -> bool {
        self.image_paste.as_ref().is_some_and(|p| {
            p.draft.source() == self.session.history_ticket()
                && Arc::ptr_eq(&p.document, &self.document_snapshot())
                && self.target == Some(p.target)
                && self.tool == Tool::Select
                && self
                    .document()
                    .layers
                    .get(&p.target)
                    .is_some_and(|layer| layer.visible)
        })
    }
    pub fn has_image_paste(&self) -> bool {
        self.image_paste
            .as_ref()
            .is_some_and(|p| !p.committing || p.draft.source() == self.session.history_ticket())
    }
    pub(super) fn guard_image_paste(&mut self) {
        if self.image_paste.is_some() && !self.image_paste_current() {
            let published = self.image_paste.as_ref().is_some_and(|p| p.committing);
            self.image_paste = None;
            self.preview_lineage = Uuid::new_v4();
            if !published {
                self.status = "The document changed; paste again.".into();
            }
        } else if !self.has_pending_edits()
            && let Some(p) = &mut self.image_paste
        {
            // A failed detached publication leaves the position correctable.
            p.committing = false;
        }
    }
    pub fn begin_image_paste(&mut self, draft: PasteDraft, key: Option<Rgba>) -> Result<()> {
        if self.has_pending_edits()
            || self.file_barrier.is_some()
            || self.gesture_in_progress()
            || self.is_editing_text()
            || self.has_floating_selection()
            || self.has_image_paste()
            || draft.source() != self.session.history_ticket()
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Finish the current edit before pasting.",
            ));
        }
        let target = self
            .target
            .filter(|id| self.document().layers.get(id).is_some_and(|l| l.visible))
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ReferenceUnknownId,
                    "Paste requires a visible target layer",
                )
            })?;
        let position = ImagePlacement::new(draft.image().clone(), draft.origin())?;
        self.select_tool(Tool::Select);
        self.preview_lineage = Uuid::new_v4();
        self.image_paste = Some(Placing {
            position,
            draft,
            target,
            key,
            document: self.document_snapshot(),
            committing: false,
        });
        self.status = "Drag to position; Enter places, Escape cancels.".into();
        Ok(())
    }
    pub(super) fn finish_image_paste_publication(&mut self, target_alias: Option<&str>) {
        // A no-change receipt deliberately has no aliases. The accepted request's
        // destination still identifies its completion, without reading status text.
        if target_alias == Some("pasted") && self.image_paste.as_ref().is_some_and(|p| p.committing)
        {
            self.image_paste = None;
            self.preview_lineage = Uuid::new_v4();
        }
    }
    pub fn cancel_image_paste(&mut self) {
        if let Some(p) = self.image_paste.take() {
            if p.committing {
                self.cancel_pending_edits();
            }
            self.preview_lineage = Uuid::new_v4();
            self.status = "Paste cancelled.".into();
        }
    }
    pub fn place_image_paste(&mut self) -> Result<()> {
        if !self.image_paste_current() {
            self.guard_image_paste();
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "The document changed; paste again.",
            ));
        }
        if self.image_paste.as_ref().is_some_and(|p| p.committing) {
            return Ok(());
        }
        let mut p = self.image_paste.take().expect("current paste");
        p.position.interrupt_drag();
        match self.start_paste_onto(p.draft.clone(), p.target, BlendMode::SourceOver, p.key) {
            Ok(()) => {
                if self.has_pending_edits() {
                    p.committing = true;
                    self.image_paste = Some(p);
                }
                Ok(())
            }
            Err(error) => {
                self.image_paste = Some(p);
                Err(error)
            }
        }
    }
    pub fn image_paste_bounds(&self) -> Option<Rect> {
        self.image_paste
            .as_ref()
            .filter(|_| self.image_paste_current())
            .and_then(|p| {
                p.position
                    .bounds()
                    .intersect(&self.document().canvas_rect())
            })
    }
    pub(crate) fn image_paste_revision(&self) -> Option<PreviewRevision> {
        self.image_paste
            .as_ref()
            .filter(|_| self.image_paste_current())
            .map(|p| p.position.revision())
    }
    pub(crate) fn capture_image_paste(&self) -> Option<Captured> {
        self.image_paste
            .as_ref()
            .filter(|_| self.image_paste_current())
            .map(|p| Captured {
                source: self.session.preparation(),
                draft: p.draft.clone(),
                target: p.target,
                key: p.key,
                canvas: self.document().canvas,
            })
    }
    fn move_image_paste(
        &mut self,
        change: impl FnOnce(&mut ImagePlacement) -> Result<()>,
    ) -> Result<()> {
        self.guard_image_paste();
        let Some(p) = &mut self.image_paste else {
            return Ok(());
        };
        if p.committing {
            return Ok(());
        }
        let previous = p.position.clone();
        let result = change(&mut p.position)
            .and_then(|()| p.draft.place(p.position.origin(), p.draft.mode()));
        if result.is_err() {
            p.position = previous;
        }
        result
    }
    pub fn nudge_image_paste(&mut self, delta: (i64, i64)) -> Result<()> {
        self.move_image_paste(|p| {
            let at = p.origin();
            let x = at.0.checked_add(delta.0);
            let y = at.1.checked_add(delta.1);
            match (x, y) {
                (Some(x), Some(y)) => p.place((x, y)),
                _ => Err(Error::new(ErrorCode::InputSchema, "Invalid paste position")),
            }
        })
    }
    pub(super) fn image_paste_press(&mut self, point: (i64, i64), button: PointerButton) -> bool {
        self.guard_image_paste();
        let Some(p) = &mut self.image_paste else {
            return false;
        };
        if !p.committing {
            p.position.press(point, button);
        }
        true
    }
    pub(super) fn image_paste_move(&mut self, point: (i64, i64)) -> bool {
        if !self.has_image_paste() {
            return false;
        }
        if let Err(error) = self.move_image_paste(|p| p.move_to(point)) {
            self.status = error.message;
        }
        true
    }
    pub(super) fn image_paste_dragging(&self) -> bool {
        self.image_paste
            .as_ref()
            .is_some_and(|p| p.position.is_dragging())
    }
    pub(super) fn image_paste_release(&mut self, button: PointerButton) -> bool {
        self.guard_image_paste();
        let Some(p) = &mut self.image_paste else {
            return false;
        };
        if button == PointerButton::Primary {
            p.position.interrupt_drag();
        }
        true
    }
    pub(super) fn interrupt_image_paste(&mut self) {
        if let Some(p) = &mut self.image_paste {
            p.position.interrupt_drag();
        }
    }
}

#[cfg(test)]
mod tests;
