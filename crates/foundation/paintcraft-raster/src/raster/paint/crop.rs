//! Sparse source-bound cropping: prepare metadata, then allocate only after admission.
use super::*;
use paintcraft_core::types::resources::BudgetAccount;
use std::mem::size_of;

const READS_MAX: u64 = 16_777_216;
#[derive(Debug)]
pub(super) struct Mapping {
    pub(super) area: Rect,
    pub(super) tiles: HashMap<TileCoord, ()>,
    charge: BudgetAccount,
}
impl Mapping {
    pub(super) fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.area.width(), self.area.height()).unwrap()
    }
    pub(super) fn bytes(&self) -> usize {
        self.charge.bytes() as usize
    }
}
#[derive(Clone, Debug)]
pub struct CropPlan {
    source: PaintSnapshot,
    mapping: Arc<Mapping>,
    reads: u64,
}
impl CropPlan {
    pub fn prepare(
        source: PaintSnapshot,
        area: Rect,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        Self::prepare_limited(source, area, admission, READS_MAX, check)
    }
    pub fn prepare_limited(
        source: PaintSnapshot,
        area: Rect,
        admission: &Admission,
        limit: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if source.bounds().intersect(&area) != Some(area) || area.width() <= 0 || area.height() <= 0
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "crop must be inside its source",
            ));
        }
        let mut mapping = Mapping {
            area,
            tiles: HashMap::new(),
            charge: admission.paint_metadata_account(),
        };
        mapping.charge.resize(size_of::<Mapping>() as u64)?;
        let mut metadata =
            metadata::Metadata::new(size_of::<Mapping>(), admission.paint_metadata_limit)?;
        let reader = source.reader(admission, check)?;
        let mut candidates = HashMap::new();
        let mut candidate_charge = admission.paint_metadata_account();
        reader.visit_candidate_tiles(check, |coord| {
            if tile_rect(coord).intersect(&area).is_some() && !candidates.contains_key(&coord) {
                metadata.table(&mut candidates, &mut candidate_charge)?;
                candidates.insert(coord, ());
            }
            Ok(())
        })?;
        let mut reads = 0_u64;
        for coord in candidates.keys() {
            let region = tile_rect(*coord).intersect(&area).unwrap();
            reads = reads
                .checked_add(region.width() as u64 * region.height() as u64)
                .ok_or_else(metadata::exhausted)?;
            if reads > limit.min(READS_MAX) {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "crop source-read budget exceeded",
                ));
            }
            for y in region.y0..region.y1 {
                check()?;
                for x in region.x0..region.x1 {
                    if reader.read_checked(x, y, check)? != source.default_pixel() {
                        let target = TileCoord {
                            tx: (x - area.x0) / i64::from(TILE_EDGE),
                            ty: (y - area.y0) / i64::from(TILE_EDGE),
                        };
                        if !mapping.tiles.contains_key(&target) {
                            metadata.table(&mut mapping.tiles, &mut mapping.charge)?;
                            mapping.tiles.insert(target, ());
                        }
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
    pub fn source_reads(&self) -> u64 {
        self.reads
    }
    pub fn snapshot(
        &self,
        admission: &Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<PaintSnapshot> {
        self.source.cropped(self.mapping.clone(), admission, check)
    }
    pub fn tile_coords(&self) -> impl Iterator<Item = TileCoord> + '_ {
        self.mapping.tiles.keys().copied()
    }
    pub fn replay(
        &self,
        admission: &mut Admission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<RasterLayer> {
        check()?;
        let bounds = self.mapping.bounds();
        let mut output = RasterLayer::new(
            bounds.width() as u32,
            bounds.height() as u32,
            self.source.default_pixel(),
        );
        let reader = self.source.reader(admission, check)?;
        for coord in self.tile_coords() {
            let region = tile_rect(coord).intersect(&bounds).unwrap();
            output.with_tile_mut(coord, admission, |tile| -> Result<()> {
                tile.mark_all();
                for y in region.y0..region.y1 {
                    check()?;
                    for x in region.x0..region.x1 {
                        let color = reader.read_checked(
                            x + self.mapping.area.x0,
                            y + self.mapping.area.y0,
                            check,
                        )?;
                        let pixel = ((y % i64::from(TILE_EDGE)) * i64::from(TILE_EDGE)
                            + x % i64::from(TILE_EDGE))
                            as usize
                            * 4;
                        tile.pixels[pixel..pixel + 4].copy_from_slice(&color.0);
                    }
                }
                Ok(())
            })??;
        }
        check()?;
        Ok(output)
    }
}
fn tile_rect(coord: TileCoord) -> Rect {
    Rect::new(
        coord.tx * i64::from(TILE_EDGE),
        coord.ty * i64::from(TILE_EDGE),
        i64::from(TILE_EDGE),
        i64::from(TILE_EDGE),
    )
    .unwrap()
}
