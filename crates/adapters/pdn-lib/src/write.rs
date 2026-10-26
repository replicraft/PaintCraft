//! Fresh observed PDN3 Normal bitmap graph, not preserving-save authority.
//! Logical bounds include borrowed input and fixed codec scratch; they are not
//! an allocator/RSS cap. Original Paint.NET qualification remains separate.
use super::*;
use std::io::Write;
mod graph;
const CHUNK: usize = 262_144;
// flate2 1.1.10 / miniz_oxide 0.9.1 fixed state + IO buffer, with
// initialization/stack headroom; see fixture README audit. Logical allowance,
// not interception of allocator failures or a portable OS-memory ceiling.
const CODEC_SCRATCH: usize = 1 << 20;
#[derive(Clone, Copy, Debug)]
pub struct RasterLayer<'a> {
    pub width: u32,
    pub height: u32,
    pub name: &'a str,
    pub visible: bool,
    pub opacity: u8,
    /// Explicit source role; never inferred from the name or stack position.
    pub is_background: bool,
    pub rgba: &'a [u8],
    /// This initial writer refuses nonempty source metadata, never discards it.
    pub metadata: &'a [(&'a str, &'a str)],
}
#[derive(Clone, Copy, Debug)]
pub struct RasterDocument<'a> {
    pub width: u32,
    pub height: u32,
    /// Bottom to top, with explicit full-canvas geometry for every layer.
    pub layers: &'a [RasterLayer<'a>],
    /// Literal producer/version fields, not an inferred Paint.NET claim.
    pub saved_with: [u32; 4],
    pub metadata: &'a [(&'a str, &'a str)],
}
#[derive(Clone, Copy, Debug)]
pub struct WriteLimits {
    pub encoded_bytes: usize,
    pub metadata_bytes: usize,
    pub pixels: u64,
    pub layers: usize,
    pub transient_bytes: usize,
}
impl Default for WriteLimits {
    fn default() -> Self {
        Self {
            encoded_bytes: 64 << 20,
            metadata_bytes: 1 << 20,
            pixels: 4_194_304,
            layers: 64,
            transient_bytes: 512 << 20,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WritePlan {
    pub source_bytes: usize,
    pub metadata_bytes: usize,
    pub output_capacity: usize,
    pub transient_bytes: usize,
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| resource("PDN write size overflow"))
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| resource("PDN write size overflow"))
}
/// Validate the complete source before any output or codec allocation.
pub fn preflight_write(
    doc: &RasterDocument<'_>,
    limits: &WriteLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<WritePlan> {
    check()?;
    if doc.width == 0
        || doc.height == 0
        || doc.width > 32768
        || doc.height > 32768
        || doc.layers.is_empty()
    {
        return Err(unsupported(
            "PDN writer needs explicit 1..32768 full-canvas bitmap geometry",
        ));
    }
    if doc.layers.len() > limits.layers.min(64) {
        return Err(resource("PDN writer layer allowance"));
    }
    if !doc.metadata.is_empty() || doc.saved_with.iter().any(|value| *value > i32::MAX as u32) {
        return Err(unsupported(
            "PDN writer cannot retain supplied metadata or version fields",
        ));
    }
    let pixels = u64::from(doc.width) * u64::from(doc.height);
    if pixels
        .checked_mul(doc.layers.len() as u64)
        .is_none_or(|n| n > limits.pixels.min(4_194_304))
    {
        return Err(resource("PDN writer aggregate pixel allowance"));
    }
    let layer_bytes = usize::try_from(
        pixels
            .checked_mul(4)
            .ok_or_else(|| resource("PDN pixel size overflow"))?,
    )
    .map_err(|_| resource("PDN pixels exceed address space"))?;
    let source_bytes = mul(layer_bytes, doc.layers.len())?;
    let mut metadata = add(4096, mul(doc.layers.len(), 8192)?)?;
    for layer in doc.layers {
        check()?;
        if (layer.width, layer.height) != (doc.width, doc.height) || layer.rgba.len() != layer_bytes
        {
            return Err(unsupported(
                "PDN writer does not pad, clip or resize layer geometry",
            ));
        }
        if !layer.metadata.is_empty() {
            return Err(unsupported(
                "PDN writer cannot retain source layer metadata",
            ));
        }
        if layer.name.len() > 4096 || layer.name.contains('\0') {
            return Err(unsupported(
                "PDN writer layer name outside initial UTF-8 profile",
            ));
        }
        metadata = add(metadata, layer.name.len())?;
    }
    if metadata > limits.metadata_bytes.min(1 << 20) {
        return Err(resource("PDN writer metadata allowance"));
    }
    // Conservative selected-codec reservation: 15-bit literal/match coding,
    // >=31 KiB nonfinal dynamic blocks, stored-block fallback, and <=256 KiB
    // input per gzip member. Output::put also enforces this ceiling during IO;
    // changed backend growth is a resource error, never an unbounded Vec grow.
    let chunks = mul(layer_bytes.div_ceil(CHUNK), doc.layers.len())?;
    let output_capacity = add(add(mul(source_bytes, 2)?, mul(chunks, 1032)?)?, metadata)?;
    if output_capacity > limits.encoded_bytes.min(64 << 20) {
        return Err(resource("PDN writer encoded allowance"));
    }
    let transient_bytes = add(
        add(source_bytes, output_capacity)?,
        add(add(layer_bytes.min(CHUNK), CODEC_SCRATCH)?, metadata)?,
    )?;
    if transient_bytes > limits.transient_bytes {
        return Err(resource("PDN writer transient allowance"));
    }
    Ok(WritePlan {
        source_bytes,
        metadata_bytes: metadata,
        output_capacity,
        transient_bytes,
    })
}
struct Output {
    bytes: Vec<u8>,
    limit: usize,
}
impl Output {
    fn new(limit: usize) -> Result<Self> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(limit)
            .map_err(|_| resource("Cannot allocate PDN private output"))?;
        Ok(Self { bytes, limit })
    }
    fn put(&mut self, bytes: &[u8]) -> Result<()> {
        if add(self.bytes.len(), bytes.len())? > self.limit {
            return Err(resource("PDN private output allowance"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn le(&mut self, value: i32) -> Result<()> {
        self.put(&value.to_le_bytes())
    }
    fn text(&mut self, value: &str) -> Result<()> {
        let mut n = value.len();
        while n >= 128 {
            self.put(&[(n as u8) | 128])?;
            n >>= 7;
        }
        self.put(&[n as u8])?;
        self.put(value.as_bytes())
    }
}
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.put(bytes)
            .map_err(|_| std::io::Error::other("PDN private output allowance"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
/// Encode a fresh typed copy. No original bytes, opaque records or runtime types
/// are copied; this API owns neither source authority nor atomic publication.
pub fn encode_document(
    doc: &RasterDocument<'_>,
    limits: &WriteLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let plan = preflight_write(doc, limits, check)?;
    let mut out = Output::new(plan.output_capacity)?;
    let version = doc.saved_with.map(|v| v.to_string()).join(".");
    let header = format!(
        "<pdnImage width=\"{}\" height=\"{}\" layers=\"{}\" savedWithVersion=\"{}\"><custom></custom></pdnImage>",
        doc.width,
        doc.height,
        doc.layers.len(),
        version
    );
    out.put(b"PDN3")?;
    out.put(&(header.len() as u32).to_le_bytes()[..3])?;
    out.put(header.as_bytes())?;
    out.put(&[0, 1])?;
    graph::write(&mut out, doc, check)?;
    let mut bgra = Vec::new();
    let chunk_bytes = doc.layers[0].rgba.len().min(CHUNK);
    bgra.try_reserve_exact(chunk_bytes)
        .map_err(|_| resource("Cannot allocate PDN bounded BGRA chunk"))?;
    bgra.resize(chunk_bytes, 0);
    for layer in doc.layers {
        check()?;
        out.put(&[0])?;
        out.put(&(CHUNK as u32).to_be_bytes())?;
        for (index, source) in layer.rgba.chunks(CHUNK).enumerate() {
            check()?;
            for (source, target) in source
                .as_chunks::<4>()
                .0
                .iter()
                .zip(bgra.as_chunks_mut::<4>().0.iter_mut())
            {
                *target = [source[2], source[1], source[0], source[3]];
            }
            out.put(&(index as u32).to_be_bytes())?;
            let length_offset = out.bytes.len();
            out.put(&[0; 4])?;
            let start = out.bytes.len();
            let mut gzip = flate2::write::GzEncoder::new(&mut out, flate2::Compression::default());
            for part in bgra[..source.len()].chunks(65536) {
                check()?;
                gzip.write_all(part)
                    .map_err(|_| resource("PDN gzip output allowance"))?;
            }
            gzip.finish()
                .map_err(|_| resource("PDN gzip finish allowance"))?;
            let length = u32::try_from(out.bytes.len() - start)
                .map_err(|_| resource("PDN compressed chunk overflow"))?;
            out.bytes[length_offset..length_offset + 4].copy_from_slice(&length.to_be_bytes());
            check()?;
        }
    }
    check()?;
    Ok(out.bytes)
}
/// Only a private staged sink is accepted; callers retain publication authority.
pub fn write_document_to(
    doc: &RasterDocument<'_>,
    limits: &WriteLimits,
    sink: &mut impl Write,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let bytes = encode_document(doc, limits, check)?;
    for chunk in bytes.chunks(65536) {
        check()?;
        sink.write_all(chunk)
            .map_err(|_| Error::new(ErrorCode::StorageFailed, "Cannot write staged PDN copy"))?;
    }
    check()
}
