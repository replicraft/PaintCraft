use super::*;
use paintcraft_core::types::Rgba;
use paintcraft_raster::{RegionCoord, RegionVersion};
use std::mem::size_of;

#[derive(Debug, PartialEq, Eq)]
struct LayerKey {
    id: [u8; 16],
    origin: (i64, i64),
    size: (u32, u32),
    opacity: u8,
    default: Rgba,
    filters: [u8; 32],
    versions: Vec<(RegionCoord, RegionVersion)>,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Key {
    doc: [u8; 16],
    pub rect: Rect,
    pub level: u8,
    layers: Vec<LayerKey>,
}
impl Key {
    pub fn weight(&self) -> usize {
        self.layers.capacity() * size_of::<LayerKey>()
            + self
                .layers
                .iter()
                .map(|l| l.versions.capacity() * size_of::<(RegionCoord, RegionVersion)>())
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
            level: 0,
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
            let Some(region_versions) = layer.raster.region_versions(local)? else {
                return Ok(None);
            };
            for version in region_versions {
                if versions.len() == versions.capacity() {
                    let grow = versions.capacity().max(4);
                    let extra = grow * size_of::<(RegionCoord, RegionVersion)>();
                    if extra > budget.saturating_sub(charge)
                        || versions.try_reserve_exact(grow).is_err()
                    {
                        return Ok(None);
                    }
                    charge += versions.capacity() * size_of::<(RegionCoord, RegionVersion)>()
                        - (versions.len() * size_of::<(RegionCoord, RegionVersion)>());
                    if charge > budget {
                        return Ok(None);
                    }
                }
                versions.push(version);
            }
            key.layers.push(LayerKey {
                id: *layer.id.as_bytes(),
                origin: layer.origin,
                size: (layer.raster.width(), layer.raster.height()),
                opacity: layer.opacity,
                default: layer.raster.default_pixel(),
                filters: layer.filters.digest(),
                versions,
            });
        }
        Ok(Some(key))
    }
}

impl Key {
    /// Conservative 64-pixel output cells affected by changed source cells. Layer
    /// geometry/policies must match; structural changes require ordinary composition.
    pub(super) fn changed_regions(&self, previous: &Self) -> Option<Vec<Rect>> {
        if self.level != previous.level
            || self.doc != previous.doc
            || self.rect != previous.rect
            || self.layers.len() != previous.layers.len()
        {
            return None;
        }
        let edge = i64::from(paintcraft_raster::REGION_EDGE);
        let columns = (self.rect.width() as usize).div_ceil(edge as usize);
        let mut changed = [false; 256]; // At most 16x16 cells in a 1024-pixel output tile.
        for (new, old) in self.layers.iter().zip(&previous.layers) {
            if new.id != old.id
                || new.origin != old.origin
                || new.size != old.size
                || new.opacity != old.opacity
                || new.default != old.default
                || new.filters != old.filters
                || new.versions.len() != old.versions.len()
            {
                return None;
            }
            for ((coord, version), (old_coord, old_version)) in
                new.versions.iter().zip(&old.versions)
            {
                if coord != old_coord {
                    return None;
                }
                if version == old_version {
                    continue;
                }
                let source = Rect {
                    x0: coord.rx * edge + new.origin.0,
                    y0: coord.ry * edge + new.origin.1,
                    x1: (coord.rx + 1) * edge + new.origin.0,
                    y1: (coord.ry + 1) * edge + new.origin.1,
                };
                let Some(area) = source.intersect(&self.rect) else {
                    continue;
                };
                for y in (area.y0 - self.rect.y0) / edge..=(area.y1 - 1 - self.rect.y0) / edge {
                    for x in (area.x0 - self.rect.x0) / edge..=(area.x1 - 1 - self.rect.x0) / edge {
                        changed[y as usize * columns + x as usize] = true;
                    }
                }
            }
        }
        let mut regions = Vec::new();
        regions
            .try_reserve_exact(changed.iter().filter(|&&v| v).count())
            .ok()?;
        for (index, dirty) in changed.into_iter().enumerate() {
            if dirty {
                let x = self.rect.x0 + (index % columns) as i64 * edge;
                let y = self.rect.y0 + (index / columns) as i64 * edge;
                regions.push(Rect {
                    x0: x,
                    y0: y,
                    x1: (x + edge).min(self.rect.x1),
                    y1: (y + edge).min(self.rect.y1),
                });
            }
        }
        Some(regions)
    }
}
