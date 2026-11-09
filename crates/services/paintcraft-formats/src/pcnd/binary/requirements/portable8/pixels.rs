//! SAME Plan/original decoder realms captured once; no public view recipes.
use super::*;
use paintcraft_raster::{
    EncodedTilePayload, PortableLayerConfig, PortablePixelEncoding, RasterLayer,
    ReservedDecodeAdmission, portable_pixel_geometry_bytes,
};
/// Standalone primitive. Caller retains ORIGINAL full simultaneous M union through
/// every returned Raster/Tile/Weak alias. Future R3 forbids such alias escapes.
/// ```compile_fail
/// use paintcraft_formats::pcnd::{PortablePixelMaterializer8, PortableBlockView};
/// fn forged(m: &mut PortablePixelMaterializer8<'_, '_>, v: PortableBlockView<'_>) {
///     let _ = m.block(v);
/// }
/// ```
#[repr(C)]
pub struct PortablePixelMaterializer8<'p, 'a> {
    plan: &'p PortableSource8Plan<'a>,
    admission: &'p mut ReservedDecodeAdmission,
    scratch: &'p BudgetScope,
    metadata: &'p BudgetAccount,
    _control: BudgetAccount,
}
impl std::fmt::Debug for PortablePixelMaterializer8<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortablePixelMaterializer8")
            .finish_non_exhaustive()
    }
}
impl<'a> PortableSource8Plan<'a> {
    pub fn pixel_materializer<'p>(
        &'p self,
        admission: &'p mut ReservedDecodeAdmission,
        scratch: &'p BudgetScope,
        metadata: &'p BudgetAccount,
    ) -> Result<PortablePixelMaterializer8<'p, 'a>> {
        self.meter.debit(64)?;
        let bytes = [
            size_of::<PortablePixelMaterializer8<'static, 'static>>(),
            size_of::<PortableLayerConfig>(),
            size_of::<wire::Cursor<'static, 'static>>(),
            size_of::<PortableLayerHeader<'static>>(),
            size_of::<PortableLayerCursor<'static, 'static>>(),
            size_of::<PortableBlockView<'static>>(),
            2 * size_of::<usize>(),
            usize::try_from(portable_pixel_geometry_bytes()).map_err(|_| bound())?,
            size_of::<[u64; 8]>(),
            4 * size_of::<usize>(),
        ]
        .into_iter()
        .try_fold(0usize, |n, v| n.checked_add(v).ok_or_else(bound))?;
        let mut control = scratch.account();
        control.resize(bytes as u64)?;
        self.meter.step(16)?;
        Ok(PortablePixelMaterializer8 {
            plan: self,
            admission,
            scratch,
            metadata,
            _control: control,
        })
    }
}
impl PortablePixelMaterializer8<'_, '_> {
    pub fn layer(&mut self, index: usize) -> Result<RasterLayer> {
        let plan = self.plan;
        let cursor = plan.layer(index)?;
        let header = cursor.header()?;
        plan.meter.step(32)?;
        let records = cursor.block_count();
        let config = PortableLayerConfig {
            width: header.width,
            height: header.height,
            default: header.default,
            records,
        };
        let spend = |n| plan.meter.debit(n);
        let check = || {
            plan.meter.checks.set(add(plan.meter.checks.get(), 1)?);
            (plan.meter.check)()
        };
        let mut builder = self.admission.portable_layer_builder_checked(
            config,
            self.scratch,
            self.metadata,
            &check,
            &spend,
        )?;
        for i in 0..records {
            let view = plan.block(index, i)?;
            plan.meter.step(32)?;
            let x = view
                .x
                .checked_mul(256)
                .filter(|x| *x < header.width)
                .ok_or_else(invalid)?;
            let y = view
                .y
                .checked_mul(256)
                .filter(|y| *y < header.height)
                .ok_or_else(invalid)?;
            let decoded = (header.width - x).min(256) * (header.height - y).min(256) * 4;
            if view.decoded_bytes != decoded {
                return Err(invalid());
            }
            let encoding = match view.encoding {
                1 => PortablePixelEncoding::Raw,
                2 => PortablePixelEncoding::Runs,
                4 => PortablePixelEncoding::Lz4,
                5 => PortablePixelEncoding::RunsLz4,
                3 => return Err(unsupported("portable Source8 excludes native blocks")),
                _ => return Err(invalid()),
            };
            builder = builder.block(
                view.x,
                view.y,
                EncodedTilePayload::new(encoding, view.stored_bytes()),
            )?;
        }
        let raster = builder.finish()?;
        plan.meter.step(32)?;
        if raster.resident_tiles() != records {
            return Err(invalid());
        }
        Ok(raster)
    }
}
#[cfg(test)]
mod tests;
