//! PCND 1: fixed semantic schema, binary IDs/integers and inline tiles.
//! The whole stream is integrity checked by a trailing SHA-256 before tile inflation.
use super::*;
use std::io::Write;
pub(super) mod catalog;
mod filters;
mod masks;
mod source_metadata;
mod wire;
use wire::{Input, Output};

const STRING_BYTES_MAX: usize = 16_384;

pub(super) fn write(
    doc: &Document,
    sink: impl Write,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    write_encoded(
        doc,
        Output::new(sink),
        check,
        compression::Encoder::new(true)?,
    )
}

pub(super) fn write_raw(
    doc: &Document,
    sink: impl Write,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    write_encoded(
        doc,
        Output::new(sink),
        check,
        compression::Encoder::new(false)?,
    )
}

pub(super) fn preflight_metadata(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<()> {
    metadata_counter(doc, check, false)
}

pub(super) fn preflight_compact_metadata(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    metadata_counter(doc, check, true)
}

fn metadata_counter(doc: &Document, check: &dyn Fn() -> Result<()>, compact: bool) -> Result<()> {
    write_encoded(
        doc,
        Output::metadata_counter(std::io::sink()),
        check,
        compression::Encoder::new(compact)?,
    )
}

fn write_encoded(
    doc: &Document,
    mut output: Output<impl Write>,
    check: &dyn Fn() -> Result<()>,
    mut encoder: compression::Encoder,
) -> Result<()> {
    check()?;
    if [doc.canvas.0, doc.canvas.1]
        .iter()
        .any(|value| !(1..=1_048_576).contains(value))
    {
        return Err(codec("invalid canvas dimensions"));
    }
    super::coverage::validate_counts(doc, check)?;
    let mut magic = *MAGIC;
    let live_filters = doc.layers.values().any(|layer| !layer.filters.is_empty());
    let masked = doc
        .layers
        .values()
        .any(|layer| layer.filters.iter().any(|filter| filter.mask().is_some()));
    let spatial = has_spatial(doc);
    let catalog_only = has_catalog(doc);
    let manifest = if catalog_only {
        Some(catalog::Manifest::capture(doc, check)?)
    } else {
        None
    };
    if catalog_only {
        magic[4] = 6;
    } else if spatial {
        magic[4] = 5;
    } else if masked {
        magic[4] = 4;
    } else if live_filters {
        magic[4] = 3;
    } else if doc.source_metadata.is_some() {
        magic[4] = 2;
    }
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
    let mut seen = HashSet::new();
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
            .filter(|layer| layer.id == *id && seen.insert(*id))
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
        // A terminator avoids a counting pass over decoded tiles. Each record is
        // bounded independently, allowing the sink to apply backpressure.
        for (bx, by, _, _, data) in layer.raster.blocks_iter() {
            check()?;
            let (encoding, stored) = encoder.block(&data)?;
            output.bytes(&[match encoding {
                "raw" => 1,
                "rgba-runs" => 2,
                "zstd" => 3,
                "lz4" => 4,
                "runs-lz4" => 5,
                _ => unreachable!(),
            }])?;
            output.number(u64::from(bx))?;
            output.number(u64::from(by))?;
            output.number(stored.len() as u64)?;
            output.payload(&stored)?;
        }
        output.bytes(&[0])?;
    }
    check()?;
    output.finish()
}

struct Block<'a> {
    x: u32,
    y: u32,
    encoding: u8,
    decoded: usize,
    bytes: &'a [u8],
}
struct Record<'a> {
    layer: Layer,
    blocks: Vec<Block<'a>>,
}

fn payload<'a>(bytes: &'a [u8], check: &dyn Fn() -> Result<()>) -> Result<&'a [u8]> {
    if bytes.len() < MAGIC.len() + 32 || &bytes[..4] != b"PCND" || bytes[5..8] != MAGIC[5..8] {
        return Err(codec("invalid compact PCND header"));
    }
    let (body, digest) = bytes.split_at(bytes.len() - 32);
    let mut hash = Sha256::new();
    for part in body.chunks(65_536) {
        check()?;
        hash.update(part);
    }
    if hash.finalize().as_slice() != digest {
        return Err(codec("compact PCND SHA-256 mismatch"));
    }
    Ok(&body[MAGIC.len()..])
}

