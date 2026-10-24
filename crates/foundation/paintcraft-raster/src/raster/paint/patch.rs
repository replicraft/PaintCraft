//! Frozen selection extraction, clearing, orientation and exact translation.
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
    paste_mask: Option<BinaryMask>,
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
            if self.selected_pixel(sx, sy)
                && self
                    .paste_mask
                    .as_ref()
                    .is_none_or(|mask| mask.coverage(sx, sy) == 255)
            {
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
            source,
            area,
            None,
            Some(transform),
            (0, 0),
            clear,
            None,
            admission,
            limit,
            check,
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
            Some(transform),
            (0, 0),
            clear,
            None,
            admission,
            limit,
            check,
        )
    }
    /// Move a frozen binary-selected patch by an exact layer-local displacement.
    /// `area` and `mask` are source-local; the returned origin shift rebases the
    /// output relative to the original layer. RGB keys exclude pasting only: all
    /// original selected finite samples are still cleared. Key alpha is ignored.
    /// A zero displacement is always identity, even with a key, so an idle drag
    /// cannot erase keyed pixels. Keyed nonzero moves freeze a paste mask before
    /// replay under the existing read, selection and paint metadata policies.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_moved_patch(
        source: PaintSnapshot,
        area: Rect,
        mask: Option<BinaryMask>,
        offset: (i64, i64),
        clear: Rgba,
        transparent_key: Option<Rgba>,
        admission: &Admission,
        limit: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<(Self, (i64, i64))> {
        Self::prepare_patch_inner(
            source,
            area,
            mask,
            None,
            offset,
            clear,
            transparent_key,
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
        transform: Option<OrthogonalTransform>,
        offset: (i64, i64),
        clear: Rgba,
        transparent_key: Option<Rgba>,
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
        if transform.is_none() && offset == (0, 0) {
            return Self::prepare_identity_patch(source, admission, check)
                .map(|plan| (plan, (0, 0)));
        }
        let (dw, dh) = transform.map_or((w as u32, h as u32), |t| t.dimensions(w as u32, h as u32));
        let destination = Rect::from_origin_size(area.x0, area.y0, dw, dh)
            .and_then(|rect| rect.translated(offset.0, offset.1))
            .ok_or_else(metadata::exhausted)?;
        if [
            destination.x0,
            destination.y0,
            destination.x1,
            destination.y1,
        ]
        .iter()
        .any(|v| v.unsigned_abs() > bound)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "patch destination outside policy",
            ));
        }
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
        let sampling = sampling::Sampling { area, transform };
        let destination = destination
            .translated(-ox, -oy)
            .ok_or_else(metadata::exhausted)?;
        let (paste_mask, key_reads) = match transparent_key {
            Some(key) => {
                let (mask, reads) =
                    keyed_paste_mask(&source, area, mask.as_ref(), key, admission, limit, check)?;
                (Some(mask), reads)
            }
            None => (None, 0),
        };
        let patch = Patch {
            selected: area,
            mask,
            paste_mask,
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
        let mut reads = key_reads;
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

impl MappedPlan {
    fn prepare_identity_patch(
        source: PaintSnapshot,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let mut mapping = mapped::Mapping {
            sampling: sampling::Sampling {
                area: source.bounds(),
                transform: None,
            },
            patch: None,
            tiles: HashMap::new(),
            charge: admission.paint_metadata_account(),
        };
        mapping.charge.resize(size_of::<mapped::Mapping>() as u64)?;
        let mut metadata =
            metadata::Metadata::new(size_of::<mapped::Mapping>(), admission.paint_metadata_limit)?;
        let reader = source.reader(admission, check)?;
        reader.visit_candidate_tiles(check, |coord| {
            let tile = Rect::new(
                coord.tx * i64::from(TILE_EDGE),
                coord.ty * i64::from(TILE_EDGE),
                i64::from(TILE_EDGE),
                i64::from(TILE_EDGE),
            )
            .unwrap();
            if tile.intersect(&source.bounds()).is_some() && !mapping.tiles.contains_key(&coord) {
                metadata.table(&mut mapping.tiles, &mut mapping.charge)?;
                mapping.tiles.insert(coord, ());
            }
            Ok(())
        })?;
        drop(reader);
        check()?;
        Ok(Self {
            source,
            mapping: Arc::new(mapping),
            reads: 0,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn keyed_paste_mask(
    source: &PaintSnapshot,
    area: Rect,
    mask: Option<&BinaryMask>,
    key: Rgba,
    admission: &Admission,
    limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<(BinaryMask, u64)> {
    check()?;
    let reads = u64::try_from(area.area()).map_err(|_| metadata::exhausted())?;
    if reads > limit.min(16_777_216) {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "patch key sample budget exceeded",
        ));
    }
    // The temporary ordered runs are capacity-priced in the paint realm; the
    // retained binary field keeps its own existing selection-storage ownership.
    let mut charge = admission.paint_metadata_account();
    let mut spans = Vec::new();
    let mut metadata = metadata::Metadata::new(0, admission.paint_metadata_limit)?;
    let reader = source.reader(admission, check)?;
    for y in area.y0..area.y1 {
        check()?;
        let mut start = None;
        for x in area.x0..area.x1 {
            if ((x - area.x0) as u64).is_multiple_of(256) {
                check()?;
            }
            let selected = mask.is_none_or(|mask| mask.coverage(x, y) == 255);
            let paste = if selected {
                let color = if source.bounds().contains(x, y) {
                    reader.read_checked(x, y, check)?
                } else {
                    Rgba::TRANSPARENT
                };
                color.0[..3] != key.0[..3]
            } else {
                false
            };
            if paste {
                start.get_or_insert(x);
            } else if let Some(x0) = start.take() {
                if spans.len() == crate::selection::MaskLimits::default().spans {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "patch key span budget exceeded",
                    ));
                }
                metadata.vector(&mut spans, &mut charge)?;
                spans.push(paintcraft_core::types::text::MaskSpan {
                    y,
                    x0,
                    x1: x - 1,
                    coverage: 255,
                });
            }
        }
        if let Some(x0) = start {
            if spans.len() == crate::selection::MaskLimits::default().spans {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "patch key span budget exceeded",
                ));
            }
            metadata.vector(&mut spans, &mut charge)?;
            spans.push(paintcraft_core::types::text::MaskSpan {
                y,
                x0,
                x1: area.x1 - 1,
                coverage: 255,
            });
        }
    }
    drop(reader);
    let field = BinaryMask::from_spans(
        area,
        false,
        &spans,
        &admission.selection_budget(),
        crate::selection::MaskLimits::default(),
        check,
    )?;
    Ok((field, reads))
}
