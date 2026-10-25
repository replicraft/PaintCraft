//! Raw active-layer selection capture. OS and asynchronous ownership stay in hosts.
use crate::session::{HistoryTicket, Session};
use paintcraft_commands::Batch;
use paintcraft_core::types::clipboard::Image;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba, json::Value};
use paintcraft_document::Document;
use paintcraft_raster::selection::{BOUNDS_CACHE_WORK, Selection};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Capture {
    pub image: Image,
    pub origin: (i64, i64),
    pub selection: Selection,
}

pub fn capture(
    document: &Document,
    selection: Selection,
    target: Uuid,
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<Capture>> {
    check()?;
    let layer = document.layer(&target).ok_or_else(|| {
        Error::new(
            ErrorCode::InputSchema,
            "Clipboard target layer does not exist",
        )
    })?;
    let canvas = document.canvas_rect();
    let bounds = match &selection {
        Selection::Inactive => Some(canvas),
        Selection::Empty => None,
        Selection::Rect(area) => area.intersect(&canvas),
        Selection::Mask(mask) => mask.bounds_checked(canvas, BOUNDS_CACHE_WORK, check)?,
    };
    let Some(bounds) = bounds else {
        return Ok(None);
    };
    let extent = layer.extent();
    let image = Image::from_fn(
        bounds.width() as u32,
        bounds.height() as u32,
        budget,
        check,
        |x, y| {
            let x = bounds.x0 + i64::from(x);
            let y = bounds.y0 + i64::from(y);
            if matches!(&selection, Selection::Mask(mask) if mask.coverage(x,y) == 0)
                || !extent.contains(x, y)
            {
                Ok(Rgba::TRANSPARENT)
            } else {
                Ok(layer.raster.get(x - layer.origin.0, y - layer.origin.1))
            }
        },
    )?;
    Ok(Some(Capture {
        image,
        origin: (bounds.x0, bounds.y0),
        selection,
    }))
}

#[cfg(test)]
mod tests;

/// Source-bound authorization for Cut's removal transaction. Creating this value
/// does not authorize deletion: the host must first acknowledge a clipboard write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CutSource {
    ticket: HistoryTicket,
    selection: Selection,
    target: Uuid,
    clear: Rgba,
}
impl CutSource {
    pub fn new(session: &Session, target: Uuid, clear: Rgba) -> Result<Self> {
        if !session
            .document()
            .layer(&target)
            .is_some_and(|layer| layer.visible)
        {
            return Err(Error::new(
                ErrorCode::ReferenceUnknownId,
                "Cut requires a visible target layer",
            ));
        }
        if matches!(session.selection(), Selection::Empty) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Cut requires selected pixels",
            ));
        }
        Ok(Self {
            ticket: session.history_ticket(),
            selection: session.selection(),
            target,
            clear,
        })
    }

    /// Validate owner, epoch, selected field and current target before building a
    /// batch whose ordinary publication guard remains bound to that same source.
    pub fn removal_batch(&self, session: &Session, target: Option<Uuid>) -> Result<Batch> {
        if session.history_ticket() != self.ticket
            || target != Some(self.target)
            || session.selection() != self.selection
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Cut source changed before clipboard acknowledgment",
            ));
        }
        Batch::from_value(&Value::object([
            ("schema", Value::str("paintcraft.batch/v2")),
            ("atomic", Value::Bool(true)),
            ("policy", Value::str(paintcraft_document::POLICY)),
            (
                "base",
                Value::object([
                    (
                        "state_id",
                        Value::str(self.ticket.base.state_id.to_string()),
                    ),
                    (
                        "publication_epoch",
                        Value::str(self.ticket.base.publication_epoch.to_string()),
                    ),
                ]),
            ),
            (
                "operations",
                Value::Array(vec![Value::object([
                    ("op", Value::str("raster.cut_selection")),
                    (
                        "layer_ref",
                        Value::object([("id", Value::str(self.target.to_string()))]),
                    ),
                    ("clear", Value::str(self.clear.to_hex())),
                ])]),
            ),
        ]))
    }
}

#[cfg(test)]
mod cut_tests;
