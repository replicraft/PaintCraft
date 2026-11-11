//! Source8-only ordered preflight; legacy tiles::preflight remains exact.
use super::*;
pub(crate) fn preflight(
    facts: &crate::blend::rooted::RootFacts<'_>,
    region: Rect,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<TiledQuote> {
    let doc = facts.document();
    facts.require_current()?;
    if facts.has_coordinate() {
        spend(facts.pass_work(1)?)?;
        return crate::noise_work::preflight(doc, region, check);
    }
    spend(facts.pass_work(1)?)?;
    let mut direct = 0u64;
    for layer in doc.layers_in_order().filter(|l| {
        l.visible
            && l.opacity != 0
            && !active(l)
            && (crate::direct::active(l) || crate::layer_mask::active(l))
    }) {
        if let Some(area) = region.intersect(&layer.extent()) {
            let local = area
                .translated(-layer.origin.0, -layer.origin.1)
                .ok_or_else(|| invalid("Direct source coordinates overflow"))?;
            // Styles run only after uniform proof, so they are never doubled
            // with the legacy filter candidate-then-fallback bound below.
            direct = direct
                .checked_add(crate::styles::checked_work(layer, local, check)?)
                .ok_or_else(|| limit("Style request work overflows"))?;
            direct = direct
                .checked_add(crate::layer_mask::work(layer, local.area() as u64)?)
                .ok_or_else(|| limit("Layer mask request work overflows"))?;
            direct = direct
                .checked_add(
                    quote_direct_work(layer, local, check)?
                        .checked_mul(if crate::layer_mask::active(layer) {
                            1
                        } else {
                            2
                        })
                        .ok_or_else(|| limit("Direct fallback work overflows"))?,
                )
                .ok_or_else(|| limit("Direct request work overflows"))?;
        }
    }
    spend(facts.pass_work(1)?)?;
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
        let prior = if facts.has_direct() || facts.has_mask() {
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
