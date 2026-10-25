//! Fresh icon/cursor asset sets, with explicit per-entry encoding and hotspots.
//! Callers supply each entry's pixels at its intended size; no implicit resize.
use super::*;
use std::io::{self, Write};

pub const WRITE_PIXELS_MAX: u64 = 4_194_304;

#[derive(Clone, Copy, Debug)]
pub struct WriteLimits {
    pub entries: usize,
    pub pixels: u64,
    pub encoded_bytes: usize,
}
impl Default for WriteLimits {
    fn default() -> Self {
        Self {
            entries: ENTRIES_MAX,
            pixels: WRITE_PIXELS_MAX,
            encoded_bytes: ENCODED_BYTES_MAX,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WriteEntry<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    pub encoding: Encoding,
    /// Required for every cursor entry and forbidden for every icon entry.
    pub hotspot: Option<(u16, u16)>,
}

fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn schema(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}

pub(super) fn admit_layout(
    kind: Kind,
    width: u32,
    height: u32,
    hotspot: Option<(u16, u16)>,
) -> Result<u64> {
    if !(1..=256).contains(&width) || !(1..=256).contains(&height) {
        return Err(schema("icon output dimensions must be between 1 and 256"));
    }
    match (kind, hotspot) {
        (Kind::Icon, None) => {}
        (Kind::Cursor, Some((x, y))) if u32::from(x) < width && u32::from(y) < height => {}
        _ => {
            return Err(schema(
                "icon output hotspot missing, unexpected, or outside entry",
            ));
        }
    }
    Ok(u64::from(width) * u64::from(height))
}

pub(super) fn preflight(kind: Kind, entries: &[WriteEntry<'_>], limits: WriteLimits) -> Result<()> {
    if entries.is_empty() || entries.len() > limits.entries.min(ENTRIES_MAX) {
        return Err(resource("icon output entry count outside writer budget"));
    }
    let mut pixels = 0u64;
    for entry in entries {
        let count = admit_layout(kind, entry.width, entry.height, entry.hotspot)?;
        pixels += count;
        if pixels > limits.pixels.min(WRITE_PIXELS_MAX) {
            return Err(resource(
                "icon output aggregate pixels exceed writer budget",
            ));
        }
        if entry.rgba.len() as u64 != count * 4 {
            return Err(schema("icon output RGBA length contradicts dimensions"));
        }
        if entry.encoding == Encoding::Bitmap
            && entry
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 0)
        {
            return Err(unsupported(
                "fully transparent bitmap icon output requires PNG encoding",
            ));
        }
    }
    if 6 + entries.len() * 16 > limits.encoded_bytes.min(ENCODED_BYTES_MAX) {
        return Err(resource("icon output directory exceeds byte budget"));
    }
    Ok(())
}

fn bitmap32(entry: &WriteEntry<'_>, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    let mask_stride = entry.width.div_ceil(32) as usize * 4;
    let mask_size = mask_stride * entry.height as usize;
    let mut data = Vec::new();
    data.try_reserve_exact(40 + entry.rgba.len() + mask_size)
        .map_err(|_| resource("cannot allocate icon bitmap"))?;
    for value in [40, entry.width, entry.height * 2] {
        data.extend_from_slice(&value.to_le_bytes());
    }
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&32u16.to_le_bytes());
    data.resize(40, 0);
    for row in entry.rgba.chunks_exact(entry.width as usize * 4).rev() {
        check()?;
        for pixel in row.as_chunks::<4>().0 {
            data.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    let mask_start = data.len();
    data.resize(mask_start + mask_size, 0);
    for (row, pixels) in entry
        .rgba
        .chunks_exact(entry.width as usize * 4)
        .rev()
        .enumerate()
    {
        check()?;
        for (column, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
            if pixel[3] == 0 {
                data[mask_start + row * mask_stride + column / 8] |= 0x80 >> (column % 8);
            }
        }
    }
    Ok(data)
}

// The selected library's automatic PNG writer emits RGB for opaque samples,
// and its bitmap writer can emit 24bpp. Build explicit ARGB payloads instead:
// https://devblogs.microsoft.com/oldnewthing/20101022-00/?p=12473
// https://devblogs.microsoft.com/oldnewthing/20101021-00/?p=12483
fn packed_entry(kind: Kind, entry: &WriteEntry<'_>, payload: Vec<u8>) -> Result<ico::IconDirEntry> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(22 + payload.len())
        .map_err(|_| resource("cannot allocate icon entry envelope"))?;
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&(if kind == Kind::Icon { 1u16 } else { 2 }).to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&[entry.width as u8, entry.height as u8, 0, 0]);
    let fields = entry.hotspot.unwrap_or((1, 32));
    bytes.extend_from_slice(&fields.0.to_le_bytes());
    bytes.extend_from_slice(&fields.1.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&22u32.to_le_bytes());
    bytes.extend_from_slice(&payload);
    let directory = ico::IconDir::read(io::Cursor::new(bytes))
        .map_err(|error| malformed(&format!("cannot assemble icon entry: {error}")))?;
    // Upstream exposes entries by reference; the payload is bounded to one
    // 256x256 entry, so this extra clone cannot scale with the whole asset set.
    Ok(directory.entries()[0].clone())
}

struct Output<'a> {
    bytes: Vec<u8>,
    limit: usize,
    check: &'a dyn Fn() -> Result<()>,
    error: Option<Error>,
}
impl Write for Output<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = (|| {
            let end = self
                .bytes
                .len()
                .checked_add(bytes.len())
                .filter(|end| *end <= self.limit)
                .ok_or_else(|| resource("icon encoded output exceeds byte budget"))?;
            self.bytes
                .try_reserve_exact(end - self.bytes.len())
                .map_err(|_| resource("cannot allocate icon encoded output"))?;
            for chunk in bytes.chunks(65536) {
                (self.check)()?;
                self.bytes.extend_from_slice(chunk);
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.error = Some(error);
            return Err(io::Error::other("icon output interrupted"));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Encode a fresh asset set privately. Entry order, dimensions, exact RGBA and
/// cursor hotspots are explicit; no foreign container is overwritten or merged.
/// Checks run before/after bounded per-entry codec calls and while copying output.
/// Upstream codec scratch allocations and within-call cancellation are not hard
/// memory/latency guarantees. Returned bytes are unpublished until caller saves.
pub fn encode_entries(
    kind: Kind,
    entries: &[WriteEntry<'_>],
    limits: WriteLimits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    preflight(kind, entries, limits)?;
    check()?;
    let mut directory = ico::IconDir::new(if kind == Kind::Icon {
        ico::ResourceType::Icon
    } else {
        ico::ResourceType::Cursor
    });
    let mut encoded = 6 + entries.len() * 16;
    for entry in entries {
        check()?;
        let payload = if entry.encoding == Encoding::Png {
            crate::png::encode_rgba8(entry.width, entry.height, entry.rgba)?
        } else {
            bitmap32(entry, check)?
        };
        encoded = encoded
            .checked_add(payload.len())
            .filter(|size| *size <= limits.encoded_bytes.min(ENCODED_BYTES_MAX))
            .ok_or_else(|| resource("icon encoded entries exceed byte budget"))?;
        check()?;
        directory.add_entry(packed_entry(kind, entry, payload)?);
    }
    let mut output = Output {
        bytes: Vec::new(),
        limit: limits.encoded_bytes.min(ENCODED_BYTES_MAX),
        check,
        error: None,
    };
    output
        .bytes
        .try_reserve_exact(encoded)
        .map_err(|_| resource("cannot allocate icon output"))?;
    if let Err(error) = directory.write(&mut output) {
        return Err(output
            .error
            .unwrap_or_else(|| malformed(&format!("icon container encoding failed: {error}"))));
    }
    check()?;
    Ok(output.bytes)
}
