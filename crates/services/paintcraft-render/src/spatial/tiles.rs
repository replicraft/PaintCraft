use super::*;
pub(crate) const EDGE: i64 = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TiledQuote {
    /// Maximum intermediate bytes of one serial tile/layer evaluation.
    pub bytes: u64,
    /// Aggregate work for all tiles, including repeated halos.
    pub work: u64,
    pub tiles: u64,
}

fn each(rect: Rect, mut visit: impl FnMut(Rect) -> Result<()>) -> Result<()> {
    pixels(rect)?;
    let mut y = rect.y0;
    while y < rect.y1 {
        let height = EDGE.min(rect.y1 - y);
        let mut x = rect.x0;
        while x < rect.x1 {
            let width = EDGE.min(rect.x1 - x);
            visit(
                Rect::new(x, y, width, height).ok_or_else(|| invalid("Spatial tile overflows"))?,
            )?;
            x += width;
        }
        y += height;
    }
    Ok(())
}

/// Exact aggregate tile work and conservative serial peak, without allocation.
pub fn quote_layer(
    layer: &Layer,
    local: Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<TiledQuote> {
    let mut quote = TiledQuote::default();
    each(local, |tile| {
        let plan = LayerAppearancePlan::new(layer, tile, check)?;
        quote.bytes = quote.bytes.max(plan.quote().bytes);
        quote.work = quote
            .work
            .checked_add(plan.quote().work)
            .ok_or_else(|| limit("Spatial tiled work overflows"))?;
        quote.tiles = quote
            .tiles
            .checked_add(1)
            .ok_or_else(|| limit("Spatial tile count overflows"))?;
        Ok(())
    })?;
    Ok(quote)
}

/// Additional direct arithmetic across actual64-native tiles, including halos.
/// Zero without callbacks/allocation for old-only layers.
pub fn quote_direct_work(
    layer: &Layer,
    local: Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<u64> {
    if !crate::direct::active(layer) {
        return Ok(0);
    }
    let mut work = 0u64;
    each(local, |tile| {
        let next = LayerAppearancePlan::new(layer, tile, check)?.direct_work(check)?;
        work = work
            .checked_add(next)
            .ok_or_else(|| limit("Direct tiled work overflows"))?;
        Ok(())
    })?;
    Ok(work)
}

pub(crate) fn visit(
    layer: &Layer,
    local: Rect,
    check: &dyn Fn() -> Result<()>,
    mut consume: impl FnMut(Rect, &[Rgba]) -> Result<()>,
) -> Result<()> {
    let admission = context::current().ok_or_else(|| {
        Error::new(
            ErrorCode::CapabilityUnsupported,
            "Spatial rendering requires an explicit shared workspace admission",
        )
    })?;
    let quote = quote_layer(layer, local, check)?;
    if quote.bytes > admission.bytes_limit || quote.work > admission.work_limit {
        return Err(limit("Spatial tiled appearance exceeds its request limit"));
    }
    each(local, |tile| {
        check()?;
        let plan = LayerAppearancePlan::new(layer, tile, check)?;
        plan.with_pixels(&admission, check, |pixels| consume(tile, pixels))
    })
}

pub(crate) fn preflight(
    doc: &paintcraft_document::Document,
    region: Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<TiledQuote> {
    crate::direct::require_document(doc)?;
    let mut direct = 0u64;
    for layer in doc
        .layers_in_order()
        .filter(|l| l.visible && l.opacity != 0 && !active(l) && crate::direct::active(l))
    {
        if let Some(area) = region.intersect(&layer.extent()) {
            let local = area
                .translated(-layer.origin.0, -layer.origin.1)
                .ok_or_else(|| invalid("Direct source coordinates overflow"))?;
            direct = direct
                .checked_add(
                    quote_direct_work(layer, local, check)?
                        .checked_mul(2)
                        .ok_or_else(|| limit("Direct fallback work overflows"))?,
                )
                .ok_or_else(|| limit("Direct request work overflows"))?;
        }
    }
    let mut total = TiledQuote::default();
    for layer in doc
        .layers_in_order()
        .filter(|layer| layer.visible && layer.opacity != 0 && active(layer))
    {
        check()?;
        let Some(area) = region.intersect(&layer.extent()) else {
            continue;
        };
        let local = area
            .translated(-layer.origin.0, -layer.origin.1)
            .ok_or_else(|| invalid("Spatial source coordinates overflow"))?;
        let quote = quote_layer(layer, local, check)?;
        total.bytes = total.bytes.max(quote.bytes);
        total.work = total
            .work
            .checked_add(quote.work)
            .ok_or_else(|| limit("Spatial request work overflows"))?;
        total.tiles = total
            .tiles
            .checked_add(quote.tiles)
            .ok_or_else(|| limit("Spatial request tile count overflows"))?;
    }
    if total.tiles != 0 || direct != 0 {
        let admission = context::current().ok_or_else(|| {
            Error::new(
                ErrorCode::CapabilityUnsupported,
                "Spatial rendering requires an explicit shared workspace admission",
            )
        })?;
        let prior = if doc.layers.values().any(crate::direct::active) {
            admission.work_used()
        } else {
            0
        };
        let required = total
            .work
            .checked_add(direct)
            .and_then(|w| w.checked_add(prior))
            .ok_or_else(|| limit("Direct cumulative request work overflows"))?;
        if total.bytes > admission.bytes_limit || required > admission.work_limit {
            return Err(limit("Spatial request exceeds its workspace or work limit"));
        }
    }
    Ok(total)
}
