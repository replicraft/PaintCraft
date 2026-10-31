//! Initial catalog export region facts; v5 methods and bytes stay separate.
use super::*;
impl SourceSpatialContext {
    pub fn capture_catalog(
        doc: &Document,
        base_payload: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let mut value = Self::default();
        for layer in doc.layers.values() {
            check()?;
            for filter in layer.filters.iter() {
                check()?;
                if filter.descriptor().is_spatial() {
                    value.stored = value.stored.checked_add(1).ok_or_else(limit)?;
                }
            }
        }
        let canvas =
            Rect::new(0, 0, i64::from(doc.canvas.0), i64::from(doc.canvas.1)).ok_or_else(limit)?;
        for layer in doc
            .layers_in_order()
            .filter(|l| l.visible && l.opacity != 0)
        {
            check()?;
            let Some(area) = layer.extent().intersect(&canvas) else {
                continue;
            };
            let local = area
                .translated(-layer.origin.0, -layer.origin.1)
                .ok_or_else(limit)?;
            let active = paintcraft_render::spatial::active(layer);
            if active {
                let count = layer
                    .filters
                    .iter()
                    .filter(|f| f.enabled() && f.descriptor().is_spatial())
                    .count();
                value.rendered = value.rendered.checked_add(count as u32).ok_or_else(limit)?;
            }
            let mut y = local.y0;
            while y < local.y1 {
                let height = 64.min(local.y1 - y);
                let mut x = local.x0;
                while x < local.x1 {
                    check()?;
                    let width = 64.min(local.x1 - x);
                    let tile = Rect::new(x, y, width, height).ok_or_else(limit)?;
                    let quote = paintcraft_render::spatial::quote_layer(layer, tile, check)?;
                    value.tiles = value.tiles.checked_add(quote.tiles).ok_or_else(limit)?;
                    // Point-only frames price uniform-proof plus dense fallback; spatial
                    // plans evaluate each stage once. Add final compositing coverage.
                    // Deterministic conservative units, never elapsed time.
                    let layer_work = if active {
                        quote.work
                    } else {
                        quote.work.checked_mul(2).ok_or_else(limit)?
                    };
                    value.work = value
                        .work
                        .checked_add(layer_work)
                        .and_then(|n| n.checked_add(tile.area() as u64))
                        .filter(|n| *n <= SPATIAL_WORK_MAX)
                        .ok_or_else(limit)?;
                    if active {
                        value.workspace = value.workspace.max(quote.bytes);
                    }
                    x += width;
                }
                y += height;
            }
        }
        value.payload = base_payload
            .checked_add(value.workspace)
            .ok_or_else(limit)?;
        value.validate_catalog(u64::MAX)?;
        check()?;
        Ok(value)
    }
    pub fn encode_catalog(self) -> [u8; 64] {
        let mut bytes = [0; 64];
        bytes[..2].copy_from_slice(&6u16.to_be_bytes());
        bytes[2..4].copy_from_slice(&1u16.to_be_bytes());
        bytes[4..8].copy_from_slice(&self.stored.to_be_bytes());
        bytes[8..12].copy_from_slice(&self.rendered.to_be_bytes());
        for (at, value) in [
            (16, self.tiles),
            (24, self.workspace),
            (32, self.work),
            (40, self.payload),
        ] {
            bytes[at..at + 8].copy_from_slice(&value.to_be_bytes());
        }
        bytes
    }
    pub fn decode_catalog(bytes: &[u8], cap: u64) -> Result<Self> {
        if bytes.len() != 64
            || bytes[..2] != 6u16.to_be_bytes()
            || bytes[2..4] != 1u16.to_be_bytes()
            || bytes[12..16] != [0; 4]
            || bytes[48..] != [0; 16]
        {
            return Err(bad("Spatial export framing/profile/reserved fields"));
        }
        let n = |at| u64::from_be_bytes(bytes[at..at + 8].try_into().unwrap());
        let value = Self {
            stored: u32::from_be_bytes(bytes[4..8].try_into().unwrap()),
            rendered: u32::from_be_bytes(bytes[8..12].try_into().unwrap()),
            tiles: n(16),
            workspace: n(24),
            work: n(32),
            payload: n(40),
        };
        value.validate_catalog(cap)?;
        Ok(value)
    }
    pub fn validate_catalog(self, cap: u64) -> Result<()> {
        if self.stored > 8 * 256
            || self.rendered > self.stored
            || (self.rendered == 0 && self.workspace != 0)
            || self.work > SPATIAL_WORK_MAX
            || self.payload < self.workspace
            || self.payload > cap
        {
            return Err(limit());
        }
        Ok(())
    }
    pub(crate) fn catalog_payload(self, payload: u64) -> Result<Self> {
        let value = Self { payload, ..self };
        value.validate_catalog(u64::MAX)?;
        Ok(value)
    }
    pub fn finalize_catalog_source(self, source_bytes: usize, cap: u64) -> Result<Self> {
        self.catalog_payload(
            self.payload
                .checked_add(source_bytes as u64)
                .ok_or_else(limit)?,
        )
        .and_then(|value| {
            value.validate_catalog(cap)?;
            Ok(value)
        })
    }
}
