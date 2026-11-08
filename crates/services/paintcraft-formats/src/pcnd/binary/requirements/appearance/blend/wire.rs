//! Two bounded borrowed walks; no authentication repeat or payload inflation.
use super::*;
struct Scan<'a> {
    body: &'a [u8],
    limits: RequirementsLimits,
    masks: Limits,
    tiles: u64,
    payload: usize,
    validate_metadata: bool,
}
impl Scan<'_> {
    fn mask(
        &mut self,
        input: &mut Input<'_>,
        key: CoverageKey,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Option<CoverageRow>> {
        Ok(
            masks::appearance::probe(input, self.limits.metadata_bytes, &mut self.masks, check)?
                .map(|m| CoverageRow {
                    key,
                    enabled: m.enabled,
                    domain: m.domain,
                    digest: m.digest,
                    bands: m.bands,
                    runs: m.runs,
                    decode_upper_bytes: m.upper,
                }),
        )
    }
    fn layer(
        &mut self,
        input: &mut Input<'_>,
        manifest: &mut catalog::Manifest<'_>,
        mut indices: Option<&mut Indices>,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
        emit: &mut dyn FnMut(CoverageRow, bool) -> Result<()>,
    ) -> Result<([u8; 16], u8)> {
        check()?;
        let layer = *input.id()?.as_bytes();
        bounded_string(input, 1024, spend)?;
        flag(input)?;
        input.byte()?;
        let mode = input.byte()?;
        if mode > 2 {
            return Err(codec("unknown PCND8 layer blend tag"));
        }
        input.signed()?;
        input.signed()?;
        let width = input.dimension()?;
        let height = input.dimension()?;
        input.take(4)?;
        let count = input.number(256)?;
        for _ in 0..count {
            check()?;
            let offset = self.body.len() - input.remaining();
            let id = *input.id()?.as_bytes();
            let enabled = flag(input)?;
            if let Some(target) = indices.as_mut() {
                target.push(Index { id, offset })?;
            }
            let entry = input.number(255)? as usize;
            let len = input.number(65_536)? as usize;
            manifest.resolve_blend_appearance(entry, input.take(len)?)?;
            if let Some(row) = self.mask(
                input,
                CoverageKey {
                    layer,
                    kind: 0,
                    filter: id,
                },
                check,
            )? {
                emit(row, enabled)?;
            }
        }
        if let Some(row) = self.mask(
            input,
            CoverageKey {
                layer,
                kind: 1,
                filter: [0; 16],
            },
            check,
        )? {
            emit(row, true)?;
        }
        loop {
            check()?;
            let encoding = input.byte()?;
            if encoding == 0 {
                break;
            }
            if encoding > 5 {
                return Err(codec("unknown compact block encoding"));
            }
            let x = input.number(u64::from(width.div_ceil(TILE_EDGE) - 1))? as u32;
            let y = input.number(u64::from(height.div_ceil(TILE_EDGE) - 1))? as u32;
            let decoded = ((width - x * TILE_EDGE).min(TILE_EDGE)
                * (height - y * TILE_EDGE).min(TILE_EDGE)
                * 4) as usize;
            let length = input.number(BLOCK_BYTES_MAX)? as usize;
            if length == 0 || (encoding == 1 && length != decoded) {
                return Err(codec("invalid stored block length"));
            }
            self.tiles = self
                .tiles
                .checked_add(1)
                .filter(|n| *n <= self.limits.tiles)
                .ok_or_else(resource)?;
            input.take(length)?;
            self.payload = self.payload.checked_add(length).ok_or_else(resource)?;
            if self.validate_metadata {
                metadata(
                    self.body.len() + MAGIC.len(),
                    input.remaining(),
                    self.payload,
                    self.limits.metadata_bytes,
                )?;
            }
        }
        if self.validate_metadata {
            metadata(
                self.body.len() + MAGIC.len(),
                input.remaining(),
                self.payload,
                self.limits.metadata_bytes,
            )?;
        }
        Ok((layer, mode))
    }
}
fn bounded_string(
    input: &mut Input<'_>,
    chars: usize,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    let length = input.number(STRING_BYTES_MAX as u64)? as usize;
    spend((length as u64).checked_mul(4).ok_or_else(resource)?)?;
    let text =
        std::str::from_utf8(input.take(length)?).map_err(|_| codec("invalid compact UTF-8"))?;
    if text.chars().count() > chars {
        return Err(codec("compact string character limit"));
    }
    Ok(())
}
fn prefix<'a>(
    verified: &CheckedContainer<'a>,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<(Input<'a>, catalog::Manifest<'a>, usize, u64, [u8; 16])> {
    let mut input = Input::new(verified.body());
    let manifest = catalog::Manifest::read_v7(&mut input, false, check)?;
    let document = *input.id()?.as_bytes();
    input.take(16)?;
    input.dimension()?;
    input.dimension()?;
    if flag(&mut input)? {
        bounded_string(&mut input, 4096, spend)?;
    }
    let before = input.remaining();
    let metadata_bytes = if flag(&mut input)? {
        source(&mut input)?;
        (before - input.remaining() - 1) as u64
    } else {
        0
    };
    let count = input.number(LAYER_COUNT_MAX as u64)? as usize;
    Ok((input, manifest, count, metadata_bytes, document))
}
pub(super) fn probe(
    verified: &CheckedContainer<'_>,
    limits: RequirementsLimits,
    scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<SourceCoverage8> {
    let mut controls = scope.account();
    controls.resize(CONTROL_BYTES)?;
    let remaining = Cell::new(limits.work);
    let debit = |units: u64| {
        let next = remaining.get().checked_sub(units).ok_or_else(resource)?;
        spend(units)?;
        remaining.set(next);
        Ok(())
    };
    let spend: &dyn Fn(u64) -> Result<()> = &debit;
    let checked = step(check, spend);
    let (mut input, mut manifest, count, wire_metadata_bytes, document) =
        prefix(verified, &checked, spend)?;
    if count > limits.layers {
        return Err(resource());
    }
    let mut layers = Indices::new(count, scope)?;
    let mut totals = Totals::default();
    let mut scan = Scan {
        body: verified.body(),
        limits,
        masks: limits.coverage,
        tiles: 0,
        payload: 0,
        validate_metadata: true,
    };
    // Existing semantic identity frame, hashed in ORIGINAL root wire order.
    let extent = (b"paintcraft.layer-blends/v1\0".len() as u64)
        .checked_add(20)
        .and_then(|v| v.checked_add((count as u64).checked_mul(17)?))
        .ok_or_else(resource)?;
    spend(extent.checked_mul(16).ok_or_else(resource)?)?;
    let mut blend = Sha256::new();
    blend.update(b"paintcraft.layer-blends/v1\0");
    blend.update(document);
    blend.update(u32::try_from(count).map_err(|_| resource())?.to_be_bytes());
    let mut has_mode = false;
    for _ in 0..count {
        let offset = verified.body().len() - input.remaining();
        let (id, mode) = scan.layer(
            &mut input,
            &mut manifest,
            None,
            &checked,
            spend,
            &mut |row, _| totals.row(row),
        )?;
        blend.update(id);
        blend.update([mode]);
        has_mode |= mode != 0;
        layers.push(Index { id, offset })?;
    }
    if input.remaining() != 0 {
        return Err(codec("trailing compact PCND data"));
    }
    manifest.finish()?;
    let requirements = manifest.blend_appearance_requirements(&checked)?;
    if !has_mode {
        return Err(codec("PCND8 requires a stored nonNormal layer blend"));
    }
    layers.sort(check, spend)?;
    let abi = CoverageAbi::current()?;
    let mut bounds = CoverageHasher::new(abi, totals.occurrences)?;
    let mut inventory = InventoryHasher::new(totals.occurrences)?;
    let mut own = IdentityHasher::new(totals.stored)?;
    let mut filters = Indices::new(256, scope)?;
    // Second canonical borrowed walk; filter array reused for each layer. Counts
    // constrain each walk separately rather than doubling a document's limits.
    let (_, mut canonical, _, _, _) = prefix(verified, &checked, spend)?;
    for layer in &layers.values {
        filters.values.clear();
        let mut input = Input::new(&verified.body()[layer.offset..]);
        let mut own_row = None;
        let mut layer_scan = Scan {
            body: verified.body(),
            limits,
            masks: limits.coverage,
            tiles: 0,
            payload: 0,
            validate_metadata: false,
        };
        // Metadata was validated in the complete first walk. Canonical per-layer
        // rewalk keeps only this layer's filter offsets, not any A8 payload.
        layer_scan.limits.metadata_bytes = DOCUMENT_MAX as usize;
        layer_scan.layer(
            &mut input,
            &mut canonical,
            Some(&mut filters),
            &checked,
            spend,
            &mut |row, _| {
                if row.key.kind == 1 {
                    own_row = Some(row);
                }
                Ok(())
            },
        )?;
        filters.sort(check, spend)?;
        for filter in &filters.values {
            let mut input = Input::new(&verified.body()[filter.offset..]);
            input.take(16)?;
            let enabled = flag(&mut input)?;
            let entry = input.number(255)? as usize;
            let len = input.number(65_536)? as usize;
            canonical.resolve_blend_appearance(entry, input.take(len)?)?;
            let mut mask_limits = limits.coverage;
            if let Some(mask) = masks::appearance::probe(
                &mut input,
                limits.metadata_bytes,
                &mut mask_limits,
                &checked,
            )? {
                let row = CoverageRow {
                    key: CoverageKey {
                        layer: layer.id,
                        kind: 0,
                        filter: filter.id,
                    },
                    enabled: mask.enabled,
                    domain: mask.domain,
                    digest: mask.digest,
                    bands: mask.bands,
                    runs: mask.runs,
                    decode_upper_bytes: mask.upper,
                };
                bounds.push(row)?;
                inventory.push(row, enabled, row.key)?;
            }
        }
        if let Some(row) = own_row {
            bounds.push(row)?;
            inventory.push(row, true, row.key)?;
            own.push(row.key.layer, row.enabled, row.digest)?;
        }
    }
    let inventory = totals.finish(abi, own.finish()?, bounds.finish()?, inventory.finish()?)?;
    Ok(SourceCoverage8 {
        layer_blend: blend.finalize().into(),
        requirements,
        inventory,
        wire_tiles: scan.tiles,
        wire_metadata_bytes,
    })
}
