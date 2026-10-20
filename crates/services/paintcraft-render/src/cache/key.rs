use super::*;
use paintcraft_core::types::Rgba;
use paintcraft_raster::{TileCoord, TileVersion};
use std::mem::size_of;

// Weak keys retain Arc control blocks after their decoded Vecs are freed. Charge
// 128 bytes per key conservatively (including duplicate keys to the same block).
// This is an admission weight, not a claim about allocator/RSS measurements.
const WEAK_CHARGE: usize = 128;
#[derive(Debug, PartialEq, Eq)]
struct LayerKey {
    id: [u8; 16],
    origin: (i64, i64),
    size: (u32, u32),
    opacity: u8,
    default: Rgba,
    versions: Vec<(TileCoord, TileVersion)>,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Key {
    doc: [u8; 16],
    pub rect: Rect,
    layers: Vec<LayerKey>,
}
impl Key {
    pub fn weight(&self) -> usize {
        self.layers.capacity() * size_of::<LayerKey>()
            + self
                .layers
                .iter()
                .map(|l| {
                    l.versions.capacity() * size_of::<(TileCoord, TileVersion)>()
                        + l.versions.len() * WEAK_CHARGE
                })
                .sum::<usize>()
    }

    pub fn build(doc: &Document, rect: Rect, budget: usize) -> Result<Option<Self>> {
        Self::build_prefix(doc, rect, budget, doc.roots.len())
    }

    pub fn build_prefix(
        doc: &Document,
        rect: Rect,
        budget: usize,
        end: usize,
    ) -> Result<Option<Self>> {
        let relevant = |layer: &&paintcraft_document::Layer| {
            layer.visible && layer.opacity != 0 && rect.intersect(&layer.extent()).is_some()
        };
        let count = doc.layers_in_order().take(end).filter(relevant).count();
        if count > budget / size_of::<LayerKey>() {
            return Ok(None);
        }
        let mut key = Self {
            doc: *doc.document_id.as_bytes(),
            rect,
            layers: Vec::new(),
        };
        if key.layers.try_reserve_exact(count).is_err() {
            return Ok(None);
        }
        let mut charge = key.layers.capacity() * size_of::<LayerKey>();
        if charge > budget {
            return Ok(None);
        }
        for layer in doc.layers_in_order().take(end).filter(relevant) {
            let local = rect
                .intersect(&layer.extent())
                .expect("filtered intersection")
                .translated(-layer.origin.0, -layer.origin.1)
                .expect("admitted origin");
            let mut versions = Vec::new();
            for version in layer.raster.tile_versions(local)? {
                if versions.len() == versions.capacity() {
                    let grow = versions.capacity().max(4);
                    let extra = grow * size_of::<(TileCoord, TileVersion)>();
                    if extra > budget.saturating_sub(charge)
                        || versions.try_reserve_exact(grow).is_err()
                    {
                        return Ok(None);
                    }
                    charge += versions.capacity() * size_of::<(TileCoord, TileVersion)>()
                        - (versions.len() * size_of::<(TileCoord, TileVersion)>());
                    if charge > budget {
                        return Ok(None);
                    }
                }
                if WEAK_CHARGE > budget.saturating_sub(charge) {
                    return Ok(None);
                }
                charge += WEAK_CHARGE;
                versions.push(version);
            }
            key.layers.push(LayerKey {
                id: *layer.id.as_bytes(),
                origin: layer.origin,
                size: (layer.raster.width(), layer.raster.height()),
                opacity: layer.opacity,
                default: layer.raster.default_pixel(),
                versions,
            });
        }
        Ok(Some(key))
    }
}
