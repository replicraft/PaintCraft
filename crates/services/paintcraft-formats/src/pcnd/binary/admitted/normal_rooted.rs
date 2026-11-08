//! Separate Normal-only rooted portable1-7 writer; old writers stay exact.
use super::*;
mod plan;
/// Pure advisory root walk; caller prepays2048/root before invoking it.
pub fn portable_normal_snapshot_rooted_scratch_quote(doc: &Document) -> Result<u64> {
    plan::scratch_quote(doc)
}
fn write_encoded_rooted(
    doc: &Document,
    mut output: Output<impl Write>,
    check: &dyn Fn() -> Result<()>,
    scope: &BudgetScope,
    spend: &dyn Fn(u64) -> Result<()>,
    facts: &plan::Facts,
) -> Result<()> {
    check()?;
    if [doc.canvas.0, doc.canvas.1]
        .iter()
        .any(|value| !(1..=1_048_576).contains(value))
    {
        return Err(codec("invalid canvas dimensions"));
    }
    let mut magic = *MAGIC;
    magic[4] = facts.version;
    let layer_masks = facts.version == 7;
    let live_filters = facts.version >= 3;
    let manifest = if facts.version >= 6 {
        // Distinct optional catalog root walk + existing bounded comparison slots.
        spend(
            plan::root_work(doc.roots.len(), 1)?
                .checked_add(facts.filters.checked_mul(768).ok_or_else(limit)?)
                .ok_or_else(limit)?,
        )?;
        Some(catalog::Manifest::capture_normal_rooted(
            doc,
            layer_masks,
            check,
        )?)
    } else {
        None
    };
    output.bytes(&magic)?;
    if let Some(manifest) = &manifest {
        manifest.write(&mut output, check)?;
    }
    output.bytes(doc.document_id.as_bytes())?;
    output.bytes(doc.state_id.as_bytes())?;
    output.number(u64::from(doc.canvas.0))?;
    output.number(u64::from(doc.canvas.1))?;
    match &doc.title {
        None => output.bytes(&[0])?,
        Some(title) => {
            if title.chars().count() > 4096 {
                return Err(codec("title exceeds 4096 characters"));
            }
            output.bytes(&[1])?;
            output.string(title)?;
        }
    }
    if live_filters {
        output.bytes(&[u8::from(doc.source_metadata.is_some())])?;
    }
    if let Some(source) = &doc.source_metadata {
        source_metadata::write(source, &mut output, check)?;
    }
    if doc.roots.len() > LAYER_COUNT_MAX || doc.roots.len() != doc.layers.len() {
        return Err(codec("invalid flat document roots"));
    }
    output.number(doc.roots.len() as u64)?;
    let mask_work = std::cell::Cell::new(paintcraft_raster::coverage_mask::Limits::default().work);
    let mask_check = || {
        check()?;
        mask_work.set(mask_work.get().checked_sub(1).ok_or_else(|| {
            Error::new(ErrorCode::ResourceLimit, "PCND aggregate mask writer work")
        })?);
        Ok(())
    };
    spend(plan::root_work(doc.roots.len(), 1)?)?;
    for id in &doc.roots {
        check()?;
        let layer = doc
            .layers
            .get(id)
            .filter(|layer| layer.id == *id)
            .ok_or_else(|| codec("invalid flat document roots"))?;
        if layer.name.chars().count() > 1024 {
            return Err(codec("layer name exceeds 1024 characters"));
        }
        if [layer.raster.width(), layer.raster.height()]
            .iter()
            .any(|value| !(1..=1_048_576).contains(value))
        {
            return Err(codec("invalid layer dimensions"));
        }
        output.bytes(id.as_bytes())?;
        output.string(&layer.name)?;
        output.bytes(&[u8::from(layer.visible), layer.opacity])?;
        output.signed(layer.origin.0)?;
        output.signed(layer.origin.1)?;
        output.number(u64::from(layer.raster.width()))?;
        output.number(u64::from(layer.raster.height()))?;
        output.bytes(&layer.raster.default_pixel().0)?;
        if live_filters {
            if let Some(manifest) = &manifest {
                catalog::write_stack(manifest, &layer.filters, &mut output, check, &mask_check)?;
            } else {
                filters::write(&layer.filters, &mut output, magic[4], check, &mask_check)?;
            }
        }
        if layer_masks {
            masks::write_layer(layer.mask.as_ref(), &mut output, &mask_check)?;
        }
        // A terminator avoids a counting pass over decoded tiles. Each record is
        // bounded independently, allowing the sink to apply backpressure.
        blocks(&layer.raster, scope, check, spend, &mut |bx, by, data| {
            let stored = compression::admitted::block(data, scope, check, spend)?;
            output.bytes(&[match stored.encoding() {
                "raw" => 1,
                "rgba-runs" => 2,
                "lz4" => 4,
                "runs-lz4" => 5,
                _ => unreachable!(),
            }])?;
            output.number(u64::from(bx))?;
            output.number(u64::from(by))?;
            output.number(stored.bytes().len() as u64)?;
            output.payload(stored.bytes())
        })?;
        output.bytes(&[0])?;
    }
    check()?;
    output.finish()
}

pub fn write_portable_normal_snapshot_rooted_admitted_checked(
    doc: &Document,
    sink: impl Write,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    let facts = plan::prepare(doc, scope, check, spend)?;
    let failure = RefCell::new(None);
    let sink = Metered {
        sink,
        check,
        spend,
        failure: &failure,
    };
    let checked = || {
        check()?;
        spend(1)
    };
    let result = write_encoded_rooted(doc, Output::new(sink), &checked, scope, spend, &facts);
    if let Some(error) = failure.into_inner() {
        return Err(error);
    }
    result
}
#[cfg(test)]
mod tests;
