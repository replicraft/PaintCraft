//! Pure same-ROI quote; no counter/owner/allocation is created here.
use super::*;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Quote {
    pub control_bytes: u64,
    pub opaque_samples: u64,
    pub general_samples: u64,
    pub extra_work: u64,
}
fn quote(doc: &Document, region: Rect, c: Context) -> Result<Quote> {
    let mut q = Quote {
        control_bytes: control_quote(),
        ..Quote::default()
    };
    for (i, l) in doc.layers_in_order().enumerate() {
        if !l.visible || l.opacity == 0 {
            continue;
        }
        let Some(area) = region.intersect(&l.extent()) else {
            continue;
        };
        let n = u64::try_from(area.area()).map_err(|_| limit())?;
        match kind_at(l.blend_mode, i, c) {
            Kind::Normal => {}
            Kind::Opaque(_) => {
                q.opaque_samples = q.opaque_samples.checked_add(n).ok_or_else(limit)?
            }
            Kind::General(_) => {
                q.general_samples = q.general_samples.checked_add(n).ok_or_else(limit)?
            }
        }
    }
    q.extra_work = q
        .opaque_samples
        .checked_mul(OPAQUE_EXTENSION_WORK)
        .and_then(|v| v.checked_add(q.general_samples.checked_mul(GENERAL_WORK)?))
        .ok_or_else(limit)?;
    Ok(q)
}
pub fn quote_document(doc: &Document, region: Rect) -> Result<Quote> {
    if region.x0 < 0
        || region.y0 < 0
        || region.x1 > i64::from(doc.canvas.0)
        || region.y1 > i64::from(doc.canvas.1)
        || region.x1 <= region.x0
        || region.y1 <= region.y0
    {
        return Err(Error::new(ErrorCode::InputSchema, "CPU blend quote region"));
    }
    quote(doc, region, context(doc)?)
}
pub(super) fn preflight(
    doc: &Document,
    region: Rect,
    c: Context,
    target: Target,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let q = quote(doc, region, c)?;
    let base = crate::spatial::current().ok_or_else(missing)?;
    let programs = match target {
        Target::Cpu => crate::prepared::quote_cpu_document_programs(doc, check)?,
        #[cfg(feature = "gpu")]
        Target::Gpu => crate::prepared::quote_gpu_document_programs(doc, check)?,
    };
    let compile = if base.has_prepared_programs() {
        0
    } else {
        programs.compile_work
    };
    let retained = if base.has_prepared_programs() {
        base.prepared_retained_bytes()
    } else {
        programs.retained_bytes
    };
    let mut spatial_peak = 0u64;
    let mut total = crate::noise_work::WorkComponents {
        base: q.extra_work.checked_add(compile).ok_or_else(limit)?,
        noise: 0,
    };
    for layer in doc
        .layers_in_order()
        .filter(|l| l.visible && l.opacity != 0)
    {
        let Some(area) = region.intersect(&layer.extent()) else {
            continue;
        };
        let local = area
            .translated(-layer.origin.0, -layer.origin.1)
            .ok_or_else(limit)?;
        if crate::spatial::active(layer) {
            let spatial = crate::noise_work::quote_layer(layer, local, check)?;
            spatial_peak = spatial_peak.max(spatial.bytes);
            total = total.checked_add(spatial.work)?;
        } else {
            let mask = crate::layer_mask::quote_work(layer, local)?;
            total = total.checked_add(
                crate::noise_work::quote_direct(layer, local, check)?.checked_mul(
                    if crate::layer_mask::active(layer) {
                        1
                    } else {
                        2
                    },
                )?,
            )?;
            total = total.checked_add(crate::noise_work::WorkComponents {
                base: mask,
                noise: 0,
            })?;
        }
    }
    let render_peak = spatial_peak.checked_add(retained).ok_or_else(limit)?;
    let prepare_peak = if base.has_prepared_programs() {
        0
    } else {
        programs.preparation_peak_bytes
    };
    if render_peak.max(prepare_peak) > base.bytes_limit() {
        return Err(limit());
    }
    if crate::noise_work::current().is_some() {
        crate::noise_work::current_for(&base)?.check_available(total)?;
    } else if total.noise != 0 {
        crate::noise_work::required()?;
        return Err(missing());
    } else if base
        .work_used()
        .checked_add(total.base)
        .filter(|n| *n <= base.work_limit())
        .is_none()
    {
        return Err(limit());
    }
    Ok(())
}

// Private validated-key route only. Public preflight remains byte-exact above.
pub(super) fn preflight_prepared(
    doc: &Document,
    region: Rect,
    c: Context,
    retained: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let q = quote(doc, region, c)?;
    preflight_for_quote(doc, region, retained, q, check)
}
// Mechanical shared tail: old prepared preflight and seeded adjusted quote use
// the same actual stage/point/own-mask/minimum validation and action order.
pub(super) fn preflight_for_quote(
    doc: &Document,
    region: Rect,
    retained: u64,
    q: Quote,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let base = crate::spatial::current().ok_or_else(missing)?;
    let mut spatial_peak = 0u64;
    let mut total = crate::noise_work::WorkComponents {
        base: q.extra_work,
        noise: 0,
    };
    for layer in doc
        .layers_in_order()
        .filter(|l| l.visible && l.opacity != 0)
    {
        let Some(area) = region.intersect(&layer.extent()) else {
            continue;
        };
        let local = area
            .translated(-layer.origin.0, -layer.origin.1)
            .ok_or_else(limit)?;
        if crate::spatial::active(layer) {
            let spatial = crate::noise_work::quote_layer(layer, local, check)?;
            spatial_peak = spatial_peak.max(spatial.bytes);
            total = total.checked_add(spatial.work)?;
        } else {
            let mask = crate::layer_mask::quote_work(layer, local)?;
            total = total.checked_add(
                crate::noise_work::quote_direct(layer, local, check)?.checked_mul(
                    if crate::layer_mask::active(layer) {
                        1
                    } else {
                        2
                    },
                )?,
            )?;
            total = total.checked_add(crate::noise_work::WorkComponents {
                base: mask,
                noise: 0,
            })?;
        }
    }
    let render_peak = spatial_peak.checked_add(retained).ok_or_else(limit)?;
    if render_peak > base.bytes_limit() {
        return Err(limit());
    }
    if crate::noise_work::current().is_some() {
        crate::noise_work::current_for(&base)?.check_available(total)?;
    } else if total.noise != 0 {
        crate::noise_work::required()?;
        return Err(missing());
    } else if base
        .work_used()
        .checked_add(total.base)
        .filter(|n| *n <= base.work_limit())
        .is_none()
    {
        return Err(limit());
    }
    Ok(())
}

// Called only by the new fresh-output gateway with its private matching proof.
pub(super) fn seeded_quote(
    doc: &Document,
    region: Rect,
    c: Context,
    seed: &super::seed::Seed<'_>,
) -> Result<Quote> {
    seed.require_matches(doc, region)?;
    let mut q = quote(doc, region, c)?;
    let n = seed.samples()?;
    q.general_samples = q.general_samples.checked_sub(n).ok_or_else(limit)?;
    q.extra_work = q
        .opaque_samples
        .checked_mul(OPAQUE_EXTENSION_WORK)
        .and_then(|n| n.checked_add(q.general_samples.checked_mul(GENERAL_WORK)?))
        .ok_or_else(limit)?;
    Ok(q)
}
