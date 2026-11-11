//! Ordered post-filter RGB intentions under the caller's original Base/W job.
use crate::spatial::WorkspaceAdmission;
use crate::*;
use paintcraft_core::types::resources::BudgetAccount;
use paintcraft_document::{Layer, StyleDescriptor, StyleInstance};
use paintcraft_effects::color_overlay::prepared::{self, PreparedColorOverlay};
use std::mem::size_of;

const OUTER_BYTES: u64 = 3072;
const STORED_WORK: u64 = 8192;
/// Combined live outer and Effects row controls; not a new workspace grant.
pub const CONTROL_BYTES: u64 = OUTER_BYTES + prepared::CONTROL_BYTES;
pub fn stored(layer: &Layer) -> bool {
    !layer.styles.is_empty()
}
fn missing() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Layer styles require an original workspace admission",
    )
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Layer style work/control allowance",
    )
}
fn original(admission: Option<&WorkspaceAdmission>) -> Result<&WorkspaceAdmission> {
    admission.ok_or_else(missing)
}
fn spend(a: &WorkspaceAdmission, units: u64) -> Result<()> {
    if let Some(noise) = crate::noise_work::current() {
        noise
            .for_workspace(a)?
            .reserve_components(crate::noise_work::WorkComponents {
                base: units,
                noise: 0,
            })
    } else {
        a.reserve_work(units)
    }
}
/// Defining style-owned controls. Successful queries allocate no payload/root.
/// # Errors
/// Refuses unsupported outer carrier layouts, never increases declared4096.
/// Effects validates its own inner1024 after its separately prepaid query;
/// this getter alone is not permission to execute the prepared kernel.
pub fn control_bytes() -> Result<u64> {
    use crate::noise_work::{NoiseAdmission, WorkComponents};
    let common = 2 * size_of::<BudgetAccount>()
        + 2 * size_of::<StyleInstance>()
        + 4 * size_of::<&Layer>()
        + 4 * size_of::<&mut [u8]>()
        + 4 * size_of::<usize>()
        + 4 * size_of::<&dyn Fn() -> Result<()>>()
        + 2 * size_of::<Result<&WorkspaceAdmission>>()
        + 2 * size_of::<Result<(&WorkspaceAdmission, BudgetAccount)>>()
        + 2 * size_of::<Result<PreparedColorOverlay>>()
        + 4 * size_of::<Result<u64>>()
        + 2 * size_of::<Option<&StyleInstance>>();
    let lookup = common
        + 2 * size_of::<WorkspaceAdmission>()
        + 2 * size_of::<Option<WorkspaceAdmission>>()
        + 2 * size_of::<Option<NoiseAdmission>>()
        + 2 * size_of::<Result<NoiseAdmission>>()
        + 2 * size_of::<WorkComponents>()
        + 4 * size_of::<Result<()>>();
    let query = common
        + 2 * size_of::<WorkspaceAdmission>()
        + 2 * size_of::<Result<prepared::Quote>>()
        + 2 * size_of::<Result<u64>>()
        + 4 * size_of::<Result<()>>()
        + 4 * size_of::<prepared::Quote>();
    let pure_query = common
        + 2 * size_of::<Rect>()
        + 12 * size_of::<u64>()
        + 4 * size_of::<Option<u64>>()
        + 4 * size_of::<Result<u64>>()
        + 2 * size_of::<usize>();
    let execution = common
        + 2 * size_of::<WorkspaceAdmission>()
        + 4 * size_of::<PreparedColorOverlay>()
        + 4 * size_of::<Rgba>()
        + 4 * size_of::<Result<()>>();
    if lookup.max(query).max(pure_query).max(execution) as u64 > OUTER_BYTES {
        return Err(limit());
    }
    Ok(CONTROL_BYTES)
}
/// Pure execution subtotal for the final output, excluding the caller-paid
/// planning query. Native256-tile cuts are bounded by the canonical64 grid.
/// This fact is not execution authority or an entire-job/grant forecast.
/// # Errors
/// For a nonempty stack, returns InputSchema for invalid geometry and
/// ResourceLimit on arithmetic overflow. Empty stacks preserve the legacy zero.
pub fn quote_work(layer: &Layer, output: Rect) -> Result<u64> {
    if !stored(layer) {
        return Ok(0);
    }
    if output.x0 < 0
        || output.y0 < 0
        || output.x1 <= output.x0
        || output.y1 <= output.y0
        || output.x1 > i64::from(layer.raster.width())
        || output.y1 > i64::from(layer.raster.height())
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Style output outside finite source",
        ));
    }
    let width = output.width() as u64;
    let height = output.height() as u64;
    let rows = (width + (output.x0 as u64 % 64))
        .div_ceil(64)
        .checked_mul(height)
        .ok_or_else(limit)?;
    let samples = width.checked_mul(height).ok_or_else(limit)?;
    let mut enabled = 0_u64;
    let mut index = 0_usize;
    while index < layer.styles.len() {
        if layer.styles.as_slice()[index].enabled() {
            enabled += 1;
        }
        index += 1;
    }
    let traversal = (layer.styles.len() as u64)
        .checked_mul(STORED_WORK)
        .and_then(|n| n.checked_mul(rows))
        .ok_or_else(limit)?;
    let arithmetic = rows
        .checked_mul(prepared::QUERY_WORK)
        .and_then(|n| n.checked_add(samples.checked_mul(prepared::PIXEL_WORK)?))
        .and_then(|n| n.checked_mul(enabled))
        .ok_or_else(limit)?;
    traversal.checked_add(arithmetic).ok_or_else(limit)
}
pub(crate) fn checked_work(
    layer: &Layer,
    output: Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<u64> {
    if !stored(layer) {
        return Ok(0);
    }
    let a = crate::spatial::current().ok_or_else(missing)?;
    let query = STORED_WORK
        .checked_mul(1 + layer.styles.len() as u64)
        .ok_or_else(limit)?;
    spend(&a, query)?;
    let mut controls = a.scope().account();
    controls.resize(CONTROL_BYTES)?;
    check()?;
    control_bytes()?;
    quote_work(layer, output)
}
fn begin<'a>(
    layer: &Layer,
    a: Option<&'a WorkspaceAdmission>,
    prepaid: bool,
) -> Result<(&'a WorkspaceAdmission, BudgetAccount)> {
    let a = original(a)?;
    if !prepaid {
        spend(
            a,
            STORED_WORK
                .checked_mul(layer.styles.len() as u64)
                .ok_or_else(limit)?,
        )?;
    }
    // Whole spatial plan prepaid the same subtotal before extraction. Scalar
    // entry prepays above before every control/helper formation.
    let mut controls = a.scope().account();
    controls.resize(CONTROL_BYTES)?;
    control_bytes()?;
    Ok((a, controls))
}
fn compile(
    style: &StyleInstance,
    n: usize,
    a: &WorkspaceAdmission,
    prepaid: bool,
) -> Result<PreparedColorOverlay> {
    if !prepaid {
        spend(a, prepared::QUERY_WORK)?;
    }
    let q = prepared::quote(n)?;
    if !prepaid {
        spend(a, q.work - prepared::QUERY_WORK)?;
    }
    match style.descriptor() {
        StyleDescriptor::ColorOverlayRgbV1(policy) => Ok(PreparedColorOverlay::new(policy)),
    }
}
pub(crate) fn pixel(
    layer: &Layer,
    pixel: &mut Rgba,
    a: Option<&WorkspaceAdmission>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    pixels(layer, std::slice::from_mut(pixel), a, false, check)
}
pub(crate) fn bytes(
    layer: &Layer,
    row: &mut [u8],
    a: Option<&WorkspaceAdmission>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    if !stored(layer) {
        return Ok(());
    }
    let (a, _controls) = begin(layer, a, false)?;
    for style in layer.styles.iter() {
        check()?;
        if style.enabled() {
            compile(style, row.len() / 4, a, false)?.bytes_in_place(row, check)?;
        }
    }
    Ok(())
}
pub(crate) fn pixels(
    layer: &Layer,
    row: &mut [Rgba],
    a: Option<&WorkspaceAdmission>,
    prepaid: bool,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    if !stored(layer) {
        return Ok(());
    }
    let (a, _controls) = begin(layer, a, prepaid)?;
    for style in layer.styles.iter() {
        check()?;
        if style.enabled() {
            compile(style, row.len(), a, prepaid)?.pixels_in_place(row, check)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "styles/tests.rs"]
mod tests;
