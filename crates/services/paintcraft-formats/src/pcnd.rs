//! PCND v1: ZIP container with `manifest.json`, `document.json` and raw
//! `data/chunk-NNNNNN.bin` blocks. The writer is canonical; the reader admits
//! only what this build can edit and bounds every size before allocating.

use std::collections::{HashMap, HashSet};

use paintcraft_core::types::ids::parse_uuid;
use paintcraft_core::types::json::{self, Value};
use paintcraft_core::types::policy::STAGED_PIXELS_MAX;
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba};
use paintcraft_document::{COORD_ABS_MAX, Document, LAYER_COUNT_MAX, Layer, POLICY, hex};
use paintcraft_raster::{Admission, RasterLayer, TILE_EDGE};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::zip;

/// Features this build can edit. A file requiring anything else is not opened.
pub const SUPPORTED_FEATURES: [&str; 3] = ["raster.rgba8", "canvas.finite", "layers.flat"];
const CHUNK_TARGET: usize = 4 * 1024 * 1024;
const MANIFEST_MAX: u64 = 1_048_576;
const DOCUMENT_MAX: u64 = 8_388_608;
const CHUNK_MAX: u64 = 8_388_608;
const ENTRIES_MAX: usize = 100_000;
const BLOCK_BYTES_MAX: u64 = 262_144;

fn int(n: usize) -> Value {
    Value::Int(n as i64)
}

fn sha(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Canonical PCND v1 bytes for a flat document.
pub fn write(doc: &Document) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    write_to(doc, &mut bytes)?;
    Ok(bytes)
}

mod compression;
mod runs;
mod writer;

/// Canonical output with at most one 4 MiB payload chunk and one block in scratch.
pub fn write_to(doc: &Document, sink: impl std::io::Write) -> Result<()> {
    writer::write_to(doc, sink, false, &|| Ok(()))
}

/// Smaller archives using lossless pixel runs on every platform and bounded
/// Zstandard frames on native platforms, only where they save bytes.
/// Canonical `write` remains raw and byte-compatible with the published fixtures.
pub fn write_compact(doc: &Document) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    write_compact_to(doc, &mut bytes)?;
    Ok(bytes)
}

pub fn write_compact_to(doc: &Document, sink: impl std::io::Write) -> Result<()> {
    write_compact_to_checked(doc, sink, &|| Ok(()))
}

/// Cooperative block-granular compact writing; failed output remains private to
/// the caller's sink and must be discarded rather than published as a saved file.
pub fn write_compact_to_checked(
    doc: &Document,
    sink: impl std::io::Write,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    writer::write_to(doc, sink, true, check)
}

fn codec(m: impl Into<String>) -> Error {
    Error::new(ErrorCode::CodecFailed, m)
}

fn unsupported(m: impl Into<String>) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, m)
}

fn obj<'a>(
    v: &'a Value,
    what: &str,
    required: &[&str],
    optional: &[&str],
) -> Result<&'a std::collections::BTreeMap<String, Value>> {
    let m = v
        .as_object()
        .ok_or_else(|| codec(format!("{what} must be an object")))?;
    if let Some(k) = required.iter().find(|k| !m.contains_key(**k)) {
        return Err(codec(format!("{what} lacks {k:?}")));
    }
    if let Some(k) = m
        .keys()
        .find(|k| !required.contains(&k.as_str()) && !optional.contains(&k.as_str()))
    {
        return Err(codec(format!("{what} has unknown property {k:?}")));
    }
    Ok(m)
}

fn text<'a>(v: &'a Value, what: &str) -> Result<&'a str> {
    v.as_str()
        .ok_or_else(|| codec(format!("{what} must be a string")))
}

fn range(v: &Value, lo: i64, hi: i64, what: &str) -> Result<i64> {
    match v.as_i64() {
        Some(n) if (lo..=hi).contains(&n) => Ok(n),
        _ => Err(codec(format!("{what} must be an integer in {lo}..={hi}"))),
    }
}

