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
) -> Result<Vec<Span>> {
    if let Some(allowed) = allowed {
        paintcraft_raster::flood::region_from_source_masked_checked(
            read,
            allowed,
            seed,
            &area,
            tolerance,
            admission.flood_scratch_limit(),
            check,
        )
    } else {
        paintcraft_raster::flood::region_from_source_checked(
            read,
            seed,
            &area,
            tolerance,
            admission.flood_scratch_limit(),
            check,
        )
    }
}
