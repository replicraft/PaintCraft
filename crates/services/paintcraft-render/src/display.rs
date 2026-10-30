//! Derived display-only box samples: compose full-resolution pixels before filtering.
use crate::{
    cache::{CacheLimits, CacheUsage, Domain, key::Key, storage::Partition},
    *,
};
use paintcraft_raster::{REGION_EDGE, RegionCoord, RegionVersion};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayRequest {
    region: Rect,
    level: u8,
}
impl DisplayRequest {
    /// Align an in-canvas viewport outward to a global power-of-two sample grid.
    /// The final canvas-edge sample contains only actual canvas pixels.
    pub fn new(doc: &Document, visible: Rect, level: u8) -> Result<Self> {
        validate_region(doc, visible)?;
        if level > 20 {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "display level exceeds canvas policy",
            ));
        }
        let step = 1i64 << level;
        let region = Rect {
            x0: visible.x0 / step * step,
            y0: visible.y0 / step * step,
            x1: ((visible.x1 + step - 1) / step * step).min(i64::from(doc.canvas.0)),
            y1: ((visible.y1 + step - 1) / step * step).min(i64::from(doc.canvas.1)),
        };
        let request = Self { region, level };
        let size = request.size();
        if u64::from(size[0]) * u64::from(size[1]) > RENDER_PIXELS_MAX
            || region.area() > u128::from(u64::MAX / 65536)
        {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "display output or accumulator exceeds limits",
            ));
        }
        Ok(request)
    }
    pub fn region(self) -> Rect {
        self.region
    }
    pub fn level(self) -> u8 {
        self.level
    }
    pub fn step(self) -> u32 {
        1 << self.level
    }
    pub fn size(self) -> [u32; 2] {
        [
            (self.region.width() as u32).div_ceil(self.step()),
            (self.region.height() as u32).div_ceil(self.step()),
        ]
    }
    /// Uniform texel footprint. Clip drawing to the actual canvas at partial edges.
    pub fn footprint(self) -> Rect {
        let [w, h] = self.size();
        Rect::new(
            self.region.x0,
            self.region.y0,
            i64::from(w) * i64::from(self.step()),
            i64::from(h) * i64::from(self.step()),
        )
        .unwrap()
    }
    fn bytes(self) -> usize {
        let [w, h] = self.size();
        w as usize * h as usize * 4
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DisplayStats {
    pub cache_hits: u64,
    pub recomposed_tiles: u64,
    pub native_pixels: u64,
    pub uniform_pixels: u64,
    pub output_pixels: u64,
}
#[derive(Debug)]
pub struct DisplayFrame {
    pub request: DisplayRequest,
    pub pixels: Vec<u8>,
    pub stats: DisplayStats,
}
#[derive(Clone, Copy, Default)]
pub(crate) struct Sum(pub(crate) [u64; 4]);
impl Sum {
    pub(crate) fn add(&mut self, color: Rgba, count: u64) {
        let alpha = u64::from(color.a()) * count;
        for (sum, color) in self.0[..3].iter_mut().zip(color.0) {
            *sum += u64::from(color) * alpha;
        }
        self.0[3] += alpha;
    }
    fn pixel(self, count: u64) -> [u8; 4] {
        let a = (self.0[3] + count / 2) / count;
        if a == 0 {
            return [0; 4];
        }
        let channel = |sum| ((sum + self.0[3] / 2) / self.0[3]) as u8;
        [
            channel(self.0[0]),
            channel(self.0[1]),
            channel(self.0[2]),
            a as u8,
        ]
    }
}

/// No layer is downsampled separately: this proves the complete ordered composite
/// uniform, or requests ordinary exact native composition for the unknown region.
pub(crate) fn uniform(
    doc: &Document,
    rect: Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<Rgba>> {
    let mut color = Rgba::TRANSPARENT;
    for layer in doc.layers_in_order() {
        check()?;
        if !layer.visible || layer.opacity == 0 {
            continue;
        }
        let Some(area) = rect.intersect(&layer.extent()) else {
            continue;
        };
        let local = area.translated(-layer.origin.0, -layer.origin.1).unwrap();
        let Some(pixel) = layer.raster.uniform_region(local, check)? else {
            return Ok(None);
        };
        let pixel = crate::appearance::pixel(layer, pixel, check)?.with_opacity(layer.opacity);
        if pixel.a() == 0 {
            continue;
        }
        if area != rect {
            return Ok(None);
        }
        color = pixel.source_over(color);
    }
    Ok(Some(color))
}

struct Reduction<'a> {
    doc: &'a Document,
    request: DisplayRequest,
    sums: Vec<Sum>,
    stats: &'a mut DisplayStats,
    check: &'a dyn Fn() -> Result<()>,
}
impl Reduction<'_> {
    fn add_uniform(&mut self, rect: Rect, color: Rgba) {
        let grid = self.request.region;
        let step = i64::from(self.request.step());
        let width = self.request.size()[0] as usize;
        for y in (rect.y0 - grid.y0) / step..=(rect.y1 - grid.y0 - 1) / step {
            for x in (rect.x0 - grid.x0) / step..=(rect.x1 - grid.x0 - 1) / step {
                let cell = Rect::new(grid.x0 + x * step, grid.y0 + y * step, step, step).unwrap();
                let count = cell.intersect(&rect).unwrap().area() as u64;
                self.sums[y as usize * width + x as usize].add(color, count);
            }
        }
    }
    fn visit(&mut self, rect: Rect) -> Result<()> {
        (self.check)()?;
        if let Some(color) = uniform(self.doc, rect, self.check)? {
            self.stats.uniform_pixels += rect.area() as u64;
            self.add_uniform(rect, color);
        } else if rect.width() <= 256 && rect.height() <= 256 {
            let pixels = render_region_cancellable(self.doc, rect, self.check)?;
            self.stats.native_pixels += rect.area() as u64;
            let grid = self.request.region;
            let step = i64::from(self.request.step());
            let out_width = self.request.size()[0] as usize;
            for (row, pixels) in pixels.chunks_exact(rect.width() as usize * 4).enumerate() {
                (self.check)()?;
                let y = (rect.y0 + row as i64 - grid.y0) / step;
                for (column, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
                    let x = (rect.x0 + column as i64 - grid.x0) / step;
                    self.sums[y as usize * out_width + x as usize].add(Rgba(*pixel), 1);
                }
            }
        } else {
            let (a, b) = if rect.width() >= rect.height() {
                let mid = rect.x0 + rect.width() / 2;
                (Rect { x1: mid, ..rect }, Rect { x0: mid, ..rect })
            } else {
                let mid = rect.y0 + rect.height() / 2;
                (Rect { y1: mid, ..rect }, Rect { y0: mid, ..rect })
            };
            self.visit(a)?;
            self.visit(b)?;
        }
        Ok(())
    }
}