fn uuid(v: &Value, what: &str) -> Result<Uuid> {
    parse_uuid(text(v, what)?).ok_or_else(|| codec(format!("{what} must be a UUID")))
}

fn sha_hex(v: &Value, what: &str) -> Result<String> {
    let s = text(v, what)?;
    if s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        Ok(s.to_owned())
    } else {
        Err(codec(format!("{what} must be lowercase SHA-256")))
    }
}

fn feature_ok(s: &str) -> bool {
    let b = s.as_bytes();
    let seg = |p: &str| {
        !p.is_empty()
            && p.bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    };
    // `[a-z0-9]+([.-][a-z0-9]+)+`
    let parts: Vec<&str> = s.split(['.', '-']).collect();
    !b.is_empty() && parts.len() >= 2 && parts.iter().all(|p| seg(p))
}

fn chunk_name(n: &str) -> bool {
    n.strip_prefix("data/chunk-")
        .and_then(|r| r.strip_suffix(".bin"))
        .is_some_and(|d| d.len() == 6 && d.bytes().all(|c| c.is_ascii_digit()))
}

struct BlockRef {
    bx: u32,
    by: u32,
    entry: String,
    offset: usize,
    len: usize,
    sha: String,
    decoded: usize,
    encoding: String,
}

struct LayerRec {
    layer: Layer,
    blocks: Vec<BlockRef>,
}

/// The structurally valid contents of `manifest.json`, before any support decision.
#[derive(Clone, Debug)]
pub struct Manifest {
    pub container_version: i64,
    pub document_schema: String,
    pub required_features: Vec<String>,
    document_sha256: String,
    document_bytes: usize,
}

impl Manifest {
    /// Refuse anything this build cannot edit: unknown versions, schemas or features.
    pub fn require_supported(&self) -> Result<()> {
        if self.container_version != 1 {
            return Err(unsupported("unknown container version"));
        }
        if self.document_schema != "paintcraft.document/v1" {
            return Err(unsupported("unknown document schema"));
        }
        if let Some(f) = self
            .required_features
            .iter()
            .find(|f| !SUPPORTED_FEATURES.contains(&f.as_str()))
        {
            return Err(unsupported(format!(
                "required feature {f:?} is not supported by this build"
            )));
        }
        for needed in ["raster.rgba8", "canvas.finite"] {
            if !self.required_features.iter().any(|f| f == needed) {
                return Err(codec("manifest lacks mandatory feature declarations"));
            }
        }
        Ok(())
    }
}

fn parse_manifest(bytes: &[u8]) -> Result<Manifest> {
    let manifest = json::parse(bytes).map_err(|e| codec(format!("manifest.json: {e}")))?;
    let m = obj(
        &manifest,
        "manifest",
        &[
            "format",
            "container_version",
            "document_schema",
            "document_entry",
            "document_sha256",
            "document_bytes",
            "required_features",
        ],
        &[],
    )?;
    if text(&m["format"], "format")? != "paintcraft.native"
        || text(&m["document_entry"], "document_entry")? != "document.json"
    {
        return Err(codec("not a PaintCraft native manifest"));
    }
    let features = m["required_features"]
        .as_array()
        .filter(|a| (1..=64).contains(&a.len()))
        .ok_or_else(|| codec("required_features must list 1..=64 features"))?;
    let mut required_features = Vec::new();
    for f in features {
        let f = text(f, "feature")?;
        if !feature_ok(f) {
            return Err(codec("malformed feature identifier"));
        }
        required_features.push(f.to_owned());
    }
    Ok(Manifest {
        container_version: range(&m["container_version"], 1, 65_535, "container_version")?,
        document_schema: text(&m["document_schema"], "document_schema")?.to_owned(),
        required_features,
        document_sha256: sha_hex(&m["document_sha256"], "document_sha256")?,
        document_bytes: range(
            &m["document_bytes"],
            2,
            DOCUMENT_MAX as i64,
            "document_bytes",
        )? as usize,
    })
}

