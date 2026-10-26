//! Fresh observed PAINT item profile, separate from original-application gates.
use super::*;
use std::io::Write;
mod container;
const CHUNK: usize = 65536;
// Same fixed flate2 1.1.10/miniz_oxide 0.9.1 state audited for PDN;
// conservative logical allowance, not an allocator/RSS ceiling.
const CODEC_SCRATCH: usize = 1 << 20;
#[derive(Clone, Copy, Debug)]
pub struct WriteLayer<'a> {
    pub width: u32,
    pub height: u32,
    pub origin: (i32, i32),
    /// Literal infe item name only; not asserted to be a UI layer name.
    pub item_name: &'a str,
    /// Exact straight encoded-sRGB RGBA8, including RGB under zero alpha.
    pub rgba: &'a [u8],
    /// Unknown extension semantics are refused, not silently dropped.
    pub visible: bool,
    pub opacity: u8,
    pub metadata: &'a [(&'a str, &'a str)],
}
#[derive(Clone, Copy, Debug)]
pub struct WriteDocument<'a> {
    pub width: u32,
    pub height: u32,
    /// Exact iovl RGBA fill fields; never inferred from a raster background.
    pub canvas_fill: [u16; 4],
    /// Bottom to top, each with its own extent and placement.
    pub layers: &'a [WriteLayer<'a>],
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
    pub wide_overlay: bool,
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| resource("PAINT write size overflow"))
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| resource("PAINT write size overflow"))
}
fn dimensions(width: u32, height: u32, limits: &WriteLimits) -> Result<u64> {
    if width == 0 || height == 0 || width > 65536 || height > 65536 {
        return Err(unsupported(
            "PAINT write dimensions outside initial profile",
        ));
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > limits.pixels.min(4_194_304) {
        return Err(resource("PAINT write pixel allowance"));
    }
    Ok(pixels)
}
/// Admit the full source before allocating metadata, BGRA, codec or output.
pub fn preflight_write(
    doc: &WriteDocument<'_>,
    limits: &WriteLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<WritePlan> {
    check()?;
    dimensions(doc.width, doc.height, limits)?;
    if doc.layers.is_empty() || doc.layers.len() > limits.layers.min(64) {
        return Err(resource("PAINT write needs 1..64 admitted layers"));
    }
    if !doc.metadata.is_empty() {
        return Err(unsupported("PAINT document metadata is unqualified"));
    }
    let mut pixels = 0u64;
    let mut metadata = add(4096, mul(doc.layers.len(), 512)?)?;
    let mut wide = doc.width > u16::MAX as u32 || doc.height > u16::MAX as u32;
    for layer in doc.layers {
        check()?;
        let count = dimensions(layer.width, layer.height, limits)?;
        pixels = pixels
            .checked_add(count)
            .ok_or_else(|| resource("PAINT aggregate pixel overflow"))?;
        if pixels > limits.pixels.min(4_194_304) {
            return Err(resource("PAINT aggregate write pixel allowance"));
        }
        if count * 4 != layer.rgba.len() as u64 {
            return Err(malformed("PAINT layer RGBA length disagrees with geometry"));
        }
        if !layer.visible || layer.opacity != 255 || !layer.metadata.is_empty() {
            return Err(unsupported(
                "PAINT visibility, opacity or metadata extensions are unqualified",
            ));
        }
        if layer.item_name.len() > 1024 || layer.item_name.contains('\0') {
            return Err(unsupported("PAINT item name outside UTF-8 profile"));
        }
        metadata = add(metadata, layer.item_name.len())?;
        wide |= i16::try_from(layer.origin.0).is_err() || i16::try_from(layer.origin.1).is_err();
    }
    if metadata > limits.metadata_bytes.min(1 << 20) {
        return Err(resource("PAINT write metadata allowance"));
    }
    let source_bytes =
        usize::try_from(pixels * 4).map_err(|_| resource("PAINT source exceeds address space"))?;
    // Selected miniz15-bit coding/stored fallback; hard append checks enforce
    // the reservation independently of this conservative growth declaration.
    let output_capacity = add(
        add(mul(source_bytes, 2)?, mul(doc.layers.len(), 1024)?)?,
        metadata,
    )?;
    if output_capacity > limits.encoded_bytes.min(64 << 20) {
        return Err(resource("PAINT encoded write allowance"));
    }
    let transient = add(
        add(source_bytes, output_capacity)?,
        add(add(CHUNK, CODEC_SCRATCH)?, metadata)?,
    )?;
    if transient > limits.transient_bytes {
        return Err(resource("PAINT transient write allowance"));
    }
    Ok(WritePlan {
        source_bytes,
        metadata_bytes: metadata,
        output_capacity,
        transient_bytes: transient,
        wide_overlay: wide,
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
            .map_err(|_| resource("Cannot allocate PAINT private output"))?;
        Ok(Self { bytes, limit })
    }
    fn put(&mut self, data: &[u8]) -> Result<()> {
        if add(self.bytes.len(), data.len())? > self.limit {
            return Err(resource("PAINT output allowance"));
        }
        self.bytes.extend_from_slice(data);
        Ok(())
    }
    fn u16(&mut self, value: u16) -> Result<()> {
        self.put(&value.to_be_bytes())
    }
    fn u32(&mut self, value: u32) -> Result<()> {
        self.put(&value.to_be_bytes())
    }
    fn start(&mut self, kind: &[u8; 4]) -> Result<usize> {
        let start = self.bytes.len();
        self.put(&[0; 4])?;
        self.put(kind)?;
        Ok(start)
    }
    fn end(&mut self, start: usize) -> Result<()> {
        let length = u32::try_from(
            self.bytes
                .len()
                .checked_sub(start)
                .ok_or_else(|| malformed("PAINT box starts outside output"))?,
        )
        .map_err(|_| resource("PAINT box length overflow"))?;
        self.patch(start, length)
    }
    fn patch(&mut self, offset: usize, value: u32) -> Result<()> {
        self.bytes
            .get_mut(
                offset
                    ..offset
                        .checked_add(4)
                        .ok_or_else(|| resource("PAINT fixup overflow"))?,
            )
            .ok_or_else(|| malformed("PAINT internal fixup outside output"))?
            .copy_from_slice(&value.to_be_bytes());
        Ok(())
    }
}
impl Write for Output {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.put(data)
            .map_err(|_| std::io::Error::other("PAINT output allowance"))?;
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
/// Serialize entirely new boxes and exact channels. No source template or
/// private metadata is copied; source/destination authority remains caller-owned.
pub fn encode_document(
    doc: &WriteDocument<'_>,
    limits: &WriteLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let plan = preflight_write(doc, limits, check)?;
    let mut out = Output::new(plan.output_capacity)?;
    let locations = container::metadata(&mut out, doc, &plan, check)?;
    let free = out.start(b"free")?;
    out.end(free)?;
    let mdat = out.start(b"mdat")?;
    let mut bgra = Vec::new();
    bgra.try_reserve_exact(CHUNK)
        .map_err(|_| resource("Cannot allocate PAINT BGRA chunk"))?;
    bgra.resize(CHUNK, 0);
    for (index, layer) in doc.layers.iter().enumerate() {
        check()?;
        let start = out.bytes.len();
        out.patch(locations[index], start as u32)?;
        let mut deflate =
            flate2::write::DeflateEncoder::new(&mut out, flate2::Compression::default());
        for source in layer.rgba.chunks(CHUNK) {
            check()?;
            for (source, target) in source
                .as_chunks::<4>()
                .0
                .iter()
                .zip(bgra.as_chunks_mut::<4>().0.iter_mut())
            {
                *target = [source[2], source[1], source[0], source[3]];
            }
            deflate
                .write_all(&bgra[..source.len()])
                .map_err(|_| resource("PAINT DEFLATE output allowance"))?;
        }
        deflate
            .finish()
            .map_err(|_| resource("PAINT DEFLATE finish allowance"))?;
        let length = u32::try_from(out.bytes.len() - start)
            .map_err(|_| resource("PAINT extent length overflow"))?;
        out.patch(locations[index] + 4, length)?;
        check()?;
    }
    out.end(mdat)?;
    check()?;
    Ok(out.bytes)
}
/// Write only to a caller-owned private staged sink, never publish a pathname.
pub fn write_document_to(
    doc: &WriteDocument<'_>,
    limits: &WriteLimits,
    sink: &mut impl Write,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let bytes = encode_document(doc, limits, check)?;
    for chunk in bytes.chunks(CHUNK) {
        check()?;
        sink.write_all(chunk)
            .map_err(|_| Error::new(ErrorCode::StorageFailed, "Cannot write staged PAINT copy"))?;
    }
    check()
}
