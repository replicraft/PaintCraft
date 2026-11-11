use super::*;
use crate::{
    Rect,
    spatial::{LayerAppearancePlan, TiledQuote, WorkspaceAdmission},
};
use paintcraft_document::{Document, Layer};
/// New-profile projection only. Original quote structures and fields are intact.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScopedLayerQuote {
    pub bytes: u64,
    pub tiles: u64,
    pub work: WorkComponents,
}
fn each(rect: Rect, mut visit: impl FnMut(Rect) -> Result<()>) -> Result<()> {
    if rect.x1 <= rect.x0 || rect.y1 <= rect.y0 {
        return Err(limit());
    }
    let mut y = rect.y0;
    while y < rect.y1 {
        let h = 64.min(rect.y1 - y);
        let mut x = rect.x0;
        while x < rect.x1 {
            let w = 64.min(rect.x1 - x);
            visit(Rect::new(x, y, w, h).ok_or_else(limit)?)?;
            x += w;
        }
        y += h;
    }
    Ok(())
}
fn validate_local(layer: &Layer, local: Rect) -> Result<()> {
    if local.x0 < 0
        || local.y0 < 0
        || local.x1 <= local.x0
        || local.y1 <= local.y0
        || local.x1 > i64::from(layer.raster.width())
        || local.y1 > i64::from(layer.raster.height())
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Scoped appearance lies outside finite source",
        ));
    }
    Ok(())
}
fn components(total: u64, noise: u64) -> Result<WorkComponents> {
    let value = WorkComponents {
        base: total.checked_sub(noise).ok_or_else(limit)?,
        noise,
    };
    if value.checked_total()? != total {
        return Err(limit());
    }
    Ok(value)
}
fn plan_components(
    plan: &LayerAppearancePlan<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<WorkComponents> {
    components(plan.quote().work, plan.noise_work(check)?)
}
pub fn quote_layer(
    layer: &Layer,
    local: Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<ScopedLayerQuote> {
    validate_local(layer, local)?;
    let mut quote = ScopedLayerQuote::default();
    each(local, |tile| {
        let plan = LayerAppearancePlan::new(layer, tile, check)?;
        quote.bytes = quote.bytes.max(plan.quote().bytes);
        quote.work = quote.work.checked_add(plan_components(&plan, check)?)?;
        quote.tiles = quote.tiles.checked_add(1).ok_or_else(limit)?;
        Ok(())
    })?;
    Ok(quote)
}
pub fn quote_direct(
    layer: &Layer,
    local: Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<WorkComponents> {
    if !crate::direct::active(layer) {
        return Ok(WorkComponents::default());
    }
    validate_local(layer, local)?;
    let mut quote = WorkComponents::default();
    each(local, |tile| {
        let plan = LayerAppearancePlan::new(layer, tile, check)?;
        quote = quote.checked_add(components(
            plan.direct_work(check)?,
            plan.noise_work(check)?,
        )?)?;
        Ok(())
    })?;
    Ok(quote)
}
pub(crate) fn check_plan(
    plan: &LayerAppearancePlan<'_>,
    base: &WorkspaceAdmission,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let caller = current_for(base)?;
    if plan.quote().bytes > caller.limits().bytes {
        return Err(limit());
    }
    caller.check_available(plan_components(plan, check)?)
}
pub(crate) fn reserve_plan(
    plan: &LayerAppearancePlan<'_>,
    base: &WorkspaceAdmission,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let caller = current_for(base)?;
    if plan.quote().bytes > caller.limits().bytes {
        return Err(limit());
    }
    caller.reserve_components(plan_components(plan, check)?)
}
pub(crate) fn preflight(
    doc: &Document,
    region: Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<TiledQuote> {
    let caller = required()?;
    let mut quote = ScopedLayerQuote::default();
    for layer in doc
        .layers_in_order()
        .filter(|l| l.visible && l.opacity != 0)
    {
        check()?;
        let Some(area) = region.intersect(&layer.extent()) else {
            continue;
        };
        let local = area
            .translated(-layer.origin.0, -layer.origin.1)
            .ok_or_else(limit)?;
        if crate::spatial::active(layer) {
            let q = quote_layer(layer, local, check)?;
            quote.bytes = quote.bytes.max(q.bytes);
            quote.work = quote.work.checked_add(q.work)?;
            quote.tiles = quote.tiles.checked_add(q.tiles).ok_or_else(limit)?;
        } else {
            if crate::direct::active(layer) {
                quote.work =
                    quote
                        .work
                        .checked_add(quote_direct(layer, local, check)?.checked_mul(
                            if crate::layer_mask::active(layer) {
                                1
                            } else {
                                2
                            },
                        )?)?;
            }
            quote.work = quote.work.checked_add(WorkComponents {
                base: crate::styles::checked_work(layer, local, check)?,
                noise: 0,
            })?;
            quote.work = quote.work.checked_add(WorkComponents {
                base: crate::layer_mask::work(layer, local.area() as u64)?,
                noise: 0,
            })?;
        }
    }
    if quote.bytes > caller.limits().bytes {
        return Err(limit());
    }
    caller.check_available(quote.work)?;
    Ok(TiledQuote {
        bytes: quote.bytes,
        tiles: quote.tiles,
        work: quote.work.checked_total()?,
    })
}