/// Bounded structural probe of a PCND file for open assessment. It never builds a
/// document and never fails because a version or feature is merely unsupported.
pub fn probe(bytes: &[u8]) -> Result<Manifest> {
    let members = zip::read(bytes, ENTRIES_MAX, |name| match name {
        "manifest.json" => Some(MANIFEST_MAX),
        "document.json" => Some(DOCUMENT_MAX),
        n if chunk_name(n) => Some(CHUNK_MAX),
        _ => None,
    })?;
    let manifest = members
        .iter()
        .find(|m| m.0 == "manifest.json")
        .ok_or_else(|| codec("manifest.json is missing"))?;
    parse_manifest(&manifest.1)
}

/// Read a flat PCND v1 file into an editable document.
pub fn read(bytes: &[u8]) -> Result<Document> {
    read_with_limits(bytes, u64::MAX, STAGED_PIXELS_MAX)
}

/// Lower host canvas/output and tile staging limits without changing file semantics.
pub fn read_with_limits(bytes: &[u8], canvas_pixels: u64, staged_pixels: u64) -> Result<Document> {
    read_with_limits_checked(bytes, canvas_pixels, staged_pixels, &|| Ok(()))
}

pub fn read_with_limits_checked(
    bytes: &[u8],
    canvas_pixels: u64,
    staged_pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    check()?;
    let members = zip::read(bytes, ENTRIES_MAX, |name| match name {
        "manifest.json" => Some(MANIFEST_MAX),
        "document.json" => Some(DOCUMENT_MAX),
        n if chunk_name(n) => Some(CHUNK_MAX),
        _ => None,
    })?;
    let mut files: HashMap<String, Vec<u8>> = members.into_iter().collect();
    let manifest_bytes = files
        .remove("manifest.json")
        .ok_or_else(|| codec("manifest.json is missing"))?;
    let document_bytes = files
        .remove("document.json")
        .ok_or_else(|| codec("document.json is missing"))?;

    let manifest = parse_manifest(&manifest_bytes)?;
    manifest.require_supported()?;
    let (doc_sha, doc_len) = (manifest.document_sha256, manifest.document_bytes);
    if document_bytes.len() != doc_len || sha(&document_bytes) != doc_sha {
        return Err(codec(
            "document.json length or SHA-256 does not match the manifest",
        ));
    }

    let document =
        json::parse(&document_bytes).map_err(|e| codec(format!("document.json: {e}")))?;
    let d = obj(
        &document,
        "document",
        &[
            "schema",
            "document_id",
            "state_id",
            "canvas",
            "profile",
            "compositing_policy",
            "persistence_block_edge",
            "roots",
            "layers",
            "metadata",
        ],
        &[],
    )?;
    if text(&d["schema"], "schema")? != "paintcraft.document/v1" {
        return Err(unsupported("unknown document schema"));
    }
    if text(&d["profile"], "profile")? != "srgb"
        || d["persistence_block_edge"].as_i64() != Some(i64::from(TILE_EDGE))
    {
        return Err(unsupported("unsupported color profile or block edge"));
    }
    if text(&d["compositing_policy"], "compositing_policy")? != POLICY {
        return Err(Error::new(
            ErrorCode::PolicyUnknown,
            "document requires an unavailable compositing policy",
        ));
    }
    let canvas = obj(&d["canvas"], "canvas", &["width", "height"], &[])?;
    let canvas = (
        range(&canvas["width"], 1, 1_048_576, "canvas width")? as u32,
        range(&canvas["height"], 1, 1_048_576, "canvas height")? as u32,
    );
    if u64::from(canvas.0) * u64::from(canvas.1) > canvas_pixels {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "canvas exceeds the host pixel limit",
        ));
    }
    let meta = obj(&d["metadata"], "metadata", &[], &["title"])?;
    let title = meta
        .get("title")
        .map(|t| text(t, "title").map(str::to_owned))
        .transpose()?;
    if title.as_ref().is_some_and(|t| t.chars().count() > 4096) {
        return Err(codec("title exceeds 4096 characters"));
    }
    let roots: Vec<Uuid> = d["roots"]
        .as_array()
        .filter(|a| a.len() <= LAYER_COUNT_MAX)
        .ok_or_else(|| codec("roots must be a bounded array"))?
        .iter()
        .map(|r| uuid(r, "root"))
        .collect::<Result<_>>()?;
    let records = d["layers"]
        .as_array()
        .filter(|a| a.len() <= LAYER_COUNT_MAX)
        .ok_or_else(|| codec("layers must be a bounded array"))?;

    let mut parsed: HashMap<Uuid, LayerRec> = HashMap::new();
    for rec in records {
        check()?;
        if rec.get("kind").and_then(Value::as_str) == Some("group") {
            return Err(unsupported("layer groups are not supported by this build"));
        }
        let l = obj(
            rec,
            "layer",
            &[
                "kind",
                "id",
                "name",
                "visible",
                "opacity_u8",
                "blend",
                "origin",
                "width",
                "height",
                "format",
                "default_pixel",
                "blocks",
            ],
            &[],
        )?;
        if text(&l["kind"], "kind")? != "raster"
            || text(&l["blend"], "blend")? != "normal"
            || text(&l["format"], "format")? != "rgba8-straight-srgb"
        {
            return Err(unsupported(
                "unsupported layer kind, blend mode or pixel format",
            ));
        }
        let id = uuid(&l["id"], "layer id")?;
        let name = text(&l["name"], "name")?;
        if name.chars().count() > 1024 {
            return Err(codec("layer name exceeds 1024 characters"));
        }
        let origin = obj(&l["origin"], "origin", &["x", "y"], &[])?;
        let origin = (
            range(&origin["x"], -COORD_ABS_MAX, COORD_ABS_MAX, "origin x")?,
            range(&origin["y"], -COORD_ABS_MAX, COORD_ABS_MAX, "origin y")?,
        );
        let (width, height) = (
            range(&l["width"], 1, 1_048_576, "layer width")? as u32,
            range(&l["height"], 1, 1_048_576, "layer height")? as u32,
        );
        let default = Rgba::parse(text(&l["default_pixel"], "default_pixel")?)
            .ok_or_else(|| codec("default_pixel must be #RRGGBBAA"))?;
        let mut blocks = Vec::new();
        let mut coords = HashSet::new();
        for b in l["blocks"]
            .as_array()
            .ok_or_else(|| codec("blocks must be an array"))?
        {
            check()?;
            let b = obj(
                b,
                "block",
                &[
                    "block_x",
                    "block_y",
                    "entry",
                    "offset",
                    "stored_bytes",
                    "decoded_bytes",
                    "encoding",
                    "sha256",
                ],
                &[],
            )?;
            let (bx, by) = (
                range(&b["block_x"], 0, i64::from(u32::MAX), "block_x")? as u32,
                range(&b["block_y"], 0, i64::from(u32::MAX), "block_y")? as u32,
            );
            if !coords.insert((bx, by)) {
                return Err(codec("duplicate block coordinates"));
            }
            let (w, h) = (
                i64::from(width) - i64::from(TILE_EDGE) * i64::from(bx),
                i64::from(height) - i64::from(TILE_EDGE) * i64::from(by),
            );
            if w <= 0 || h <= 0 {
                return Err(codec("block lies outside its layer"));
            }
            let expect = (w.min(256) * h.min(256) * 4) as u64;
            let decoded = range(
                &b["decoded_bytes"],
                1,
                BLOCK_BYTES_MAX as i64,
                "decoded_bytes",
            )? as u64;
            let stored = range(&b["stored_bytes"], 1, CHUNK_MAX as i64, "stored_bytes")? as u64;
            let offset = range(&b["offset"], 0, CHUNK_MAX as i64, "offset")? as usize;
            if decoded != expect {
                return Err(codec("decoded_bytes does not match the block extent"));
            }
            let encoding = text(&b["encoding"], "encoding")?;
            match encoding {
                "raw" if stored == decoded => (),
                "raw" => {
                    return Err(codec(
                        "raw block stored length differs from its decoded length",
                    ));
                }
                "zstd" | "rgba-runs" => (),
                _ => return Err(codec("unknown block encoding")),
            };
            let entry = text(&b["entry"], "entry")?;
            if !chunk_name(entry) {
                return Err(codec("malformed chunk entry name"));
            }
            blocks.push(BlockRef {
                bx,
                by,
                entry: entry.to_owned(),
                offset,
                len: stored as usize,
                sha: sha_hex(&b["sha256"], "block sha256")?,
                decoded: decoded as usize,
                encoding: encoding.to_owned(),
            });
        }
        let layer = Layer {
            id,
            name: name.to_owned(),
            origin,
            visible: l["visible"]
                .as_bool()
                .ok_or_else(|| codec("visible must be boolean"))?,
            opacity: range(&l["opacity_u8"], 0, 255, "opacity_u8")? as u8,
            raster: RasterLayer::new(width, height, default),
        };
        if parsed.insert(id, LayerRec { layer, blocks }).is_some() {
            return Err(codec("duplicate layer id"));
        }
    }

    // Flat forest: every root exists once and every record is reachable.
    let mut seen = HashSet::new();
    if roots
        .iter()
        .any(|r| !parsed.contains_key(r) || !seen.insert(*r))
        || seen.len() != parsed.len()
    {
        return Err(codec("roots do not form a flat forest of every layer"));
    }

    // Blocks: chunk ranges, overlap consistency, lengths and hashes.
    let mut used: HashSet<String> = HashSet::new();
    let mut ranges: HashMap<String, Vec<(usize, usize, String)>> = HashMap::new();
    let mut adm = Admission::new(staged_pixels.min(STAGED_PIXELS_MAX));
    let tile_count = parsed.values().try_fold(0u64, |count, rec| {
        count
            .checked_add(rec.blocks.len() as u64)
            .ok_or_else(|| codec("block count overflows"))
    })?;
    adm.reserve_tiles(tile_count)?;
    let mut layers = HashMap::new();
    for id in &roots {
        check()?;
        let rec = parsed.remove(id).expect("checked above");
        let LayerRec { mut layer, blocks } = rec;
        for b in &blocks {
            check()?;
            let chunk = files
                .get(&b.entry)
                .ok_or_else(|| codec("block references a missing chunk"))?;
            used.insert(b.entry.clone());
            let end = b
                .offset
                .checked_add(b.len)
                .filter(|&e| e <= chunk.len())
                .ok_or_else(|| codec("block lies outside its chunk"))?;
            let seen = ranges.entry(b.entry.clone()).or_default();
            if seen.iter().any(|&(s, e, ref h)| {
                b.offset < e && s < end && (s, e, h.as_str()) != (b.offset, end, b.sha.as_str())
            }) {
                return Err(codec("partially overlapping blocks"));
            }
            seen.push((b.offset, end, b.sha.clone()));
            let stored = &chunk[b.offset..end];
            let decoded;
            let data = match b.encoding.as_str() {
                "zstd" => {
                    decoded = compression::decode(stored, b.decoded, check)?;
                    decoded.as_slice()
                }
                "rgba-runs" => {
                    decoded = runs::decode(stored, b.decoded, check)?;
                    decoded.as_slice()
                }
                _ => stored,
            };
            if sha(data) != b.sha {
                return Err(codec("block SHA-256 mismatch"));
            }
            layer.raster.set_block(b.bx, b.by, data, &mut adm)?;
        }
        layers.insert(*id, layer);
    }
    if files.keys().any(|n| !used.contains(n)) {
        return Err(codec("archive contains an unreferenced member"));
    }
    check()?;
    Ok(Document {
        document_id: uuid(&d["document_id"], "document_id")?,
        state_id: uuid(&d["state_id"], "state_id")?,
        canvas,
        roots,
        layers,
        title,
    })
}

#[cfg(test)]
mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod compact_tests;
