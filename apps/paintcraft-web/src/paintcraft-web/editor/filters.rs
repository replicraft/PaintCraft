//! Captured filter edits never resolve or discard another private tool draft.
use super::*;
use paintcraft_commands::{FilterOp, FilterRef, LayerRef};
use paintcraft_tools::FilterEdit;

impl Editor {
    pub(crate) fn filter_operation(&self, edit: &FilterEdit) -> Result<Value> {
        let ticket = self.session.ticket();
        if !edit.matches_base(
            self.session.owner_id(),
            ticket.state_id,
            ticket.publication_epoch,
        ) {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Filter edit source changed",
            ));
        }
        if self.tools.is_editing_text()
            || self.tools.gesture_in_progress()
            || self.floating_source.is_some()
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Finish or cancel the private tool draft before editing filters",
            ));
        }
        let (layer, filter) = match &edit.operation {
            FilterOp::Add { layer, .. } => (layer, None),
            FilterOp::Update { layer, filter, .. }
            | FilterOp::SetEnabled { layer, filter, .. }
            | FilterOp::Reorder { layer, filter, .. }
            | FilterOp::Remove { layer, filter }
            | FilterOp::Duplicate { layer, filter, .. } => (layer, Some(filter)),
        };
        let LayerRef::Id(layer) = layer else {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Captured filter edits require a layer UUID",
            ));
        };
        let layer = self.session.document().layers.get(layer).ok_or_else(|| {
            Error::new(ErrorCode::ReferenceUnknownId, "Filter layer is unavailable")
        })?;
        if let Some(filter) = filter {
            let FilterRef::Id(id) = filter else {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "Captured filter edits require a filter UUID",
                ));
            };
            if !layer.filters.iter().any(|filter| filter.id() == *id) {
                return Err(Error::new(
                    ErrorCode::ReferenceUnknownId,
                    "Filter is unavailable",
                ));
            }
        }
        edit.operation.to_value()
    }
}

#[cfg(test)]
mod tests;
