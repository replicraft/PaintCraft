//! New Source7 totals; never alters Source6 quote structs or debits.
use super::*;
use paintcraft_core::types::Rect;
use paintcraft_document::Layer;
use paintcraft_render::noise_work;
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TileQuote {
    pub work: WorkComponents,
    /// Trusted non-renderer work plus admitted mask upper-minus-actual margin.
    pub other: u64,
    pub mask_actual: u64,
    pub mask_upper: u64,
    pub peak: u64,
}
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| resource("Source7 work overflow"))
}
fn upper_mask(layer: &Layer, local: Rect) -> Result<(u64, u64)> {
    let actual = paintcraft_render::layer_mask::quote_work(layer, local)?;
    let Some(mask) = layer.mask.as_ref().filter(|m| m.enabled()) else {
        return Ok((0, 0));
    };
    let stats = mask.coverage().stats();
    let bound = pcnd::layer_appearance::coverage_decode_quote_checked(stats.bands, stats.runs)?;
    let retained = mask
        .coverage()
        .storage_usage()
        .map_or(0, |(_, bytes)| bytes);
    if retained > bound {
        return Err(resource("Source7 own-mask retained bound mismatch"));
    }
    let bits = u64::from(u64::BITS - bound.leading_zeros());
    let samples = u64::try_from(local.area()).map_err(|_| resource("Source7 mask area"))?;
    let upper = samples
        .checked_mul(48 + 2 * bits)
        .filter(|value| *value >= actual)
        .ok_or_else(|| resource("Source7 mask upper work mismatch"))?;
    Ok((actual, upper))
}
pub(super) fn tile_quote(
    doc: &Document,
    tile: Rect,
    resolve: u64,
    policy: SlicePolicy,
    check: &dyn Fn() -> Result<()>,
) -> Result<TileQuote> {
    let mut quote = TileQuote {
        other: add(
            u64::try_from(tile.area()).map_err(|_| resource("Source7 tile area"))?,
            resolve,
        )?,
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
        let dx = layer
            .origin
            .0
            .checked_neg()
            .ok_or_else(|| bad("Source7 local x"))?;
        let dy = layer
            .origin
            .1
            .checked_neg()
            .ok_or_else(|| bad("Source7 local y"))?;
        let local = area
            .translated(dx, dy)
            .ok_or_else(|| bad("Source7 local coordinates"))?;
        let q = noise_work::quote_layer(layer, local, check)?;
        quote.peak = quote.peak.max(q.bytes);
        let (actual, upper) = upper_mask(layer, local)?;
        quote.mask_actual = add(quote.mask_actual, actual)?;
        quote.mask_upper = add(quote.mask_upper, upper)?;
        quote.other = add(quote.other, area.area() as u64)?;
        quote.other = add(
            quote.other,
            upper
                .checked_sub(actual)
                .ok_or_else(|| resource("Source7 mask margin"))?,
        )?;
        if paintcraft_render::spatial::active(layer) {
            // Whole spatial plan includes its final own-mask actual pass once.
            deferred = deferred.checked_add(q.work)?;
        } else {
            // Enabled own mask refuses uniform proof BEFORE any point work.
            let passes = if layer.mask.as_ref().is_some_and(|m| m.enabled()) {
                1
            } else {
                2
            };
            let all = q.work.checked_mul(passes)?;
            let direct = noise_work::quote_direct(layer, local, check)?.checked_mul(passes)?;
            if all.noise != direct.noise {
                return Err(resource("Source7 direct Noise subtotal"));
            }
            let other = all
                .base
                .checked_sub(direct.base)
                .and_then(|n| n.checked_sub(actual))
                .ok_or_else(|| resource("Source7 trusted mask subtotal"))?;
            quote.other = add(quote.other, other)?;
            deferred = deferred.checked_add(direct)?.checked_add(WorkComponents {
                base: actual,
                noise: 0,
            })?;
        }
    }
    quote.work = deferred.checked_add(WorkComponents {
        base: quote.other,
        noise: 0,
    })?;
    policy.check(quote.work)?;
    Ok(quote)
}

pub(super) const PATCH: u32 = 64;
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct FrameQuote {
    pub work: WorkComponents,
    pub other: u64,
    pub mask_actual: u64,
    pub mask_upper: u64,
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
pub(super) fn frame_quote(
    doc: &Document,
    request: DisplayRequest,
    policy: SlicePolicy,
    job: &OpenJob,
    render: bool,
    check: &dyn Fn() -> Result<()>,
) -> Result<FrameQuote> {
    let mut result = FrameQuote::default();
    let [w, h] = request.size();
    if u64::from(w) * u64::from(h) > OUTPUT_PIXELS_MAX {
        return Err(resource("Source7 output exceeds4MP"));
    }
    // Quote once and prepay the same bounded metadata checks for step revalidation.
    // render:false quotes only; it never allocates a frame or prepared owner.
    let charged = || {
        job.spend_control(control::CHECK_SLOTS * if render { 2 } else { 1 })?;
        check()
    };
    let mut row = 0;
    while row < h {
        let mut column = 0;
        while column < w {
            let area = patch(request, column, row);
            let mut y = area.y0;
            while y < area.y1 {
                let mut x = area.x0;
                while x < area.x1 {
                    charged()?;
                    let tile = Rect {
                        x0: x,
                        y0: y,
                        x1: (x + policy.native_edge()).min(area.x1),
                        y1: (y + policy.native_edge()).min(area.y1),
                    };
                    let next =
                        tile_quote(doc, tile, resolve(request, area, tile), policy, &charged)?;
                    result.work = result.work.checked_add(next.work)?;
                    result.other = add(result.other, next.other)?;
                    result.mask_actual = add(result.mask_actual, next.mask_actual)?;
                    result.mask_upper = add(result.mask_upper, next.mask_upper)?;
                    result.peak = result.peak.max(next.peak);
                    result.tiles = add(result.tiles, 1)?;
                    job.check_available(result.work)?;
                    x = tile.x1;
                }
                y = (y + policy.native_edge()).min(area.y1);
            }
            column += PATCH;
        }
        row += PATCH;
    }
    Ok(result)
}
pub(super) fn value(work: WorkComponents, mask_actual: u64, mask_upper: u64) -> Result<Value> {
    let margin = mask_upper
        .checked_sub(mask_actual)
        .ok_or_else(|| resource("Source7 mask work margin"))?;
    Ok(Value::object([
        ("baseWork", Value::str(work.base.to_string())),
        ("noiseWork", Value::str(work.noise.to_string())),
        ("totalWork", Value::str(work.checked_total()?.to_string())),
        ("maskWorkActual", Value::str(mask_actual.to_string())),
        ("maskWorkUpper", Value::str(mask_upper.to_string())),
        ("maskWorkMargin", Value::str(margin.to_string())),
    ]))
}
