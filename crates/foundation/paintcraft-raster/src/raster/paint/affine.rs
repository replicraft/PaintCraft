//! Source-bound affine sampling; metadata first, decoded allocation only on replay.
use super::*;
use crate::Selection;
use crate::resampling::Geometry;
use paintcraft_core::types::{rotation::Rotation, stretch_skew::StretchSkew};
use std::mem::size_of;
/// Global coordinates bind the geometry to one captured layer. Selected mode
/// preserves unselected pixels and clears the original selection before replacing.
#[derive(Clone, Debug)]
pub struct StretchSkewMapping {
    pub transform: StretchSkew,
    pub source_origin: (i64, i64),
    pub destination: Rect,
    pub selection: Option<Selection>,
    pub clear: Rgba,
}
#[derive(Clone, Debug)]
pub struct RotationMapping {
    pub transform: Rotation,
    pub source_origin: (i64, i64),
    pub destination: Rect,
    pub selection: Option<Selection>,
    pub clear: Rgba,
}
#[derive(Debug)]
struct Request {
    pub transform: Geometry,
    pub source_origin: (i64, i64),
    pub destination: Rect,
    pub selection: Option<Selection>,
    pub clear: Rgba,
}
#[derive(Debug)]
pub(super) struct Affine {
    request: Request,
    source_bounds: Rect,
}
impl Affine {
    // Destination was validated before formation. Avoid retaining a duplicate
    // local bounds Rect alongside the same destination width/height.
    pub(super) fn bounds(&self) -> Rect {
        Rect {
            x0: 0,
            y0: 0,
            x1: self.request.destination.width(),
            y1: self.request.destination.height(),
        }
    }
    fn selected(&self, x: i64, y: i64) -> bool {
        if self.request.selection.is_some() && !self.request.transform.source().contains(x, y) {
            return false;
        }
        match &self.request.selection {
            Some(Selection::Rect(r)) => r.contains(x, y),
            Some(Selection::Mask(m)) => m.coverage(x, y) == 255,
            Some(Selection::Empty) => false,
            _ => true,
        }
    }
    pub fn sample(&self, x: i64, y: i64) -> Result<patch::Sample> {
        let r = &self.request;
        let (gx, gy) = (x + r.destination.x0, y + r.destination.y0);
        let (sx, sy) = r.transform.source_pixel(gx, gy)?;
        if (r.selection.is_none() || r.transform.source().contains(sx, sy)) && self.selected(sx, sy)
        {
            let local = (
                sx.checked_sub(r.source_origin.0)
                    .ok_or_else(metadata::exhausted)?,
                sy.checked_sub(r.source_origin.1)
                    .ok_or_else(metadata::exhausted)?,
            );
            return Ok(if self.source_bounds.contains(local.0, local.1) {
                patch::Sample::Source(local.0, local.1)
            } else {
                patch::Sample::Color(Rgba::TRANSPARENT)
            });
        }
        if r.selection.is_some() {
            let local = (gx - r.source_origin.0, gy - r.source_origin.1);
            if self.source_bounds.contains(local.0, local.1) {
                return Ok(if self.selected(gx, gy) {
                    patch::Sample::Color(r.clear)
                } else {
                    patch::Sample::Source(local.0, local.1)
                });
            }
        }
        Ok(patch::Sample::Color(Rgba::TRANSPARENT))
    }
}
impl MappedPlan {
    /// Destination visits are bounded cumulatively by the caller's existing
    /// transform-read allowance, including implicit/default and padding samples.
    pub fn prepare_stretch_skew(
        source: PaintSnapshot,
        request: StretchSkewMapping,
        admission: &Admission,
        limit: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        Self::prepare_affine(
            source,
            Request {
                transform: Geometry::StretchSkew(request.transform),
                source_origin: request.source_origin,
                destination: request.destination,
                selection: request.selection,
                clear: request.clear,
            },
            admission,
            limit,
            check,
        )
    }
    /// Prepare one immutable-source nearest-neighbor rotation; no intermediate
    /// resampled image is created. Admission and the cumulative read limit are
    /// the caller's original ones, exactly as for stretch/skew.
    /// # Errors
    /// Refuses invalid extents, resource admission or the original callback error.
    pub fn prepare_rotation(
        source: PaintSnapshot,
        request: RotationMapping,
        admission: &Admission,
        limit: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        Self::prepare_affine(
            source,
            Request {
                transform: Geometry::Rotation(request.transform),
                source_origin: request.source_origin,
                destination: request.destination,
                selection: request.selection,
                clear: request.clear,
            },
            admission,
            limit,
            check,
        )
    }
    fn prepare_affine(
        source: PaintSnapshot,
        request: Request,
        admission: &Admission,
        limit: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        let d = request.destination;
        use paintcraft_core::types::policy::{CANVAS_SIDE_MAX, COORD_ABS_MAX};
        if [
            d.x0,
            d.y0,
            d.x1,
            d.y1,
            request.source_origin.0,
            request.source_origin.1,
        ]
        .into_iter()
        .any(|n| n.unsigned_abs() > COORD_ABS_MAX as u64)
            || d.x1 <= d.x0
            || d.y1 <= d.y0
            || d.width() > i64::from(CANVAS_SIDE_MAX)
            || d.height() > i64::from(CANVAS_SIDE_MAX)
        {
            return Err(metadata::exhausted());
        }
        let bounds = Rect::from_origin_size(
            0,
            0,
            u32::try_from(d.width()).map_err(|_| metadata::exhausted())?,
            u32::try_from(d.height()).map_err(|_| metadata::exhausted())?,
        )
        .ok_or_else(metadata::exhausted)?;
        let reads = (bounds.width() as u64)
            .checked_mul(bounds.height() as u64)
            .ok_or_else(metadata::exhausted)?;
        if reads > limit.min(16_777_216) {
            return Err(metadata::exhausted());
        }
        // Validate finite output before any source reader/index allocation.
        request.transform.bounds(request.transform.source())?;
        let affine = Affine {
            request,
            source_bounds: source.bounds(),
        };
        let mut mapping = mapped::Mapping {
            sampling: sampling::Sampling {
                area: source.bounds(),
                transform: None,
            },
            patch: None,
            affine: Some(affine),
            tiles: HashMap::new(),
            charge: admission.paint_metadata_account(),
        };
        mapping.charge.resize(size_of::<mapped::Mapping>() as u64)?;
        let mut metadata =
            metadata::Metadata::new(size_of::<mapped::Mapping>(), admission.paint_metadata_limit)?;
        let reader = source.reader(admission, check)?;
        for y in 0..bounds.y1 {
            check()?;
            for x in 0..bounds.x1 {
                if x % 64 == 0 {
                    check()?;
                }
                let pixel = match mapping.sample(x, y)? {
                    patch::Sample::Source(sx, sy) => reader.read_checked(sx, sy, check)?,
                    patch::Sample::Color(c) => c,
                };
                if pixel != source.default_pixel() {
                    let coord = TileCoord {
                        tx: x / i64::from(TILE_EDGE),
                        ty: y / i64::from(TILE_EDGE),
                    };
                    if !mapping.tiles.contains_key(&coord) {
                        metadata.table(&mut mapping.tiles, &mut mapping.charge)?;
                        mapping.tiles.insert(coord, ());
                    }
                }
            }
        }
        drop(reader);
        check()?;
        Ok(Self {
            source,
            mapping: Arc::new(mapping),
            reads,
        })
    }
}
