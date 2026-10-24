//! Raw active-layer selection capture. OS and asynchronous ownership stay in hosts.
use paintcraft_core::types::clipboard::Image;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba};
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
