//! Streaming canonical/raw and optional compact v1 output.
use super::*;
pub(super) fn write_to(
    doc: &Document,
    sink: impl std::io::Write,
    compact: bool,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    let mut encoder = super::compression::Encoder::new(compact)?;
    let mut chunks: Vec<usize> = vec![0];
    let mut layer_records = Vec::new();
    for layer in doc.layers_in_order() {
        check()?;
        let mut blocks = Vec::new();
        for (bx, by, _w, _h, data) in layer.raster.blocks_iter() {
            check()?;
            let (encoding, stored) = encoder.block(&data)?;
            if *chunks.last().expect("one chunk") != 0
                && *chunks.last().expect("one chunk") + stored.len() > CHUNK_TARGET
            {
                chunks.push(0);
            }
            let index = chunks.len() - 1;
            let chunk = chunks.last_mut().expect("one chunk");
            blocks.push(Value::object([
                ("block_x", Value::Int(i64::from(bx))),
                ("block_y", Value::Int(i64::from(by))),
                ("entry", Value::str(format!("data/chunk-{index:06}.bin"))),
                ("offset", int(*chunk)),
                ("stored_bytes", int(stored.len())),
                ("decoded_bytes", int(data.len())),
                ("encoding", Value::str(encoding)),
                ("sha256", Value::str(sha(&data))),
            ]));
            *chunk += stored.len();
        }
        layer_records.push(Value::object([
            ("kind", Value::str("raster")),
            ("id", Value::str(layer.id.to_string())),
            ("name", Value::str(&layer.name)),
            ("visible", Value::Bool(layer.visible)),
            ("opacity_u8", Value::Int(i64::from(layer.opacity))),
            ("blend", Value::str("normal")),
            (
                "origin",
                Value::object([
                    ("x", Value::Int(layer.origin.0)),
                    ("y", Value::Int(layer.origin.1)),
                ]),
            ),
            ("width", Value::Int(i64::from(layer.raster.width()))),
            ("height", Value::Int(i64::from(layer.raster.height()))),
            ("format", Value::str("rgba8-straight-srgb")),
            (
                "default_pixel",
                Value::str(layer.raster.default_pixel().to_hex()),
            ),
            ("blocks", Value::Array(blocks)),
        ]));
    }
    let metadata = doc.title.as_ref().map_or_else(
        || Value::Object(Default::default()),
        |t| Value::object([("title", Value::str(t))]),
    );
    let document = json::canonical(&Value::object([
        ("schema", Value::str("paintcraft.document/v1")),
        ("document_id", Value::str(doc.document_id.to_string())),
        ("state_id", Value::str(doc.state_id.to_string())),
        (
            "canvas",
            Value::object([
                ("width", Value::Int(i64::from(doc.canvas.0))),
                ("height", Value::Int(i64::from(doc.canvas.1))),
            ]),
        ),
        ("profile", Value::str("srgb")),
        ("compositing_policy", Value::str(POLICY)),
        ("persistence_block_edge", Value::Int(i64::from(TILE_EDGE))),
        (
            "roots",
            Value::Array(
                doc.roots
                    .iter()
                    .map(|r| Value::str(r.to_string()))
                    .collect(),
            ),
        ),
        ("layers", Value::Array(layer_records)),
        ("metadata", metadata),
    ]));
    if document.len() as u64 > DOCUMENT_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "document.json exceeds its size limit",
        ));
    }
    let manifest = json::canonical(&Value::object([
        ("format", Value::str("paintcraft.native")),
        ("container_version", Value::Int(1)),
        ("document_schema", Value::str("paintcraft.document/v1")),
        ("document_entry", Value::str("document.json")),
        ("document_sha256", Value::str(sha(&document))),
        ("document_bytes", int(document.len())),
        (
            "required_features",
            Value::Array(SUPPORTED_FEATURES.iter().map(|f| Value::str(*f)).collect()),
        ),
    ]));
    let mut archive = zip::StoredWriter::new(sink);
    let mut chunk = Vec::with_capacity(CHUNK_TARGET);
    let mut index = 0;
    for layer in doc.layers_in_order() {
        check()?;
        for (_, _, _, _, data) in layer.raster.blocks_iter() {
            check()?;
            let (_, stored) = encoder.block(&data)?;
            if !chunk.is_empty() && chunk.len() + stored.len() > CHUNK_TARGET {
                archive.member(&format!("data/chunk-{index:06}.bin"), &chunk)?;
                chunk.clear();
                index += 1;
            }
            chunk.extend_from_slice(&stored);
        }
    }
    if !chunk.is_empty() {
        archive.member(&format!("data/chunk-{index:06}.bin"), &chunk)?;
    }
    archive.member("document.json", &document)?;
    archive.member("manifest.json", &manifest)?;
    check()?;
    archive.finish()?;
    Ok(())
}
