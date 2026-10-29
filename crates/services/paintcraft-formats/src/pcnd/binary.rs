//! PCND 1: fixed semantic schema, binary IDs/integers and inline tiles.
//! The whole stream is integrity checked by a trailing SHA-256 before tile inflation.
use super::*;
use std::io::Write;
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
    write_encoded(
        doc,
        Output::metadata_counter(std::io::sink()),
        check,
        compression::Encoder::new(false)?,
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
    let mut magic = *MAGIC;
    if doc.source_metadata.is_some() {
        magic[4] = 2;
    }
    output.bytes(&magic)?;
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
    if let Some(source) = &doc.source_metadata {
        source_metadata::write(source, &mut output, check)?;
    }
    if doc.roots.len() > LAYER_COUNT_MAX || doc.roots.len() != doc.layers.len() {
        return Err(codec("invalid flat document roots"));
    }
    output.number(doc.roots.len() as u64)?;
    let mut seen = HashSet::new();
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
    payload(bytes, &|| Ok(()))?;
    Ok(header(bytes[4]))
}
fn header(version: u8) -> Header {
    let known = matches!(version, 1 | 2);
    Header {
        container_version: i64::from(version),
        document_schema: if known {
            if version == 1 {
                "paintcraft.document/v1"
            } else {
                "paintcraft.document/v2"
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
    let body = payload(bytes, check)?;
    header(bytes[4]).require_supported()?;
    let mut input = Input::new(body);
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
    let source_metadata = if bytes[4] == 2 {
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
                raster: RasterLayer::new(width, height, default),
            },
            blocks,
        });
    }
    if input.remaining() != 0 {
        return Err(codec("trailing compact PCND data"));
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
