//! Select classifier once, retaining the unrestricted monomorphized normal path.
use super::*;
pub(in crate::execute) fn classify(
    read: impl Fn(i64, i64) -> Result<Rgba>,
    allowed: Option<&dyn Fn(i64, i64) -> Result<bool>>,
    seed: (i64, i64),
    area: Rect,
    tolerance: u8,
    admission: &Admission,
    check: &dyn Fn() -> Result<()>,
) -> Result<paintcraft_raster::flood::AdmittedSpans> {
    if let Some(allowed) = allowed {
        paintcraft_raster::flood::region_from_source_masked_admitted_checked(
            read,
            allowed,
            seed,
            &area,
            tolerance,
            (
                admission.flood_scratch_limit(),
                admission.paint_metadata_scope(),
            ),
            check,
        )
    } else {
        paintcraft_raster::flood::region_from_source_masked_admitted_checked(
            read,
            |_, _| Ok(true),
            seed,
            &area,
            tolerance,
            (
                admission.flood_scratch_limit(),
                admission.paint_metadata_scope(),
            ),
            check,
        )
    }
}
