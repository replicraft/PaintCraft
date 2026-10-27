//! Shared source inventory/raw serialization for admitted browser export jobs.
//! No codec settings, rendering, scheduling or public reader policy lives here.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    pub label: &'static str,
    pub side: u32,
    pub pixels: u64,
    pub layers: usize,
    pub layer_pixels: u64,
    pub resident: u64,
    pub metadata: u64,
    pub snapshot: usize,
}
#[derive(Clone, Debug)]
pub(crate) struct Plan {
    document: Uuid,
    state: Uuid,
    size: (u32, u32),
    flags: u8,
    limits: Limits,
}
impl Plan {
    pub fn capture(doc: &Document, limits: Limits) -> Result<Self> {
        let resource = |detail| {
            Error::new(
                ErrorCode::ResourceLimit,
                format!("{} {detail}", limits.label),
            )
        };
        if doc.canvas.0 == 0
            || doc.canvas.1 == 0
            || doc.canvas.0 > limits.side
            || doc.canvas.1 > limits.side
            || u64::from(doc.canvas.0) * u64::from(doc.canvas.1) > limits.pixels
            || doc.layers.len() > limits.layers
            || doc.roots.len() != doc.layers.len()
        {
            return Err(resource("document geometry/layer profile"));
        }
        let (mut pixels, mut resident) = (0u64, 0u64);
        let mut metadata = doc.title.as_ref().map_or(0, |s| s.len() as u64);
        for layer in doc.layers.values() {
            pixels = pixels
                .checked_add(u64::from(layer.raster.width()) * u64::from(layer.raster.height()))
                .ok_or_else(|| resource("layer extent overflow"))?;
            let tiles = (layer.raster.resident_tiles() as u64)
                .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64)
                .ok_or_else(|| resource("tile extent overflow"))?;
            resident = resident
                .checked_add(tiles)
                .ok_or_else(|| resource("resident overflow"))?;
            metadata = (layer.name.len() as u64)
                .checked_add(256)
                .and_then(|n| metadata.checked_add(n))
                .ok_or_else(|| resource("metadata overflow"))?;
        }
        if pixels > limits.layer_pixels || resident > limits.resident || metadata > limits.metadata
        {
            return Err(resource("source workspace profile"));
        }
        Ok(Self {
            document: doc.document_id,
            state: doc.state_id,
            size: doc.canvas,
            flags: u8::from(doc.roots.len() > 1)
                | u8::from(doc.layers.values().any(|l| l.raster.has_hidden_rgb())) << 1,
            limits,
        })
    }
    pub fn state(&self) -> Uuid {
        self.state
    }
    pub fn size(&self) -> (u32, u32) {
        self.size
    }
    pub fn flags(&self) -> u8 {
        self.flags
    }
    /// Caller must retain its service admission before invoking this method.
    /// The codec checks fresh geometry after resource inspection, preserving its
    /// own policy/error ordering without adding policy to the shared inventory.
    pub fn snapshot(
        &self,
        doc: &Document,
        check_layout: impl FnOnce((u32, u32)) -> Result<()>,
    ) -> Result<Vec<u8>> {
        let fresh = Self::capture(doc, self.limits)?;
        check_layout(doc.canvas)?;
        if fresh.document != self.document
            || fresh.state != self.state
            || fresh.size != self.size
            || fresh.flags != self.flags
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                format!("{} snapshot changed", self.limits.label),
            ));
        }
        struct Sink {
            bytes: Vec<u8>,
            max: usize,
        }
        impl std::io::Write for Sink {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self
                    .bytes
                    .len()
                    .checked_add(bytes.len())
                    .is_none_or(|n| n > self.max)
                {
                    return Err(std::io::Error::other("snapshot allowance"));
                }
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let resource = |detail| {
            Error::new(
                ErrorCode::ResourceLimit,
                format!("{} {detail}", self.limits.label),
            )
        };
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(self.limits.snapshot)
            .map_err(|_| resource("snapshot allocation"))?;
        let mut sink = Sink {
            bytes,
            max: self.limits.snapshot,
        };
        crate::pcnd::write_to(doc, &mut sink).map_err(|_| resource("snapshot allowance"))?;
        Ok(sink.bytes)
    }
}
#[cfg(test)]
mod tests;
