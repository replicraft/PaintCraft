//! Separate Source8 portable snapshot writer. Old7 writer body is untouched.
use super::*;
mod rooted;
/// Same nonoverlapping root/tile scratch phases, no new payload codec or pool.
/// Quote walks layers; caller prices that walk before calling this pure helper.
pub fn portable_blend_snapshot_scratch_quote(doc: &Document) -> Result<u64> {
    rooted::scratch_quote(doc)
}
fn write_encoded_blend_admitted(
    doc: &Document,
    mut output: Output<impl Write>,
    check: &dyn Fn() -> Result<()>,
    scope: &BudgetScope,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    check()?;
    if [doc.canvas.0, doc.canvas.1]
        .iter()
        .any(|value| !(1..=1_048_576).contains(value))
    {
        return Err(codec("invalid canvas dimensions"));
    }
    rooted::validate_coverage_counts(doc, check)?;
    let mut magic = *MAGIC;
    magic[4] = 8;
    let manifest = catalog::Manifest::capture_blend_rooted(doc, check)?;
    output.bytes(&magic)?;
    manifest.write(&mut output, check)?;
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
    output.bytes(&[u8::from(doc.source_metadata.is_some())])?;
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
        blends::write(8, layer.blend_mode, &mut output)?;
        output.signed(layer.origin.0)?;
        output.signed(layer.origin.1)?;
        output.number(u64::from(layer.raster.width()))?;
        output.number(u64::from(layer.raster.height()))?;
        output.bytes(&layer.raster.default_pixel().0)?;
        catalog::write_stack(&manifest, &layer.filters, &mut output, check, &mask_check)?;
        masks::write_layer(layer.mask.as_ref(), &mut output, &mask_check)?;
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

pub fn write_portable_blend_snapshot_admitted_checked(
    doc: &Document,
    sink: impl Write,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    // No table-wide scan precedes shape validation or metadata admission.
    rooted::preflight(doc, scope, check, spend)?;
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
    let result = write_encoded_blend_admitted(doc, Output::new(sink), &checked, scope, spend);
    if let Some(error) = failure.into_inner() {
        return Err(error);
    }
    result
}
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
