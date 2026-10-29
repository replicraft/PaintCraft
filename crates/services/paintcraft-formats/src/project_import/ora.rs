use super::*;

/// Flat Normal PNG layers only. The original archive is never a Save binding.
pub fn decode_ora(
    bytes: &[u8],
    pixel_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Converted> {
    convert(bytes, pixel_limit, false, check)
}
/// Backend-only converted copy. Generic ORA dispatch still rejects every group.
/// Neutral auto groups lose their names, hierarchy and separate visibility controls.
pub fn decode_ora_with_neutral_groups(
    bytes: &[u8],
    pixel_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Converted> {
    convert(bytes, pixel_limit, true, check)
}
fn convert(
    bytes: &[u8],
    pixel_limit: u64,
    allow_groups: bool,
    check: &dyn Fn() -> Result<()>,
) -> Result<Converted> {
    let limit = pixel_limit.min(4_194_304);
    let limits = crate::ora::Limits {
        archive_bytes: 64 << 20,
        entries: 1024,
        inflated_bytes: 64 << 20,
        entry_bytes: 32 << 20,
        layers: 256,
        pixels: limit,
        decoded_bytes: usize::try_from(limit.saturating_mul(if allow_groups { 12 } else { 4 }))
            .unwrap_or(usize::MAX),
        ..Default::default()
    };
    let (source, projection) = if allow_groups {
        let prepared = crate::ora::decode_neutral_groups(bytes, &limits, check)?;
        (prepared.decoded, prepared.merged)
    } else {
        (crate::ora::decode_layers(bytes, &limits, check)?, None)
    };
    let mut hash = Sha256::new();
    for chunk in bytes.chunks(65536) {
        check()?;
        hash.update(chunk);
    }
    let sha = paintcraft_document::hex(&hash.finalize());
    let identity = |kind: &str, object: &str| {
        deterministic_id(&["paintcraft.ora-conversion/v1", kind, &sha, object])
    };
    let mut roots = Vec::new();
    roots
        .try_reserve_exact(source.layers.len())
        .map_err(|_| resource())?;
    let mut layers = HashMap::new();
    layers
        .try_reserve(source.layers.len())
        .map_err(|_| resource())?;
    let tiles = source.layers.iter().try_fold(0u64, |total, layer| {
        let count = u64::from(layer.width.div_ceil(paintcraft_raster::TILE_EDGE))
            * u64::from(layer.height.div_ceil(paintcraft_raster::TILE_EDGE));
        total.checked_add(count).ok_or_else(resource)
    })?;
    let mut admission = Admission::new(4_194_304 * 16);
    admission.reserve_tiles(tiles)?;
    for (index, layer) in source.layers.into_iter().enumerate() {
        check()?;
        let id = identity("layer", &index.to_string());
        let raster = RasterLayer::from_rgba_checked(
            layer.width,
            layer.height,
            Rgba::TRANSPARENT,
            &layer.rgba,
            &mut admission,
            check,
        )?;
        roots.push(id);
        layers.insert(
            id,
            Layer {
                id,
                name: layer.name,
                origin: (i64::from(layer.origin.0), i64::from(layer.origin.1)),
                visible: layer.visible,
                opacity: layer.opacity,
                raster,
            },
        );
    }
    check()?;
    let document = Document {
        source_metadata: None,
        document_id: identity("document", ""),
        state_id: identity("state", ""),
        canvas: (source.width, source.height),
        roots,
        layers,
        title: source.title,
    };
    if let Some(expected) = projection {
        let rendered = paintcraft_render::render_cancellable(&document, check)?;
        if rendered != expected {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "ORA neutral group conversion differs from its merged rendering/projection profile",
            ));
        }
    }
    check()?;
    Ok(Converted {
        document,
        losses: source.losses,
    })
}
