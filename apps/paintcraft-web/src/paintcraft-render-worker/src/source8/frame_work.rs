//! Allocation-free structural forecast attached only to the actual closed source.
use super::*;
use paintcraft_core::types::{LayerBlendMode, Rect, filters::Descriptor};
use paintcraft_document::{Document, Layer};
use paintcraft_render::{
    prepared::DocumentProgramsQuote,
    spatial::{LayerAppearancePlan, WorkspaceAdmission},
};
const WORK: usize = 0;
const SAMPLES: usize = 1;
const SELECT: usize = 2;
const EVAL: usize = 3;
const MASK: usize = 4;
const PROOF: usize = 5;
const BLUR: usize = 6;
const ENABLED: usize = 7;
const STORED: usize = 8;
const SPATIAL: usize = 9;
const COORD: usize = 10;
const LOOKUP: usize = 11;
type Facts = [u64; 16];
fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or_else(frame_inputs::invalid)
}
fn mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or_else(frame_inputs::invalid)
}
fn push(a: &mut u64, b: u64) -> Result<()> {
    *a = add(*a, b)?;
    Ok(())
}
fn area(r: Rect) -> Result<u64> {
    u64::try_from(r.area()).map_err(|_| frame_inputs::invalid())
}
fn spans(r: Rect, width: u64) -> Result<u64> {
    let w = u64::try_from(r.width()).map_err(|_| frame_inputs::invalid())?;
    let h = u64::try_from(r.height()).map_err(|_| frame_inputs::invalid())?;
    mul(h, add(w.div_ceil(width), 1)?)
}
// Caller paid the immutable one-root/descriptor/formation phase before this helper.
fn layer_facts(
    out: &mut Facts,
    layer: &Layer,
    base: &WorkspaceAdmission,
    numerical: &DocumentProgramsQuote,
) -> Result<()> {
    out[STORED] = layer.filters.len() as u64;
    let table = layer.raster.retained_tile_table_quote()?;
    out[LOOKUP] = add(
        65,
        add(
            table.bucket_upper as u64,
            mul(18, table.live_entries as u64)?,
        )?,
    )?;
    for filter in layer.filters.iter().filter(|f| f.enabled()) {
        push(&mut out[ENABLED], 1)?;
        let d = filter.descriptor();
        out[SPATIAL] |= u64::from(d.is_spatial());
        out[COORD] |= u64::from(matches!(d, Descriptor::AddNoiseRgbV1(_)));
        let bytes = filter
            .mask()
            .filter(|m| m.enabled())
            .map(|m| m.coverage().storage_usage().map_or(0, |(_, n)| n));
        let q = match bytes {
            Some(n) => add(64, mul(16, u64::from(64 - n.leading_zeros()))?)?,
            None => 0,
        };
        if d.is_spatial() {
            // The mix helper is invoked even for default255, though its early
            // return is cheaper. Query only executes for an enabled mask.
            push(&mut out[BLUR], add(q, 128)?)?;
            continue;
        }
        let family = match d {
            Descriptor::LevelsRgbV1(_) => Some(
                base.prepared_levels()
                    .map_or(numerical.levels_programs, |p| p.len()),
            ),
            Descriptor::ExposureSrgbV1(_) => Some(
                base.prepared_programs()
                    .map_or(numerical.exposure_programs, |p| p.len()),
            ),
            Descriptor::CurvesRgbV1(_) => Some(
                base.prepared_curves()
                    .map_or(numerical.curves_programs(), |p| p.len()),
            ),
            _ => None,
        };
        let s = match family {
            Some(n) => add(176, mul(32, n as u64)?)?,
            None => 176,
        };
        let e = if family.is_some() {
            400
        } else if matches!(
            d,
            Descriptor::VibranceRgbV1(_)
                | Descriptor::HueSaturationRgbV1(_)
                | Descriptor::AddNoiseRgbV1(_)
        ) {
            368
        } else {
            816
        };
        push(&mut out[SELECT], s)?;
        push(&mut out[EVAL], e)?;
        if let Some(n) = bytes {
            push(&mut out[MASK], add(q, 64)?)?;
            push(&mut out[PROOF], add(q, add(2048, mul(1024, n / 6)?)?)?)?;
        }
    }
    Ok(())
}
/// All math/helper construction below is prepaid on the original caller. The
/// return is only a checked scalar forecast; it never creates credit or a pool.
pub(super) fn supplement(
    doc: &Document,
    region: Rect,
    base: &WorkspaceAdmission,
    general: u64,
    numerical: &DocumentProgramsQuote,
    auth: &entry::EntryAuth8<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<u64> {
    auth.spend(1280)?;
    if std::mem::size_of::<Result<()>>() > 112 || std::mem::size_of::<Result<u64>>() > 112 {
        return Err(frame_inputs::invalid());
    }
    let mut total: Facts = [0; 16];
    total[WORK] = mul(4, area(region)?)?; // output_buffer's actual zeroing
    for id in &doc.roots {
        auth.spend(2048)?; // Root/get structural policy, not a hash timing theorem.
        let layer = doc.layers.get(id).ok_or_else(frame_inputs::invalid)?;
        auth.spend(8192)?;
        let stored = layer.filters.len() as u64;
        auth.spend(add(mul(128 + 4096, stored)?, 384)?)?;
        let mut facts: Facts = [0; 16];
        layer_facts(&mut facts, layer, base, numerical)?;
        if !layer.visible || layer.opacity == 0 {
            continue;
        }
        let Some(intersection) = region.intersect(&layer.extent()) else {
            continue;
        };
        let local = intersection
            .translated(-layer.origin.0, -layer.origin.1)
            .ok_or_else(frame_inputs::invalid)?;
        let n = area(local)?;
        push(&mut total[SAMPLES], n)?;
        push(&mut total[WORK], mul(1024, stored)?)?;
        if facts[SPATIAL] == 0 {
            let p = spans(local, 256)?;
            let c = add(
                mul(local.height() as u64, (local.width() as u64).div_ceil(64))?,
                p,
            )?;
            if stored != 0 || layer.mask.as_ref().is_some_and(|m| m.enabled()) {
                push(&mut total[WORK], add(mul(256, p)?, mul(8, n)?)?)?;
            }
            push(&mut total[WORK], mul(p, facts[LOOKUP])?)?;
            push(&mut total[WORK], mul(mul(64, stored)?, add(p, c)?)?)?;
            // Normal uniform proof is an attempt, not permission to omit fallback.
            if layer.blend_mode == LayerBlendMode::Normal && facts[COORD] == 0 {
                let one = add(
                    add(facts[SELECT], facts[EVAL])?,
                    add(facts[PROOF], facts[MASK])?,
                )?;
                push(&mut total[WORK], mul(p, one)?)?;
            }
            push(
                &mut total[WORK],
                add(
                    mul(c, facts[SELECT])?,
                    mul(n, add(facts[EVAL], facts[MASK])?)?,
                )?,
            )?;
            // Direct/coordinate decompositions are separate preflight paths.
            if facts[COORD] != 0
                || layer.filters.iter().any(|f| {
                    f.enabled()
                        && matches!(
                            f.descriptor(),
                            Descriptor::HueSaturationRgbV1(_) | Descriptor::VibranceRgbV1(_)
                        )
                })
            {
                let tiles = mul(
                    (local.width() as u64).div_ceil(64),
                    (local.height() as u64).div_ceil(64),
                )?;
                push(
                    &mut total[WORK],
                    mul(mul(6, tiles)?, add(512, mul(2048, stored)?)?)?,
                )?;
            }
        } else {
            let mut y = local.y0;
            while y < local.y1 {
                let mut x = local.x0;
                while x < local.x1 {
                    auth.spend(16384)?;
                    auth.spend(add(512, mul(2048, stored)?)?)?;
                    let tile = Rect {
                        x0: x,
                        y0: y,
                        x1: (x + 64).min(local.x1),
                        y1: (y + 64).min(local.y1),
                    };
                    let plan = LayerAppearancePlan::new(layer, tile, check)?;
                    auth.spend(64)?;
                    let q = plan.quote();
                    let i = q.pixels_per_buffer as u64;
                    let reverse = if facts[COORD] != 0 { 11 } else { 5 };
                    let future = mul(reverse, add(512, mul(2048, stored)?)?)?;
                    push(&mut total[WORK], add(future, mul(1024, stored)?)?)?;
                    push(
                        &mut total[WORK],
                        add(mul(8, i)?, mul(8, q.scratch_elements as u64)?)?,
                    )?;
                    push(&mut total[WORK], mul(1280, facts[ENABLED])?)?;
                    push(&mut total[WORK], facts[SELECT])?;
                    push(
                        &mut total[WORK],
                        mul(i, add(add(facts[EVAL], facts[MASK])?, facts[BLUR])?)?,
                    )?;
                    push(
                        &mut total[WORK],
                        mul(spans(plan.required_input(), 256)?, facts[LOOKUP])?,
                    )?;
                    push(
                        &mut total[WORK],
                        add(mul(8, area(plan.required_input())?)?, mul(8, area(tile)?)?)?,
                    )?;
                    x = tile.x1;
                }
                y = (y + 64).min(local.y1);
            }
        }
    }
    auth.spend(1024)?;
    let normal = total[SAMPLES]
        .checked_sub(general)
        .ok_or_else(frame_inputs::invalid)?;
    add(total[WORK], mul(64, normal)?)
}
