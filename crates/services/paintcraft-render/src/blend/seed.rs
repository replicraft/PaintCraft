//! Only fresh Source8 rooted output can consume this private structural proof.
use super::{limit, missing};
use paintcraft_core::types::{Error, ErrorCode, LayerBlendMode as Mode, Rect, Result};
use paintcraft_document::Document;
pub(crate) const CONTROL: u64 = 2048;
#[repr(C)]
pub(crate) struct Seed<'a> {
    region: Rect,
    doc: &'a Document,
    index: usize,
}
pub(crate) fn future_work(doc: &Document) -> Result<u64> {
    (doc.roots.len() as u64)
        .checked_mul(6144)
        .and_then(|n| n.checked_add(64 + 2 * CONTROL + 128))
        .ok_or_else(limit)
}
impl<'a> Seed<'a> {
    pub(crate) fn require_matches(&self, doc: &Document, region: Rect) -> Result<()> {
        if !std::ptr::eq(self.doc, doc) || self.region != region {
            return Err(Error::new(
                ErrorCode::InvariantViolation,
                "Seed proof binding differs",
            ));
        }
        Ok(())
    }

    pub(crate) fn capture(
        doc: &'a Document,
        region: Rect,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<Self> {
        spend(
            (doc.roots.len() as u64)
                .checked_mul(6144)
                .ok_or_else(limit)?,
        )?;
        check()?;
        let mut index = usize::MAX;
        for (i, id) in doc.roots.iter().enumerate() {
            check()?;
            let layer = doc.layers.get(id).ok_or_else(missing)?;
            if layer.visible && layer.opacity != 0 && region.intersect(&layer.extent()).is_some() {
                // O(1) source facts; Normal zero alpha over fresh zero output
                // writes nothing, including hiddenRGB. Do not skip General or
                // any stored filter/style/mask whose work/evaluation must remain.
                if layer.blend_mode == Mode::Normal
                    && layer.filters.is_empty()
                    && !crate::styles::stored(layer)
                    && layer.mask.is_none()
                    && layer.raster.resident_tiles() == 0
                    && layer.raster.default_pixel().a() == 0
                {
                    continue;
                }
                if layer.blend_mode != Mode::Normal {
                    index = i;
                }
                break;
            }
        }
        check()?;
        Ok(Self { doc, region, index })
    }
    pub(crate) fn samples(&self) -> Result<u64> {
        if self.index == usize::MAX {
            return Ok(0);
        }
        let layer = self
            .doc
            .layers
            .get(&self.doc.roots[self.index])
            .ok_or_else(missing)?;
        u64::try_from(
            self.region
                .intersect(&layer.extent())
                .ok_or_else(missing)?
                .area(),
        )
        .map_err(|_| limit())
    }
    pub(crate) fn document(&self) -> &Document {
        self.doc
    }
    pub(crate) fn region(&self) -> Rect {
        self.region
    }
}
#[path = "seed/compose.rs"]
mod compose;
pub(crate) use compose::fresh_output;
#[cfg(test)]
#[path = "seed/tests.rs"]
mod tests;
