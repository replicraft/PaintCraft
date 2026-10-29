use super::*;

/// Convert the admitted Photoshop RGB8 raster subset into a private native
/// document. This never binds Save to the PSD/PSB source or publishes an editor
/// state. Frontends must still obtain consent for all reported metadata losses.
pub fn decode_psd(
    bytes: &[u8],
    pixel_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Converted> {
    let source = crate::psd::decode(bytes, pixel_limit, check)?;
    let mut hash = Sha256::new();
    for chunk in bytes.chunks(65536) {
        check()?;
        hash.update(chunk);
    }
    let sha = paintcraft_document::hex(&hash.finalize());
    let identity = |kind: &str, object: &str| {
        deterministic_id(&["paintcraft.psd-conversion/v1", kind, &sha, object])
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
        let count = u64::from(layer.info.width.div_ceil(paintcraft_raster::TILE_EDGE))
            * u64::from(layer.info.height.div_ceil(paintcraft_raster::TILE_EDGE));
        total.checked_add(count).ok_or_else(resource)
    })?;
    let mut admission = Admission::new(4_194_304 * 16);
    admission.reserve_tiles(tiles)?;
    for (index, layer) in source.layers.into_iter().enumerate() {
        check()?;
        // Record index is essential: source names and numeric layer IDs can
        // repeat. The original file digest distinguishes different imports.
        let id = identity("layer", &index.to_string());
        let info = layer.info;
        let raster = RasterLayer::from_rgba_checked(
            info.width,
            info.height,
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
                name: info.name,
                origin: (i64::from(info.origin.0), i64::from(info.origin.1)),
                visible: info.visible,
                opacity: info.opacity,
                raster,
            },
        );
    }
    check()?;
    Ok(Converted {
        document: Document {
            source_metadata: None,
            document_id: identity("document", ""),
            state_id: identity("state", ""),
            canvas: (source.width, source.height),
            roots,
            layers,
            title: None,
        },
        losses: source.losses,
    })
}
