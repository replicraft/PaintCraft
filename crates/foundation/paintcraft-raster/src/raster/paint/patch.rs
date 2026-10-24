//! Frozen rectangular extraction, clear and exact replacement over one source.
use super::*;
use crate::selection::BinaryMask;
use paintcraft_core::types::orthogonal::OrthogonalTransform;
use std::mem::size_of;

pub(super) enum Sample {
    Source(i64, i64),
    Color(Rgba),
}
#[derive(Debug)]
pub(super) struct Patch {
    pub bounds: Rect,
    source_bounds: Rect,
    source_origin: (i64, i64),
    destination: Rect,
    clear: Rgba,
    selected: Rect,
    mask: Option<BinaryMask>,
    coordinates: sampling::Coordinates,
}
impl Patch {
    fn selected_pixel(&self, x: i64, y: i64) -> bool {
        self.selected.contains(x, y)
            && self
                .mask
                .as_ref()
                .is_none_or(|mask| mask.coverage(x, y) == 255)
    }
    pub fn sample(&self, x: i64, y: i64) -> Result<Sample> {
        if self.destination.contains(x, y) {
            let (sx, sy) = self
                .coordinates
                .pixel(x - self.destination.x0, y - self.destination.y0)?;
            if self.selected_pixel(sx, sy) {
                return Ok(if self.source_bounds.contains(sx, sy) {
                    Sample::Source(sx, sy)
                } else {
                    Sample::Color(Rgba::TRANSPARENT)
                });
            }
        }
        let (sx, sy) = (x - self.source_origin.0, y - self.source_origin.1);
        Ok(if !self.source_bounds.contains(sx, sy) {
            Sample::Color(Rgba::TRANSPARENT)
        } else if self.selected_pixel(sx, sy) {
            Sample::Color(self.clear)
        } else {
            Sample::Source(sx, sy)
        })
    }
}
fn difference(outer: Rect, inner: Rect) -> Vec<Rect> {
    let Some(i) = outer.intersect(&inner) else {
        return vec![outer];
    };
    [
        (outer.x0, outer.y0, outer.width(), i.y0 - outer.y0),
        (outer.x0, i.y1, outer.width(), outer.y1 - i.y1),
        (outer.x0, i.y0, i.x0 - outer.x0, i.height()),
        (i.x1, i.y0, outer.x1 - i.x1, i.height()),
    ]
    .into_iter()
    .filter_map(|(x, y, w, h)| Rect::new(x, y, w, h))
    .collect()
}
fn transformed(s: sampling::Sampling, r: Rect) -> Rect {
    let a = s.target_pixel(r.x0, r.y0);
    let b = s.target_pixel(r.x1 - 1, r.y1 - 1);
    Rect::new(
        a.0.min(b.0),
        a.1.min(b.1),
        (a.0 - b.0).abs() + 1,
        (a.1 - b.1).abs() + 1,
    )
    .unwrap()
}
impl MappedPlan {
    /// One source layer, extracted before clearing. `area` is source-local and
    /// may extend outside its finite bounds; those samples are transparent.
    /// Returns output-origin displacement relative to the old source origin.
    pub fn prepare_patch(
        source: PaintSnapshot,
        area: Rect,
        transform: OrthogonalTransform,
        clear: Rgba,
        admission: &Admission,
        limit: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<(Self, (i64, i64))> {
        Self::prepare_patch_inner(
            source, area, None, transform, clear, admission, limit, check,
        )
    }
    /// Cut and reorient only binary-selected pixels within `area`, relative to
    /// that rectangle's top-left. Both `area` and `mask` use source-layer-local
    /// coordinates; callers translate document-space masks before preparation.
    /// `area` should be the tight selected bounds. Destination holes preserve
    /// the old pixel unless it belongs to the original cut; selected destination
    /// pixels win on overlap. Selected samples outside the old finite layer are
    /// transparent, regardless of its default pixel. Retains the admitted mask
    /// view without copying it; preparation/replay use the ordinary sample limit.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_masked_patch(
        source: PaintSnapshot,
        area: Rect,
        mask: BinaryMask,
        transform: OrthogonalTransform,
        clear: Rgba,
        admission: &Admission,
        limit: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<(Self, (i64, i64))> {
        Self::prepare_patch_inner(
            source,
            area,
            Some(mask),
            transform,
            clear,
            admission,
            limit,
            check,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_patch_inner(
        source: PaintSnapshot,
        area: Rect,
        mask: Option<BinaryMask>,
        transform: OrthogonalTransform,
        clear: Rgba,
        admission: &Admission,
        limit: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<(Self, (i64, i64))> {
        check()?;
        // Check arbitrary typed rectangles before subtracting their endpoints.
        let w = area
            .x1
            .checked_sub(area.x0)
            .filter(|w| *w > 0 && *w <= 1_048_576);
        let h = area
            .y1
            .checked_sub(area.y0)
            .filter(|h| *h > 0 && *h <= 1_048_576);
        let (Some(w), Some(h)) = (w, h) else {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "invalid patch rectangle",
            ));
        };
        let bound = paintcraft_core::types::policy::COORD_ABS_MAX as u64 + 1_048_576;
        if [area.x0, area.y0, area.x1, area.y1]
            .iter()
            .any(|v| v.unsigned_abs() > bound)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "patch coordinates outside policy",
            ));
        }
        let (dw, dh) = transform.dimensions(w as u32, h as u32);
        let destination =
            Rect::from_origin_size(area.x0, area.y0, dw, dh).ok_or_else(metadata::exhausted)?;
        let old = source.bounds();
        let (ox, oy) = (old.x0.min(destination.x0), old.y0.min(destination.y0));
        let (endx, endy) = (old.x1.max(destination.x1), old.y1.max(destination.y1));
        let width = endx
            .checked_sub(ox)
            .filter(|w| *w <= 1_048_576)
            .ok_or_else(metadata::exhausted)?;
        let height = endy
            .checked_sub(oy)
            .filter(|h| *h <= 1_048_576)
            .ok_or_else(metadata::exhausted)?;
        let sampling = sampling::Sampling {
            area,
            transform: Some(transform),
        };
        let destination = destination
            .translated(-ox, -oy)
            .ok_or_else(metadata::exhausted)?;
        let patch = Patch {
            selected: area,
            mask,
            coordinates: sampling.coordinates(),
            bounds: Rect::new(0, 0, width, height).unwrap(),
            source_bounds: old,
            source_origin: (-ox, -oy),
            destination,
            clear,
        };
        let mut mapping = mapped::Mapping {
            sampling,
            patch: Some(patch),
            tiles: HashMap::new(),
            charge: admission.paint_metadata_account(),
        };
        mapping.charge.resize(size_of::<mapped::Mapping>() as u64)?;
        let mut metadata =
            metadata::Metadata::new(size_of::<mapped::Mapping>(), admission.paint_metadata_limit)?;
        let mut candidates = HashMap::new();
        let mut candidate_charge = admission.paint_metadata_account();
        let mut add = |r: Rect| -> Result<()> {
            if let Some(r) = r.intersect(&mapping.bounds()) {
                for ty in r.y0 / i64::from(TILE_EDGE)..=(r.y1 - 1) / i64::from(TILE_EDGE) {
                    check()?;
                    for tx in r.x0 / i64::from(TILE_EDGE)..=(r.x1 - 1) / i64::from(TILE_EDGE) {
                        let coord = TileCoord { tx, ty };
                        if !candidates.contains_key(&coord) {
                            metadata.table(&mut candidates, &mut candidate_charge)?;
                            candidates.insert(coord, ());
                        }
                    }
                }
            }
            Ok(())
        };
        let reader = source.reader(admission, check)?;
        reader.visit_candidate_tiles(check, |coord| {
            let tile = Rect::new(
                coord.tx * i64::from(TILE_EDGE),
                coord.ty * i64::from(TILE_EDGE),
                i64::from(TILE_EDGE),
                i64::from(TILE_EDGE),
            )
            .unwrap();
            if let Some(r) = tile.intersect(&old) {
                add(r.translated(-ox, -oy).unwrap())?;
                if let Some(r) = r.intersect(&area) {
                    add(transformed(sampling, r)
                        .translated(destination.x0, destination.y0)
                        .unwrap())?;
                }
            }
            Ok(())
        })?;
        if clear != source.default_pixel()
            && let Some(r) = area.intersect(&old)
        {
            add(r.translated(-ox, -oy).unwrap())?;
        }
        if source.default_pixel() != Rgba::TRANSPARENT {
            for r in difference(mapping.bounds(), old.translated(-ox, -oy).unwrap()) {
                add(r)?;
            }
            for r in difference(area, old) {
                add(transformed(sampling, r)
                    .translated(destination.x0, destination.y0)
                    .unwrap())?;
            }
        }
        let mut reads = 0u64;
        for coord in candidates.keys() {
            let r = Rect::new(
                coord.tx * i64::from(TILE_EDGE),
                coord.ty * i64::from(TILE_EDGE),
                i64::from(TILE_EDGE),
                i64::from(TILE_EDGE),
            )
            .unwrap()
            .intersect(&mapping.bounds())
            .unwrap();
            reads = reads
                .checked_add(r.width() as u64 * r.height() as u64)
                .ok_or_else(metadata::exhausted)?;
            if reads > limit.min(16_777_216) {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "patch sample budget exceeded",
                ));
            }
            let mut nondefault = false;
            for y in r.y0..r.y1 {
                check()?;
                for x in r.x0..r.x1 {
                    let c = match mapping.sample(x, y)? {
                        Sample::Source(sx, sy) => reader.read_checked(sx, sy, check)?,
                        Sample::Color(c) => c,
                    };
                    nondefault |= c != source.default_pixel();
                }
            }
            if nondefault {
                metadata.table(&mut mapping.tiles, &mut mapping.charge)?;
                mapping.tiles.insert(*coord, ());
            }
        }
        drop(reader);
        check()?;
        Ok((
            Self {
                source,
                mapping: Arc::new(mapping),
                reads,
            },
            (ox, oy),
        ))
    }
}
