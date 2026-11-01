use super::*;
use paintcraft_core::types::Rect;
use paintcraft_render::noise_work;
pub(super) const PATCH: u32 = 64;
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct FrameQuote {
    pub work: WorkComponents,
    pub other: u64,
    pub peak: u64,
    pub tiles: u64,
}
pub(super) fn patch(request: DisplayRequest, column: u32, row: u32) -> Rect {
    let step = i64::from(request.step());
    let r = request.region();
    Rect {
        x0: r.x0 + i64::from(column) * step,
        y0: r.y0 + i64::from(row) * step,
        x1: (r.x0 + i64::from(column + PATCH) * step).min(r.x1),
        y1: (r.y0 + i64::from(row + PATCH) * step).min(r.y1),
    }
}
pub(super) fn resolve(request: DisplayRequest, area: Rect, tile: Rect) -> u64 {
    if tile.x1 != area.x1 || tile.y1 != area.y1 {
        return 0;
    }
    let step = i64::from(request.step());
    (((area.width() + step - 1) / step) * ((area.height() + step - 1) / step)) as u64
}
pub(super) fn tile_quote(
    doc: &Document,
    tile: Rect,
    resolve: u64,
    policy: SlicePolicy,
    check: &dyn Fn() -> Result<()>,
) -> Result<FrameQuote> {
    let mut quote = FrameQuote {
        other: u64::try_from(tile.area())
            .map_err(|_| resource("Tile area"))?
            .checked_add(resolve)
            .ok_or_else(|| resource("Resolve work"))?,
        tiles: 1,
        ..Default::default()
    };
    let mut deferred = WorkComponents::default();
    for layer in doc
        .layers_in_order()
        .filter(|l| l.visible && l.opacity != 0)
    {
        check()?;
        let Some(area) = tile.intersect(&layer.extent()) else {
            continue;
        };
        let local = area
            .translated(-layer.origin.0, -layer.origin.1)
            .ok_or_else(|| bad("Layer coordinates"))?;
        let q = noise_work::quote_layer(layer, local, check)?;
        quote.peak = quote.peak.max(q.bytes);
        quote.other = quote
            .other
            .checked_add(area.area() as u64)
            .ok_or_else(|| resource("Composition work"))?;
        if paintcraft_render::spatial::active(layer) {
            // Spatial evaluator reserves the full typed pair once, after extraction admission.
            deferred = deferred.checked_add(q.work)?;
        } else {
            let all = q.work.checked_mul(2)?;
            let direct = noise_work::quote_direct(layer, local, check)?.checked_mul(2)?;
            // Noise has no hidden non-direct arithmetic component. Only trusted
            // independently derived subtotal is excluded from initial base debit.
            if all.noise != direct.noise {
                return Err(resource("Noise subtotal mismatch"));
            }
            quote.other = quote
                .other
                .checked_add(
                    all.base
                        .checked_sub(direct.base)
                        .ok_or_else(|| resource("Direct subtotal mismatch"))?,
                )
                .ok_or_else(|| resource("Point control work"))?;
            deferred = deferred.checked_add(direct)?;
        }
    }
    quote.work = deferred.checked_add(WorkComponents {
        base: quote.other,
        noise: 0,
    })?;
    policy.check(quote.work)?;
    Ok(quote)
}
pub(super) fn each(
    request: DisplayRequest,
    policy: SlicePolicy,
    mut visit: impl FnMut(Rect, u64) -> Result<()>,
) -> Result<()> {
    let [w, h] = request.size();
    let mut row = 0;
    while row < h {
        let mut column = 0;
        while column < w {
            let area = patch(request, column, row);
            let mut y = area.y0;
            while y < area.y1 {
                let mut x = area.x0;
                while x < area.x1 {
                    let tile = Rect {
                        x0: x,
                        y0: y,
                        x1: (x + policy.native_edge()).min(area.x1),
                        y1: (y + policy.native_edge()).min(area.y1),
                    };
                    visit(tile, resolve(request, area, tile))?;
                    x = tile.x1;
                }
                y = (y + policy.native_edge()).min(area.y1);
            }
            column += PATCH;
        }
        row += PATCH;
    }
    Ok(())
}
pub(super) fn frame_quote(
    doc: &Document,
    request: DisplayRequest,
    profile: wire::Profile,
    policy: SlicePolicy,
    check: &dyn Fn() -> Result<()>,
) -> Result<FrameQuote> {
    let mut total = FrameQuote::default();
    each(request, policy, |tile, resolve| {
        check()?;
        let q = tile_quote(doc, tile, resolve, policy, check)?;
        total.work = total.work.checked_add(q.work)?;
        total.other = total
            .other
            .checked_add(q.other)
            .ok_or_else(|| resource("Frame composition sum"))?;
        total.tiles = total
            .tiles
            .checked_add(1)
            .ok_or_else(|| resource("Frame tile count"))?;
        total.peak = total.peak.max(q.peak);
        profile.workload.check(total.work)?;
        Ok(())
    })?;
    Ok(total)
}