pub(super) fn probe(bytes: &[u8]) -> Result<Header> {
    let body = payload(bytes, &|| Ok(()))?;
    let mut result = header(bytes[4]);
    if matches!(bytes[4], 3..=6) {
        let mut input = Input::new(body);
        if bytes[4] == 6 {
            catalog::Manifest::read(&mut input, false, &|| Ok(()))?;
        }
        input.take(32)?;
        input.dimension()?;
        input.dimension()?;
        match input.byte()? {
            0 => {}
            1 => {
                input.string(4096)?;
            }
            _ => return Err(codec("invalid title flag")),
        }
        match input.byte()? {
            0 => {}
            1 => result
                .required_features
                .push(SOURCE_METADATA_FEATURE.into()),
            _ => return Err(codec("invalid source metadata presence")),
        }
    }
    Ok(result)
}
fn header(version: u8) -> Header {
    let known = matches!(version, 1..=6);
    Header {
        container_version: i64::from(version),
        document_schema: if known {
            if version == 1 {
                "paintcraft.document/v1"
            } else if version == 2 {
                "paintcraft.document/v2"
            } else if version == 3 {
                "paintcraft.document/v3"
            } else if version == 4 {
                "paintcraft.document/v4"
            } else if version == 5 {
                "paintcraft.document/v5"
            } else {
                "paintcraft.document/v6"
            }
        } else {
            "unknown"
        }
        .into(),
        required_features: if known {
            SUPPORTED_FEATURES
                .iter()
                .map(|feature| (*feature).into())
                .chain((version == 2).then_some(SOURCE_METADATA_FEATURE.to_owned()))
                .chain((version >= 3).then_some(LAYER_FILTERS_FEATURE.to_owned()))
                .chain((version >= 4).then_some(LAYER_MASKS_FEATURE.to_owned()))
                .chain((version == 5).then_some(LAYER_SPATIAL_FILTERS_FEATURE.to_owned()))
                .chain((version == 6).then_some(LAYER_CATALOG_FEATURE.to_owned()))
                .collect()
        } else {
            Vec::new()
        },
    }
}

