//! Device-local derived layer views. No host pixels or document leases are retained.
use super::*;
use paintcraft_core::types::{Rect, Rgba};
use paintcraft_document::Layer;
use paintcraft_raster::{REGION_EDGE, RegionCoord, RegionVersion};
use std::mem::size_of;

#[derive(Clone, Copy, Debug)]
pub struct SourceCacheLimits {
    pub buffer_bytes: u64,
    pub metadata_bytes: usize,
    pub entries: usize,
}
impl Default for SourceCacheLimits {
    fn default() -> Self {
        Self {
            buffer_bytes: 64 << 20,
            metadata_bytes: 4 << 20,
            entries: 64,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceCacheUsage {
    pub buffers: usize,
    pub buffer_bytes: u64,
    pub metadata_bytes: usize,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuRenderStats {
    pub composition_submissions: u64,
    pub max_submission_layers: u32,
    pub max_submission_source_bytes: u64,
    pub max_submission_metadata_bytes: u64,
    pub uploaded_bytes: u64,
    pub scaled_native_pixels: u64,
    pub scaled_uniform_pixels: u64,
    pub scaled_tiles: u64,
    pub output_allocations: u64,
    pub output_reuses: u64,
    pub output_cleared_bytes: u64,
    pub retained_output_bytes: u64,
    pub upload_strips: u64,
    pub source_hits: u64,
    pub patched_sources: u64,
    pub source_allocations: u64,
    pub retained: SourceCacheUsage,
}
#[derive(Debug)]
pub struct GpuFrame {
    pub pixels: Vec<u8>,
    pub stats: GpuRenderStats,
}
#[derive(Debug, PartialEq, Eq)]
struct Key {
    doc: [u8; 16],
    layer: [u8; 16],
    origin: (i64, i64),
    extent: (u32, u32),
    default: Rgba,
    area: Rect,
    local: Rect,
    versions: Vec<(RegionCoord, RegionVersion)>,
}
impl Key {
    fn same_layer(&self, other: &Self) -> bool {
        self.doc == other.doc
            && self.layer == other.layer
            && self.origin == other.origin
            && self.extent == other.extent
            && self.default == other.default
    }
    fn same_view(&self, other: &Self) -> bool {
        self.same_layer(other) && self.area == other.area && self.local == other.local
    }
    fn weight(&self) -> usize {
        self.versions.capacity() * size_of::<(RegionCoord, RegionVersion)>()
    }
    fn build(doc: &Document, layer: &Layer, area: Rect, budget: usize) -> Result<Option<Self>> {
        let local = area
            .translated(-layer.origin.0, -layer.origin.1)
            .expect("clipped layer view");
        let Some(regions) = layer.raster.region_versions(local)? else {
            return Ok(None);
        };
        let mut versions = Vec::new();
        for version in regions {
            if versions.len() == versions.capacity() {
                let grow = versions.capacity().max(4);
                if (versions.capacity() + grow) * size_of::<(RegionCoord, RegionVersion)>() > budget
                    || versions.try_reserve_exact(grow).is_err()
                {
                    return Ok(None);
                }
                if versions.capacity() * size_of::<(RegionCoord, RegionVersion)>() > budget {
                    return Ok(None);
                }
            }
            versions.push(version);
        }
        Ok(Some(Self {
            doc: *doc.document_id.as_bytes(),
            layer: *layer.id.as_bytes(),
            origin: layer.origin,
            extent: (layer.raster.width(), layer.raster.height()),
            default: layer.raster.default_pixel(),
            area,
            local,
            versions,
        }))
    }
    fn changed_strips(&self, previous: &Self) -> Option<Vec<Rect>> {
        if !self.same_view(previous) || self.versions.len() != previous.versions.len() {
            return None;
        }
        let mut strips: Vec<Rect> = Vec::new();
        // Keys are row-major. Full-width strips keep each GPU copy contiguous;
        // changed cells on one row and adjacent dirty bands coalesce.
        for ((coord, version), (old_coord, old_version)) in
            self.versions.iter().zip(&previous.versions)
        {
            if coord != old_coord {
                return None;
            }
            if version == old_version {
                continue;
            }
            let edge = i64::from(REGION_EDGE);
            let y0 = (coord.ry * edge).max(self.local.y0);
            let y1 = ((coord.ry + 1) * edge).min(self.local.y1);
            if let Some(last) = strips.last_mut()
                && y0 <= last.y1
            {
                last.y1 = last.y1.max(y1);
                continue;
            }
            strips.try_reserve(1).ok()?;
            strips.push(Rect {
                x0: self.local.x0,
                x1: self.local.x1,
                y0,
                y1,
            });
        }
        Some(strips)
    }
}
#[derive(Debug)]
struct Entry {
    key: Key,
    buffer: wgpu::Buffer,
    touched: u64,
    used_this_frame: bool,
}
#[derive(Debug)]
pub(super) struct SourceCache {
    pub limits: SourceCacheLimits,
    entries: Vec<Entry>,
    clock: u64,
    pub region_views: bool,
}
#[derive(Debug)]
pub(super) struct Prepared {
    pub buffer: wgpu::Buffer,
    key: Option<Key>,
}
impl Prepared {
    pub fn metadata_bytes(&self) -> usize {
        self.key.as_ref().map_or(0, Key::weight)
    }
}
impl SourceCache {
    pub fn new(limits: SourceCacheLimits) -> Result<Self> {
        if limits.entries > 4096 {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "too many GPU source entries",
            ));
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(
                limits
                    .entries
                    .min(limits.metadata_bytes / size_of::<Entry>()),
            )
            .map_err(|_| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "cannot allocate GPU source cache table",
                )
            })?;
        if entries.capacity() * size_of::<Entry>() > limits.metadata_bytes {
            entries = Vec::new()
        }
        Ok(Self {
            limits,
            entries,
            clock: 0,
            region_views: false,
        })
    }
    pub fn clear(&mut self) {
        self.entries.clear()
    }
    pub fn usage(&self) -> SourceCacheUsage {
        SourceCacheUsage {
            buffers: self.entries.len(),
            buffer_bytes: self.entries.iter().map(|e| e.buffer.size()).sum(),
            metadata_bytes: self.entries.capacity() * size_of::<Entry>()
                + self.entries.iter().map(|e| e.key.weight()).sum::<usize>(),
        }
    }
    pub fn begin_frame(&mut self) {
        for entry in &mut self.entries {
            entry.used_this_frame = false;
        }
    }
    fn evict(&mut self) -> bool {
        if let Some((index, _)) = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.used_this_frame)
            .min_by_key(|(_, e)| e.touched)
        {
            self.entries.swap_remove(index);
            true
        } else {
            false
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        doc: &Document,
        layer: &Layer,
        area: Rect,
        check: &impl Fn() -> Result<()>,
        stats: &mut GpuRenderStats,
    ) -> Result<Prepared> {
        check()?;
        let size = area.area() as u64 * 4;
        let budget = self
            .limits
            .metadata_bytes
            .saturating_sub(self.entries.capacity() * size_of::<Entry>());
        let mut key = if self.entries.capacity() > 0 && size <= self.limits.buffer_bytes {
            Key::build(doc, layer, area, budget)?
        } else {
            None
        };
        if let Some(new_key) = key.as_ref()
            && let Some(entry) = self.entries.iter_mut().find(|e| e.key.same_view(new_key))
            && let Some(strips) = new_key.changed_strips(&entry.key)
        {
            entry.used_this_frame = true;
            let buffer = entry.buffer.clone();
            if strips.is_empty() {
                stats.source_hits += 1;
            } else {
                stats.patched_sources += 1;
            }
            for strip in strips {
                let bytes = gather(layer, strip, check)?;
                let staging = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("PaintCraft source strip staging"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::COPY_SRC,
                });
                let offset =
                    (strip.y0 - new_key.local.y0) as u64 * new_key.local.width() as u64 * 4;
                encoder.copy_buffer_to_buffer(&staging, 0, &buffer, offset, bytes.len() as u64);
                stats.uploaded_bytes += bytes.len() as u64;
                stats.upload_strips += 1;
            }
            return Ok(Prepared { buffer, key });
        }
        // Optional regional retention keeps independent views of a compatible layer.
        // A changed layer scope always retires old handles before replacement;
        // pending encoders retain their own references.
        self.entries.retain(|e| {
            e.key.doc != *doc.document_id.as_bytes()
                || e.key.layer != *layer.id.as_bytes()
                || (self.region_views
                    && e.key.origin == layer.origin
                    && e.key.extent == (layer.raster.width(), layer.raster.height())
                    && e.key.default == layer.raster.default_pixel())
        });
        if let Some(weight) = key.as_ref().map(Key::weight) {
            while self.entries.len() == self.entries.capacity()
                || self.usage().buffer_bytes + size > self.limits.buffer_bytes
                || self.usage().metadata_bytes + weight > self.limits.metadata_bytes
            {
                if !self.evict() {
                    // Protect views already used in this frame from a sequential
                    // scan larger than the cache. Stream the overflow instead.
                    key = None;
                    break;
                }
            }
        }
        let local = area
            .translated(-layer.origin.0, -layer.origin.1)
            .expect("clipped layer view");
        let bytes = gather(layer, local, check)?;
        check()?;
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PaintCraft resident layer samples"),
            contents: &bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        stats.uploaded_bytes += size;
        stats.upload_strips += 1;
        stats.source_allocations += 1;
        Ok(Prepared { buffer, key })
    }
    /// Only publish source identities after this layer's GPU work has completed.
    pub fn completed(&mut self, source: Prepared) {
        let Some(key) = source.key else { return };
        self.clock = self.clock.wrapping_add(1);
        if let Some(entry) = self.entries.iter_mut().find(|e| e.key.same_view(&key)) {
            entry.key = key;
            entry.touched = self.clock;
            entry.used_this_frame = true;
            return;
        }
        let usage = self.usage();
        if self.entries.len() < self.entries.capacity()
            && usage.buffer_bytes + source.buffer.size() <= self.limits.buffer_bytes
            && usage.metadata_bytes + key.weight() <= self.limits.metadata_bytes
        {
            self.entries.push(Entry {
                key,
                buffer: source.buffer,
                touched: self.clock,
                used_this_frame: true,
            });
        }
    }
}
fn gather(layer: &Layer, local: Rect, check: &impl Fn() -> Result<()>) -> Result<Vec<u8>> {
    let mut bytes = crate::output_buffer(local.area() as usize * 4)?;
    for (row, y) in bytes
        .chunks_exact_mut(local.width() as usize * 4)
        .zip(local.y0..local.y1)
    {
        check()?;
        layer.raster.read_span(y, local.x0, row);
    }
    check()?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_document::{POLICY, new_document};
    use paintcraft_raster::{Admission, BlendMode};
    #[test]
    fn dirty_source_cells_coalesce_into_clipped_nonoverlapping_row_strips() {
        let mut doc = new_document(257, 257, Some(Rgba([31, 63, 127, 0])), 0, POLICY).unwrap();
        let id = doc.roots[0];
        let area = Rect::new(3, 5, 250, 250).unwrap();
        let before = Key::build(&doc, &doc.layers[&id], area, 1 << 20)
            .unwrap()
            .unwrap();
        let mut adm = Admission::new(65536);
        for (x, y) in [(4, 6), (100, 7), (4, 200)] {
            doc.layers
                .get_mut(&id)
                .unwrap()
                .raster
                .blend_pixel(x, y, Rgba([17, 88, 156, 128]), BlendMode::Replace, &mut adm)
                .unwrap();
        }
        let after = Key::build(&doc, &doc.layers[&id], area, 1 << 20)
            .unwrap()
            .unwrap();
        assert_eq!(
            after.changed_strips(&before).unwrap(),
            vec![
                Rect::new(3, 5, 250, 59).unwrap(),
                Rect::new(3, 192, 250, 63).unwrap()
            ]
        );
        assert!(
            Key::build(&doc, &doc.layers[&id], area, 0)
                .unwrap()
                .is_none()
        );
        doc.layers.get_mut(&id).unwrap().opacity = 12;
        let opacity = Key::build(&doc, &doc.layers[&id], area, 1 << 20)
            .unwrap()
            .unwrap();
        assert!(opacity.changed_strips(&after).unwrap().is_empty());
        doc.layers.get_mut(&id).unwrap().origin = (-1, 0);
        let moved = Key::build(&doc, &doc.layers[&id], area, 1 << 20)
            .unwrap()
            .unwrap();
        assert!(moved.changed_strips(&after).is_none());
    }
    #[test]
    fn source_table_metadata_and_invalid_entry_limits_are_explicit() {
        let disabled = SourceCache::new(SourceCacheLimits {
            buffer_bytes: 0,
            metadata_bytes: 0,
            entries: 0,
        })
        .unwrap();
        assert_eq!(disabled.usage(), SourceCacheUsage::default());
        assert_eq!(
            SourceCache::new(SourceCacheLimits {
                entries: 4097,
                ..SourceCacheLimits::default()
            })
            .unwrap_err()
            .code,
            ErrorCode::InputSchema
        );
    }
}
