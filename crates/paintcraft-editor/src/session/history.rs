//! Guarded navigation of recorded history, without re-executing commands.
use super::*;
use paintcraft_commands::Op;
use paintcraft_history::{EntryInfo, Label};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryTicket {
    pub owner: Uuid,
    pub base: Ticket,
}

#[derive(Clone, Debug)]
pub struct HistoryView {
    pub ticket: HistoryTicket,
    pub entries: Vec<EntryInfo>,
    pub trimmed_total: u64,
}

impl Session {
    /// Metadata only: does not clone documents, retain tiles, or modify history.
    pub fn history_view(&self) -> HistoryView {
        HistoryView {
            ticket: HistoryTicket {
                owner: self.owner,
                base: self.ticket(),
            },
            entries: self.history.entries().collect(),
            trimmed_total: self.history.trimmed_total(),
        }
    }

    /// Publish an exact retained snapshot once. Invalid targets, stale views,
    /// and clicks from another session leave document, selection and cursor intact.
    pub fn navigate_history(
        &mut self,
        ticket: HistoryTicket,
        target: Option<Uuid>,
    ) -> Result<Option<Navigation>> {
        if ticket.owner != self.owner || ticket.base != self.ticket() {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "History view is no longer current",
            ));
        }
        let (snapshot, changed) = self.history.seek(target).ok_or_else(|| {
            Error::new(
                ErrorCode::ReferenceUnknownId,
                "History entry is no longer retained",
            )
        })?;
        Ok(changed.then(|| self.navigate(snapshot)))
    }
}

pub(super) fn label(batch: &Batch) -> Label {
    if matches!(batch.operations.first(), Some(Op::CanvasResize { .. }))
        && batch.operations[1..]
            .iter()
            .all(|op| matches!(op, Op::LayerExpand { .. }))
    {
        return Label::ResizeCanvas;
    }
    if batch.operations.len() != 1 {
        return Label::Batch;
    }
    match &batch.operations[0] {
        Op::LayerAdd { .. } => Label::AddLayer,
        Op::LayerDuplicate { .. } => Label::DuplicateLayer,
        Op::LayerDelete { .. } => Label::DeleteLayer,
        Op::LayerSetProperties { .. } => Label::LayerProperties,
        Op::LayerReorder { .. } => Label::ReorderLayer,
        Op::FillRect { .. } | Op::FloodFill { .. } => Label::Fill,
        Op::PencilPath { .. } => Label::Pencil,
        Op::StrokePath {
            color,
            mode,
            size,
            round,
            ..
        } => {
            if *mode == paintcraft_commands::Mode::Replace && color.a() == 0 {
                Label::Erase
            } else if *size == 1 && !round {
                Label::Pencil
            } else {
                Label::Brush
            }
        }
        Op::SelectionSetRect { .. } | Op::SelectionClear => Label::Selection,
        Op::Shape { .. } => Label::Shape,
        Op::Text { .. } => Label::Text,
        Op::CanvasResize { .. } | Op::LayerExpand { .. } => Label::ResizeCanvas,
    }
}