pub(super) fn read(
    bytes: &[u8],
    canvas_pixels: u64,
    staged_pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    read_with_layer_limit(bytes, canvas_pixels, staged_pixels, LAYER_COUNT_MAX, check)
}
pub(super) fn read_with_layer_limit(
    bytes: &[u8],
    canvas_pixels: u64,
    staged_pixels: u64,
    layer_limit: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    let budget = SharedBudget::new(paintcraft_document::source_metadata::BYTES_MAX + 4096);
    read_with_metadata(
        bytes,
        canvas_pixels,
        staged_pixels,
        layer_limit,
        MetadataAdmission {
            bytes: paintcraft_document::source_metadata::BYTES_MAX,
            budget: &budget,
        },
        check,
    )
}
pub(super) fn read_with_metadata(
    bytes: &[u8],
    canvas_pixels: u64,
    staged_pixels: u64,
    layer_limit: usize,
    metadata_admission: MetadataAdmission<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    read_admitted(
        bytes,
        canvas_pixels,
        staged_pixels,
        layer_limit,
        metadata_admission,
        None,
        check,
    )
}
pub(super) fn read_admitted(
    bytes: &[u8],
    canvas_pixels: u64,
    staged_pixels: u64,
    layer_limit: usize,
    metadata_admission: MetadataAdmission<'_>,
    coverage: Option<CoverageAdmission<'_>>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    let body = payload(bytes, check)?;
    header(bytes[4]).require_supported()?;
    if matches!(bytes[4], 4..=6) && coverage.is_none() {
        return Err(unsupported(if bytes[4] == 6 {
            "PCND6 requires explicit shared coverage admission"
        } else {
            "PCND4/5 requires explicit shared coverage admission"
        }));
    }
    let mut coverage = coverage.map(super::coverage::CoverageReader::new);
    let mut input = Input::new(body);
    let mut manifest = if bytes[4] == 6 {
        Some(catalog::Manifest::read(&mut input, true, check)?)
    } else {
        None
    };
    let document_id = input.id()?;
    let state_id = input.id()?;
    let canvas = (input.dimension()?, input.dimension()?);
    if u64::from(canvas.0) * u64::from(canvas.1) > canvas_pixels {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "canvas exceeds the host pixel limit",
        ));
    }
    let title = match input.byte()? {
        0 => None,
        1 => Some(input.string(4096)?),
        _ => return Err(codec("invalid title flag")),
    };
    let has_source = if matches!(bytes[4], 3..=6) {
        match input.byte()? {
            0 => false,
            1 => true,
            _ => return Err(codec("invalid source metadata presence")),
        }
    } else {
        bytes[4] == 2
    };
    let source_metadata = if has_source {
        let consumed = bytes.len() - 32 - input.remaining();
        let remaining = (DOCUMENT_MAX as usize)
            .checked_sub(consumed)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "PCND aggregate metadata allowance",
                )
            })?;
        Some(source_metadata::read(
            &mut input,
            metadata_admission.bytes.min(remaining as u64),
            metadata_admission.budget,
            check,
        )?)
    } else {
        None
    };
    let count = input.number(LAYER_COUNT_MAX as u64)? as usize;
    if count > layer_limit {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "PCND exceeds the host layer limit",
        ));
    }
    let mut roots = Vec::new();
    let mut records = Vec::new();
    let mut seen = HashSet::new();
    let mut tiles = 0u64;
    let mut any_filters = false;
    let mut any_masks = false;
    let mut any_spatial = false;
    let mut any_catalog = false;
    let mut metadata = bytes.len() - 32 - input.remaining();
    let mut admission = Admission::new(staged_pixels.min(STAGED_PIXELS_MAX));
    for _ in 0..count {
        check()?;
        let start = input.remaining();
        let id = input.id()?;
        if !seen.insert(id) {
            return Err(codec("duplicate layer id"));
        }
        let name = input.string(1024)?;
        let visible = match input.byte()? {
            0 => false,
            1 => true,
            _ => return Err(codec("invalid visibility flag")),
        };
        let opacity = input.byte()?;
        let origin = (input.signed()?, input.signed()?);
        let width = input.dimension()?;
        let height = input.dimension()?;
        let default = Rgba(input.take(4)?.try_into().unwrap());
        let layer_filters = if matches!(bytes[4], 3..=6) {
            let available = (DOCUMENT_MAX as usize)
                .checked_sub(metadata + start - input.remaining())
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::ResourceLimit,
                        "PCND aggregate metadata allowance",
                    )
                })?;
            if let Some(manifest) = &mut manifest {
                catalog::read_stack(
                    manifest,
                    &mut input,
                    available,
                    coverage.as_mut().expect("coverage admission checked"),
                    check,
                )?
            } else {
                filters::read(
                    &mut input,
                    available,
                    bytes[4],
                    if matches!(bytes[4], 4 | 5) {
                        coverage.as_mut()
                    } else {
                        None
                    },
                    check,
                )?
            }
        } else {
            paintcraft_document::FilterStack::default()
        };
        any_spatial |= layer_filters.iter().any(|filter| {
            matches!(
                filter.descriptor(),
                paintcraft_core::types::filters::Descriptor::BoxBlurV1(_)
            )
        });
        any_catalog |= layer_filters
            .iter()
            .any(|filter| filter.descriptor().is_catalog_only());
        any_filters |= !layer_filters.is_empty();
        any_masks |= layer_filters.iter().any(|filter| filter.mask().is_some());
        metadata += start - input.remaining();
        let mut blocks = Vec::new();
        let mut coords = HashSet::new();
        loop {
            check()?;
            let start = input.remaining();
            let encoding = input.byte()?;
            if encoding == 0 {
                metadata += 1;
                break;
            }
            if encoding > 5 {
                return Err(codec("unknown compact block encoding"));
            }
            let x = input.number(u64::from(width.div_ceil(TILE_EDGE) - 1))? as u32;
            let y = input.number(u64::from(height.div_ceil(TILE_EDGE) - 1))? as u32;
            if !coords.insert((x, y)) {
                return Err(codec("duplicate block coordinates"));
            }
            let decoded = ((width - x * TILE_EDGE).min(TILE_EDGE)
                * (height - y * TILE_EDGE).min(TILE_EDGE)
                * 4) as usize;
            let length = input.number(BLOCK_BYTES_MAX)? as usize;
            if length == 0 || (encoding == 1 && length != decoded) {
                return Err(codec("invalid stored block length"));
            }
            metadata += start - input.remaining();
            if metadata > DOCUMENT_MAX as usize {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "compact metadata exceeds its size limit",
                ));
            }
            tiles = tiles
                .checked_add(1)
                .ok_or_else(|| codec("block count overflows"))?;
            // Admission happens before any decoder sees a compressed payload.
            if tiles
                > staged_pixels.min(STAGED_PIXELS_MAX)
                    / (u64::from(TILE_EDGE) * u64::from(TILE_EDGE))
            {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "staged pixel budget exceeded",
                ));
            }
            let bytes = input.take(length)?;
            blocks.push(Block {
                x,
                y,
                encoding,
                decoded,
                bytes,
            });
        }
        if metadata > DOCUMENT_MAX as usize {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "compact metadata exceeds its size limit",
            ));
        }
        roots.push(id);
        records.push(Record {
            layer: Layer {
                id,
                name,
                origin,
                visible,
                opacity,
                filters: layer_filters,
                raster: RasterLayer::new(width, height, default),
            },
            blocks,
        });
    }
    if input.remaining() != 0 {
        return Err(codec("trailing compact PCND data"));
    }
    if bytes[4] == 5 && !any_spatial {
        return Err(codec("PCND5 requires a stored spatial filter"));
    }
    if bytes[4] == 4 && !any_masks {
        return Err(codec("unmasked documents require legacy PCND encoding"));
    }
    if bytes[4] == 3 && !any_filters {
        return Err(codec("empty filter stacks require legacy PCND encoding"));
    }
    if let Some(manifest) = &manifest {
        manifest.finish()?;
        if !any_catalog {
            return Err(codec("legacy-only documents require legacy PCND encoding"));
        }
    }
    admission.reserve_tiles(tiles)?;
    let mut layers = HashMap::new();
    for Record { mut layer, blocks } in records {
        for block in blocks {
            check()?;
            let decoded;
            let bytes = match block.encoding {
                1 => block.bytes,
                2 => {
                    decoded = runs::decode(block.bytes, block.decoded, check)?;
                    &decoded
                }
                3 => {
                    decoded = compression::decode(block.bytes, block.decoded, check)?;
                    &decoded
                }
                4 | 5 => {
                    decoded = compression::decode_lz4(
                        block.bytes,
                        block.decoded,
                        block.encoding == 5,
                        check,
                    )?;
                    &decoded
                }
                _ => unreachable!(),
            };
            layer
                .raster
                .set_block(block.x, block.y, bytes, &mut admission)?;
        }
        layers.insert(layer.id, layer);
    }
    check()?;
    Ok(Document {
        source_metadata,
        document_id,
        state_id,
        canvas,
        title,
        roots,
        layers,
    })
}

