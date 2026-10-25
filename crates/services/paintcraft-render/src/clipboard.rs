//! Selection-clipped visible-image capture using the canonical ordered compositor.
use super::*;
use paintcraft_core::types::{clipboard::Image, resources::SharedBudget};
use paintcraft_raster::selection::{BOUNDS_CACHE_WORK, Selection};

/// Return exact tight canvas-clipped pixels and their document-space origin.
/// The output allocation is admitted before any composition and shared by clones.
pub fn capture_visible(
    document: &Document,
    selection: &Selection,
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<(Image, (i64, i64))>> {
    check()?;
    let canvas = document.canvas_rect();
    let bounds = match selection {
        Selection::Inactive => Some(canvas),
        Selection::Empty => None,
        Selection::Rect(area) => area.intersect(&canvas),
        Selection::Mask(mask) => mask.bounds_checked(canvas, BOUNDS_CACHE_WORK, check)?,
    };
    let Some(bounds) = bounds else {
        return Ok(None);
    };
    let image = Image::from_writer(
        bounds.width() as u32,
        bounds.height() as u32,
        budget,
        check,
        |pixels| {
            compose_roots_into(document, bounds, 0..document.roots.len(), pixels, &check)?;
            if let Selection::Mask(mask) = selection {
                for (y, row) in pixels
                    .chunks_exact_mut(bounds.width() as usize * 4)
                    .enumerate()
                {
                    for (x, pixel) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                        if x % 64 == 0 {
                            check()?;
                        }
                        if mask.coverage(bounds.x0 + x as i64, bounds.y0 + y as i64) == 0 {
                            pixel.fill(0);
                        }
                    }
                }
            }
            Ok(())
        },
    )?;
    Ok(Some((image, (bounds.x0, bounds.y0))))
}

#[cfg(test)]
mod tests;
