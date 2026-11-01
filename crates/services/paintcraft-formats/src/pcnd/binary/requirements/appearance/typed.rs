//! Canonical actual per-use inventories and typed immutable-source provenance.
use super::*;
fn layers(
    doc: &Document,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<Indices> {
    if doc.layers.len() > LAYER_COUNT_MAX {
        return Err(resource());
    }
    let mut index = Indices::new(doc.layers.len(), scope)?;
    let checked = step(check, spend);
    for id in doc.layers.keys() {
        checked()?;
        index.push(Index {
            id: *id.as_bytes(),
            offset: 0,
        })?;
    }
    index.sort(check, spend)?;
    Ok(index)
}
fn source_row(
    base: &Document,
    key: CoverageKey,
    require_retained: bool,
) -> Result<(CoverageRow, bool)> {
    let layer = base
        .layers
        .get(&Uuid::from_bytes(key.layer))
        .ok_or_else(invalid)?;
    if key.kind == 1 {
        let mask = layer.mask.as_ref().ok_or_else(invalid)?;
        Ok((
            mask_row(key, mask.coverage(), mask.enabled(), require_retained)?,
            true,
        ))
    } else {
        if layer.filters.len() > 256 {
            return Err(resource());
        }
        let filter = layer
            .filters
            .iter()
            .find(|f| f.id().as_bytes() == &key.filter)
            .ok_or_else(invalid)?;
        let mask = filter.mask().ok_or_else(invalid)?;
        Ok((
            mask_row(key, mask.coverage(), mask.enabled(), require_retained)?,
            filter.enabled(),
        ))
    }
}
fn origin(
    base: &Document,
    key: CoverageKey,
    provenance: CandidateProvenance,
) -> Result<CoverageKey> {
    if let CandidateProvenance::Duplicate {
        layer,
        source_filter,
        new_filter,
    } = provenance
        && key.kind == 0
        && key.layer == *layer.as_bytes()
        && key.filter == *new_filter.as_bytes()
    {
        let source = base.layers.get(&layer).ok_or_else(invalid)?;
        if source.filters.len() > 256 {
            return Err(resource());
        }
        if source.filters.iter().any(|f| f.id() == new_filter) || source_filter == new_filter {
            return Err(invalid());
        }
        return Ok(CoverageKey {
            filter: *source_filter.as_bytes(),
            ..key
        });
    }
    Ok(key)
}
fn each(
    doc: &Document,
    indices: &Indices,
    filter_index: &mut Indices,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
    emit: &mut dyn FnMut(CoverageRow, bool) -> Result<()>,
    require_retained: bool,
) -> Result<()> {
    let checked = step(check, spend);
    for index in &indices.values {
        checked()?;
        let layer = doc
            .layers
            .get(&Uuid::from_bytes(index.id))
            .ok_or_else(invalid)?;
        if layer.id.as_bytes() != &index.id {
            return Err(invalid());
        }
        filter_index.values.clear();
        if layer.filters.len() > 256 {
            return Err(resource());
        }
        for (offset, filter) in layer.filters.iter().enumerate() {
            checked()?;
            filter_index.push(Index {
                id: *filter.id().as_bytes(),
                offset,
            })?;
        }
        filter_index.sort(check, spend)?;
        for filter in &filter_index.values {
            checked()?;
            let instance = layer
                .filters
                .iter()
                .nth(filter.offset)
                .ok_or_else(invalid)?;
            if let Some(mask) = instance.mask() {
                emit(
                    mask_row(
                        CoverageKey {
                            layer: index.id,
                            kind: 0,
                            filter: filter.id,
                        },
                        mask.coverage(),
                        mask.enabled(),
                        require_retained,
                    )?,
                    instance.enabled(),
                )?;
            }
        }
        if let Some(mask) = &layer.mask {
            emit(
                mask_row(
                    CoverageKey {
                        layer: index.id,
                        kind: 1,
                        filter: [0; 16],
                    },
                    mask.coverage(),
                    mask.enabled(),
                    require_retained,
                )?,
                true,
            )?;
        }
    }
    Ok(())
}
pub(super) fn capture(
    base: &Document,
    doc: &Document,
    provenance: CandidateProvenance,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
    require_retained: bool,
) -> Result<Inventory7> {
    if base.layers.len() > LAYER_COUNT_MAX {
        return Err(resource());
    }
    let mut controls = scope.account();
    controls.resize(CONTROL_BYTES)?;
    let checked = step(check, spend);
    // Source lookup scans at most256 filters. Price its fixed maximum per row
    // before any source search, in addition to traversal/hash slots.
    let mut layers = layers(doc, scope, check, spend)?;
    let mut filters = Indices::new(256, scope)?;
    let mut totals = Totals::default();
    each(
        doc,
        &layers,
        &mut filters,
        check,
        spend,
        &mut |row, _| totals.row(row),
        require_retained,
    )?;
    let abi = CoverageAbi::current()?;
    let mut bounds = CoverageHasher::new(abi, totals.occurrences)?;
    let mut inventory = InventoryHasher::new(totals.occurrences)?;
    let mut own = IdentityHasher::new(totals.stored)?;
    each(
        doc,
        &layers,
        &mut filters,
        check,
        spend,
        &mut |row, enabled| {
            // Duplicate origin scans for an existing new ID, then source_row
            // independently finds the original. Prepay BOTH bounded passes.
            let duplicate = matches!(provenance,CandidateProvenance::Duplicate{layer,new_filter,..} if row.key.kind==0 && row.key.layer==*layer.as_bytes() && row.key.filter==*new_filter.as_bytes());
            let passes = if row.key.kind == 0 {
                if duplicate { 2 } else { 1 }
            } else {
                0
            };
            spend(FILTER_LOOKUP_WORK * passes)?;
            checked()?;
            let source = origin(base, row.key, provenance)?;
            let (expected, _) = source_row(base, source, require_retained)?;
            let inherited = CoverageRow { key: source, ..row };
            if inherited != expected {
                return Err(invalid());
            }
            bounds.push(row)?;
            inventory.push(row, enabled, source)?;
            if row.key.kind == 1 {
                own.push(row.key.layer, row.enabled, row.digest)?;
            }
            Ok(())
        },
        require_retained,
    )?;
    // Avoid retaining index capacity in fixed returned facts.
    layers.values.clear();
    totals.finish(abi, own.finish()?, bounds.finish()?, inventory.finish()?)
}
/// Duplicate is the only supported numeric BEGIN that increases logical A8 use.
/// Run BEFORE private stage allocation; this reserves/debits no fresh authority.
pub fn preflight_candidate_counts(
    base: &Document,
    provenance: CandidateProvenance,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    let duplicate = match provenance {
        CandidateProvenance::Unchanged => None,
        CandidateProvenance::Duplicate {
            layer,
            source_filter,
            ..
        } => Some((layer, source_filter)),
    };
    preflight(base, duplicate, check, spend)
}
pub fn preflight_duplicate_counts(
    base: &Document,
    layer: Uuid,
    source_filter: Uuid,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    preflight(base, Some((layer, source_filter)), check, spend)
}
fn preflight(
    base: &Document,
    duplicate: Option<(Uuid, Uuid)>,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    if base.layers.len() > LAYER_COUNT_MAX || base.roots.len() > LAYER_COUNT_MAX {
        return Err(resource());
    }
    let require_retained = false;
    let checked = step(check, spend);
    let mut totals = Totals::default();
    for layer in base.layers.values() {
        if layer.filters.len() > 256 {
            return Err(resource());
        }
        checked()?;
        for filter in layer.filters.iter() {
            checked()?;
            if let Some(mask) = filter.mask() {
                totals.row(mask_row(
                    CoverageKey {
                        layer: *layer.id.as_bytes(),
                        kind: 0,
                        filter: *filter.id().as_bytes(),
                    },
                    mask.coverage(),
                    mask.enabled(),
                    require_retained,
                )?)?;
            }
        }
        if let Some(mask) = &layer.mask {
            totals.row(mask_row(
                CoverageKey {
                    layer: *layer.id.as_bytes(),
                    kind: 1,
                    filter: [0; 16],
                },
                mask.coverage(),
                mask.enabled(),
                require_retained,
            )?)?;
        }
    }
    if let Some((layer, source_filter)) = duplicate {
        checked()?;
        let target = base.layers.get(&layer).ok_or_else(invalid)?;
        if target.filters.len() >= 256 {
            return Err(resource());
        }
        spend(FILTER_LOOKUP_WORK)?;
        let filter = target
            .filters
            .iter()
            .find(|f| f.id() == source_filter)
            .ok_or_else(invalid)?;
        if let Some(mask) = filter.mask() {
            totals.row(mask_row(
                CoverageKey {
                    layer: *layer.as_bytes(),
                    kind: 0,
                    filter: *source_filter.as_bytes(),
                },
                mask.coverage(),
                mask.enabled(),
                require_retained,
            )?)?;
        }
    }
    Ok(())
}
