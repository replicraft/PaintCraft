//! Finite visual-only protocol. No file-copy or publication authority is carried.
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Result,
    json::{self, Value},
};
use uuid::Uuid;
pub const SOURCE_BYTES_MAX: u64 = 64 << 20;
pub const LINEAR_MEMORY_BYTES: u64 = 256 << 20;
pub const OUTPUT_PIXELS_MAX: u64 = 4_194_304;
pub const CACHE_BYTES: usize = 4 << 20;
pub const CONTROL_BYTES_MAX: usize = 4096;
pub fn bad(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
pub fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
pub fn stale() -> Error {
    Error::new(
        ErrorCode::ConflictStaleBase,
        "Filter preview owner or generation changed",
    )
}
pub fn parse(bytes: &str) -> Result<Value> {
    if bytes.len() > CONTROL_BYTES_MAX {
        return Err(resource("Filter preview control size"));
    }
    json::parse(bytes.as_bytes()).map_err(|_| bad("Malformed filter preview control"))
}
pub fn field<'a>(v: &'a Value, key: &str) -> Result<&'a Value> {
    v.get(key)
        .ok_or_else(|| bad("Missing filter preview field"))
}
pub fn object(v: &Value, names: &[&str]) -> Result<()> {
    let o = v
        .as_object()
        .ok_or_else(|| bad("Expected filter preview object"))?;
    if o.len() != names.len() || o.keys().any(|k| !names.contains(&k.as_str())) {
        return Err(bad("Unexpected filter preview fields"));
    }
    Ok(())
}
pub fn string(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(|| bad("Expected string"))
}
pub fn uuid(v: &Value) -> Result<Uuid> {
    Uuid::parse_str(string(v)?).map_err(|_| bad("Expected UUID"))
}
pub fn integer(v: &Value) -> Result<u64> {
    u64::try_from(v.as_i64().ok_or_else(|| bad("Expected unsigned integer"))?)
        .map_err(|_| bad("Negative integer"))
}
pub fn decimal(v: &Value) -> Result<u64> {
    let s = string(v)?;
    if s.is_empty()
        || s.len() > 20
        || (s.len() > 1 && s.starts_with('0'))
        || !s.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(bad("Expected canonical decimal u64"));
    }
    s.parse().map_err(|_| bad("Decimal exceeds u64"))
}
pub fn digest(v: &Value) -> Result<&str> {
    let s = string(v)?;
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(bad("Expected lowercase SHA256"));
    }
    Ok(s)
}
pub fn text(v: &Value) -> String {
    String::from_utf8(json::canonical(v)).expect("JSON is UTF8")
}
#[derive(Clone, Debug)]
pub struct Source {
    pub value: Value,
    pub state: Uuid,
    pub document: Uuid,
    pub size: (u32, u32),
    pub layers: usize,
    pub resident: u64,
    pub metadata: u64,
    pub metadata_charge: u64,
    pub quote: u64,
}
impl Source {
    pub fn parse(value: Value) -> Result<Self> {
        object(
            &value,
            &[
                "owner",
                "draft",
                "document",
                "sourceSha256",
                "size",
                "layers",
                "residentBytes",
                "metadataBytes",
                "metadataCharge",
                "filterDigest",
            ],
        )?;
        let owner = field(&value, "owner")?;
        object(owner, &["session", "state", "epoch", "frontend"])?;
        uuid(field(owner, "session")?)?;
        let state = uuid(field(owner, "state")?)?;
        decimal(field(owner, "epoch")?)?;
        if integer(field(owner, "frontend")?)? != 3 {
            return Err(bad("Filter preview is Studio-only"));
        }
        uuid(field(&value, "draft")?)?;
        let document = uuid(field(&value, "document")?)?;
        digest(field(&value, "sourceSha256")?)?;
        digest(field(&value, "filterDigest")?)?;
        let size = field(&value, "size")?
            .as_array()
            .filter(|v| v.len() == 2)
            .ok_or_else(|| bad("Expected size pair"))?;
        let side = |v: &Value| -> Result<u32> {
            let n = integer(v)?;
            if !(1..=1_048_576).contains(&n) {
                return Err(bad("Canvas side exceeds canonical policy"));
            }
            Ok(n as u32)
        };
        let size = (side(&size[0])?, side(&size[1])?);
        let layers = usize::try_from(integer(field(&value, "layers")?)?)
            .map_err(|_| resource("Layer count"))?;
        if layers > paintcraft_document::LAYER_COUNT_MAX {
            return Err(resource("Layer count"));
        }
        let resident = integer(field(&value, "residentBytes")?)?;
        if !resident.is_multiple_of(paintcraft_raster::RasterLayer::TILE_BYTES as u64) {
            return Err(bad("Resident allowance must price full padded tiles"));
        }
        let metadata = integer(field(&value, "metadataBytes")?)?;
        let metadata_charge = integer(field(&value, "metadataCharge")?)?;
        if metadata > 8 << 20 || (metadata_charge == 0 && metadata != 0) {
            return Err(resource("Retained metadata declaration"));
        }
        // Price before decoding: natural per-layer filter cap, three private
        // metadata copies during execute plan/replay, wire input, caches,
        // output/transfer, and existing aggregate metadata/parser allowance.
        let per_layer = std::mem::size_of::<paintcraft_document::Layer>()
            + 256 * std::mem::size_of::<paintcraft_document::FilterInstance>();
        let quote = (layers as u64)
            .checked_mul(per_layer as u64)
            .and_then(|n| n.checked_mul(3))
            .and_then(|n| n.checked_add(resident))
            .and_then(|n| n.checked_add(metadata_charge))
            .and_then(|n| {
                n.checked_add(
                    SOURCE_BYTES_MAX
                        + 2 * OUTPUT_PIXELS_MAX * 4
                        + 2 * CACHE_BYTES as u64
                        + (16 << 20),
                )
            })
            .ok_or_else(|| resource("Filter preview workspace overflow"))?;
        // Leave a declared 32MiB for Wasm runtime/stack/allocator behavior.
        // Logical payload admission is not interception of allocator or RSS.
        if quote > LINEAR_MEMORY_BYTES - (32 << 20) {
            return Err(resource(
                "Filter preview workspace exceeds linear-memory profile",
            ));
        }
        Ok(Self {
            value,
            state,
            document,
            size,
            layers,
            resident,
            metadata,
            metadata_charge,
            quote,
        })
    }
}
#[derive(Clone, Debug)]
pub struct View {
    pub value: Value,
    pub rect: Rect,
    pub level: u8,
}
impl View {
    pub fn parse(value: &Value) -> Result<Self> {
        object(value, &["region", "level"])?;
        let a = field(value, "region")?
            .as_array()
            .filter(|a| a.len() == 4)
            .ok_or_else(|| bad("Expected ROI quadruple"))?;
        let mut n = [0i64; 4];
        for (dst, v) in n.iter_mut().zip(a) {
            *dst = v.as_i64().ok_or_else(|| bad("Expected ROI integers"))?;
        }
        let rect = Rect::new(n[0], n[1], n[2], n[3]).ok_or_else(|| bad("Invalid ROI"))?;
        let level = integer(field(value, "level")?)?;
        if level > 20 {
            return Err(bad("Display level"));
        }
        Ok(Self {
            value: value.clone(),
            rect,
            level: level as u8,
        })
    }
}