#[cfg(test)]
mod tests;

pub(super) fn filter_metadata_bytes(stack: &paintcraft_document::FilterStack) -> u64 {
    filters::metadata_bytes(stack)
}

pub(super) fn mask_metadata_bound(mask: Option<&paintcraft_document::FilterMask>) -> Option<u64> {
    masks::coarse_bytes(mask)
}

pub(super) fn coverage_decode_quote(bands: usize, runs: usize) -> Result<u64> {
    masks::decode_quote(bands, runs)
}

/// Container capability is selected by stored descriptors, including disabled/r0.
pub(super) fn has_spatial(doc: &Document) -> bool {
    doc.layers.values().any(|layer| {
        layer.filters.iter().any(|filter| {
            matches!(
                filter.descriptor(),
                paintcraft_core::types::filters::Descriptor::BoxBlurV1(_)
            )
        })
    })
}

pub(super) fn has_catalog(doc: &Document) -> bool {
    doc.layers
        .values()
        .any(|l| l.filters.iter().any(|f| f.descriptor().is_catalog_only()))
}
pub(super) fn catalog_manifest(
    bytes: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<catalog::Requirement>> {
    let body = payload(bytes, check)?;
    if bytes[4] != 6 {
        header(bytes[4]).require_supported()?;
        return Ok(Vec::new());
    }
    catalog::Manifest::read(&mut Input::new(body), false, check)?.requirements(check)
}

pub(super) fn catalog_metadata_bytes(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
) -> Result<u64> {
    Ok(catalog::Manifest::capture(doc, check)?.wire_bytes())
}
pub(super) fn catalog_stack_bytes(stack: &paintcraft_document::FilterStack) -> u64 {
    catalog::stack_bytes(stack)
}