fn reduce(
    doc: &Document,
    request: DisplayRequest,
    stats: &mut DisplayStats,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let [w, h] = request.size();
    let count = w as usize * h as usize;
    let mut sums = Vec::new();
    sums.try_reserve_exact(count).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "cannot allocate display accumulators",
        )
    })?;
    sums.resize(count, Sum::default());
    let mut reduction = Reduction {
        doc,
        request,
        sums,
        stats,
        check,
    };
    reduction.visit(request.region)?;
    let mut pixels = output_buffer(request.bytes())?;
    let step = i64::from(request.step());
    for y in 0..h as usize {
        check()?;
        for x in 0..w as usize {
            let cell = Rect::new(
                request.region.x0 + x as i64 * step,
                request.region.y0 + y as i64 * step,
                step,
                step,
            )
            .unwrap();
            let count = cell.intersect(&request.region).unwrap().area() as u64;
            pixels[(y * w as usize + x) * 4..][..4]
                .copy_from_slice(&reduction.sums[y * w as usize + x].pixel(count));
        }
    }
    Ok(pixels)
}

#[derive(Debug)]
pub struct DisplayCache {
    limits: CacheLimits,
    committed: Partition,
    preview: Partition,
}
impl DisplayCache {
    pub fn new(limits: CacheLimits) -> Result<Self> {
        if !(1..=64).contains(&limits.tile_edge) || !(1..=4096).contains(&limits.entries_per_domain)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "invalid display cache geometry",
            ));
        }
        Ok(Self {
            limits,
            committed: Partition::new(limits.committed_bytes, limits.entries_per_domain)?,
            preview: Partition::new(limits.preview_bytes, limits.entries_per_domain)?,
        })
    }
    pub fn usage(&self, domain: Domain) -> CacheUsage {
        match domain {
            Domain::Committed => self.committed.usage(),
            Domain::Preview => self.preview.usage(),
        }
    }
    pub fn clear_preview(&mut self) {
        self.preview.clear();
    }
    pub fn render(
        &mut self,
        doc: &Document,
        request: DisplayRequest,
        domain: Domain,
        check: impl Fn() -> Result<()>,
    ) -> Result<DisplayFrame> {
        let request = DisplayRequest::new(doc, request.region, request.level)?;
        let mut pixels = output_buffer(request.bytes())?;
        let mut stats = DisplayStats {
            output_pixels: request.bytes() as u64 / 4,
            ..Default::default()
        };
        let step = i64::from(request.step());
        let span = step * i64::from(self.limits.tile_edge);
        let bounds = request.region;
        for ty in bounds.y0 / span..=(bounds.y1 - 1) / span {
            for tx in bounds.x0 / span..=(bounds.x1 - 1) / span {
                check()?;
                let rect = Rect {
                    x0: tx * span,
                    y0: ty * span,
                    x1: ((tx + 1) * span).min(i64::from(doc.canvas.0)),
                    y1: ((ty + 1) * span).min(i64::from(doc.canvas.1)),
                };
                let tile_request = DisplayRequest::new(doc, rect, request.level)?;
                let partition = match domain {
                    Domain::Committed => &self.committed,
                    Domain::Preview => &self.preview,
                };
                let key_budget = partition.key_budget(tile_request.bytes());
                // Uniform results are already cheap and need no potentially huge
                // region-version key. Large unknown keys bypass before enumeration.
                let color = uniform(doc, rect, &check)?;
                let cells = (rect.width() as u64).div_ceil(u64::from(REGION_EDGE))
                    * (rect.height() as u64).div_ceil(u64::from(REGION_EDGE));
                let key = if color.is_none()
                    && cells
                        <= (key_budget / std::mem::size_of::<(RegionCoord, RegionVersion)>()) as u64
                {
                    Key::build(doc, rect, key_budget)?.map(|mut key| {
                        key.level = request.level;
                        key
                    })
                } else {
                    None
                };
                let hit = key.as_ref().and_then(|key| {
                    if domain == Domain::Preview
                        && let Some(index) = self.committed.find(key)
                    {
                        return Some((Domain::Committed, index));
                    }
                    partition.find(key).map(|i| (domain, i))
                });
                let mut computed = None;
                let tile = if let Some((cached_domain, index)) = hit {
                    stats.cache_hits += 1;
                    match cached_domain {
                        Domain::Committed => self.committed.touch(index),
                        Domain::Preview => self.preview.touch(index),
                    }
                } else {
                    stats.recomposed_tiles += 1;
                    computed = Some(if let Some(color) = color {
                        stats.uniform_pixels += rect.area() as u64;
                        let mut out = output_buffer(tile_request.bytes())?;
                        let color = if color.a() == 0 { [0; 4] } else { color.0 };
                        for p in out.as_chunks_mut::<4>().0 {
                            p.copy_from_slice(&color);
                        }
                        out
                    } else {
                        reduce(doc, tile_request, &mut stats, &check)?
                    });
                    computed.as_ref().unwrap()
                };
                let area = rect.intersect(&bounds).unwrap();
                let tile_width = tile_request.size()[0] as usize;
                let width = request.size()[0] as usize;
                let columns = (area.width() as u64).div_ceil(step as u64) as usize;
                for row in 0..(area.height() as u64).div_ceil(step as u64) as usize {
                    let src = (((area.y0 - rect.y0) / step) as usize + row) * tile_width
                        + ((area.x0 - rect.x0) / step) as usize;
                    let dst = (((area.y0 - bounds.y0) / step) as usize + row) * width
                        + ((area.x0 - bounds.x0) / step) as usize;
                    pixels[dst * 4..][..columns * 4]
                        .copy_from_slice(&tile[src * 4..][..columns * 4]);
                }
                if let (Some(key), Some(computed)) = (key, computed) {
                    let partition = match domain {
                        Domain::Committed => &mut self.committed,
                        Domain::Preview => &mut self.preview,
                    };
                    partition.insert(key, computed, false);
                }
            }
        }
        check()?;
        Ok(DisplayFrame {
            request,
            pixels,
            stats,
        })
    }
}

/// Correct uncached display baseline; no full-canvas native intermediate is created.
pub fn render_display_cancellable(
    doc: &Document,
    request: DisplayRequest,
    check: impl Fn() -> Result<()>,
) -> Result<DisplayFrame> {
    DisplayCache::new(CacheLimits {
        tile_edge: 32,
        committed_bytes: 0,
        preview_bytes: 0,
        entries_per_domain: 1,
    })?
    .render(doc, request, Domain::Committed, check)
}
