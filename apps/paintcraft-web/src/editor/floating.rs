//! A floating browser drag always samples the immutable revision captured at press.
use super::*;
use paintcraft_editor::session::Ticket;
use paintcraft_raster::Selection;
use paintcraft_tools::{FloatingOptions, PointerButton, Tool};
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct Source {
    ticket: Ticket,
    document: Arc<Document>,
    selection: Selection,
    bounds: paintcraft_core::types::Rect,
    layer: Uuid,
}
impl Editor {
    fn source_is_current(&self, source: &Source) -> bool {
        source.ticket == self.session.ticket()
            && Arc::ptr_eq(&source.document, &self.session.snapshot())
            && self.target == Some(source.layer)
            && self.tools.tool == Tool::Select
            && self.session.selection() == source.selection
    }
    pub(crate) fn validate_floating(&mut self) {
        if self
            .floating_source
            .as_ref()
            .is_some_and(|source| !self.source_is_current(source))
            || (self.floating_source.is_some() && self.tools.floating_selection().is_none())
        {
            self.cancel_floating();
        }
    }
    pub(crate) fn cancel_floating(&mut self) {
        if self.floating_source.take().is_some() || self.tools.floating_selection().is_some() {
            self.tools.cancel_gesture();
        }
    }
    pub(crate) fn begin_floating(
        &mut self,
        bounds: paintcraft_core::types::Rect,
        at: (i64, i64),
        button: PointerButton,
    ) -> Result<bool> {
        self.validate_floating();
        if self.tools.tool != Tool::Select
            || self.tools.gesture_in_progress()
            || self.tools.is_editing_text()
        {
            return Ok(false);
        }
        let Some(layer) = self
            .target
            .filter(|id| self.session.document().layers.contains_key(id))
        else {
            return Ok(false);
        };
        let selection = self.session.selection();
        let source = Source {
            ticket: self.session.ticket(),
            document: self.session.snapshot(),
            selection: selection.clone(),
            bounds,
            layer,
        };
        let started = self.tools.begin_selection_move(
            FloatingOptions {
                layer,
                selection,
                bounds,
                clear: WHITE,
                transparent_key: self
                    .tools
                    .options
                    .transparent_selection
                    .then_some(self.tools.secondary),
                token: 0,
                commit_on_release: true,
            },
            at,
            button,
        )?;
        if started {
            self.floating_source = Some(source);
        }
        Ok(started)
    }
    fn captured_move(&self, layer: Uuid) -> Result<&Source> {
        self.floating_source
            .as_ref()
            .filter(|source| source.layer == layer && self.source_is_current(source))
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ConflictStaleBase,
                    "selection move cancelled: source, tool or target changed",
                )
            })
    }
    pub(super) fn preview_selection_move(
        &self,
        layer: Uuid,
        offset: (i64, i64),
        clear: Rgba,
        key: Option<Rgba>,
    ) -> Result<Option<Document>> {
        let source = self.captured_move(layer)?;
        let batch = self.batch(vec![commands::move_selection(layer, offset, clear, key)?])?;
        Ok(Some(
            execute::execute_with_budget(
                &source.document,
                source.selection.clone(),
                &batch,
                MAX_PIXELS * 4,
            )?
            .doc,
        ))
    }
    pub(super) fn commit_selection_move(
        &mut self,
        layer: Uuid,
        offset: (i64, i64),
        clear: Rgba,
        key: Option<Rgba>,
    ) -> Result<()> {
        if let Err(error) = self.captured_move(layer) {
            self.cancel_floating();
            return Err(error);
        }
        let result = commands::move_selection(layer, offset, clear, key)
            .and_then(|operation| self.commit(vec![operation]));
        self.cancel_floating();
        result
    }
    /// Feedback moves the immutable field, without publishing the source session.
    pub(crate) fn display_selection(&self) -> Result<Selection> {
        let Some(floating) = self.tools.floating_selection() else {
            return Ok(self.session.selection());
        };
        let source = self.captured_move(self.target.unwrap_or(Uuid::nil()))?;
        let offset = floating.offset();
        if offset == (0, 0) {
            return Ok(source.selection.clone());
        }
        let canvas = source.document.canvas_rect();
        Ok(match &source.selection {
            Selection::Mask(mask) => mask
                .clipped(source.bounds)?
                .expect("nonempty captured field")
                .translated(offset.0, offset.1)?
                .clipped(canvas)?
                .map_or(Selection::Empty, Selection::Mask),
            Selection::Rect(rect) => rect
                .translated(offset.0, offset.1)
                .and_then(|rect| rect.intersect(&canvas))
                .map_or(Selection::Empty, Selection::Rect),
            selection => selection.clone(),
        })
    }
}
