//! Bounded ICO/CUR inspection. A directory is an asset set, not one raster.
use paintcraft_core::types::{Error, ErrorCode, Result};

mod payload;
mod write;
pub use write::{WRITE_PIXELS_MAX, WriteEntry, WriteLimits, encode_entries};

pub const ENTRIES_MAX: usize = 256;
pub const ENCODED_BYTES_MAX: usize = 64 * 1024 * 1024;
pub const KNOWN_IMPORT_LOSSES: &[&str] = &[
    "icon-palette-expanded",
    "icon-other-entries-not-imported",
    "cursor-hotspot-not-retained-in-flat-document",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Icon,
    Cursor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Png,
    Bitmap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub index: u16,
    pub width: u32,
    pub height: u32,
    pub encoding: Encoding,
    pub bits_per_pixel: u16,
    pub hotspot: Option<(u16, u16)>,
    pub offset: u64,
    pub length: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Directory {
    pub kind: Kind,
    pub entries: Vec<Entry>,
}

fn malformed(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

/// Inspect all entries before any pixel decoding. This initial profile requires
/// nonoverlapping payloads covering the file, with no private gaps or suffix.
/// Payload order may differ from directory order. Duplicate sizes are retained.
pub fn inspect(bytes: &[u8]) -> Result<Directory> {
    if bytes.len() > ENCODED_BYTES_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "icon exceeds encoded input budget",
        ));
    }
    let header = bytes
        .get(..6)
        .ok_or_else(|| malformed("truncated icon directory"))?;
    if u16_at(header, 0) != 0 {
        return Err(malformed("invalid icon reserved field"));
    }
    let kind = match u16_at(header, 2) {
        1 => Kind::Icon,
        2 => Kind::Cursor,
        _ => return Err(malformed("invalid icon resource type")),
    };
    let count = usize::from(u16_at(header, 4));
    if count == 0 || count > ENTRIES_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "icon entry count outside admitted budget",
        ));
    }
    let directory_end = 6 + count * 16;
    let table = bytes
        .get(6..directory_end)
        .ok_or_else(|| malformed("truncated icon entries"))?;
    let mut entries = Vec::with_capacity(count);
    let mut spans = Vec::with_capacity(count);
    for (index, row) in table.as_chunks::<16>().0.iter().enumerate() {
        if row[3] != 0 {
            return Err(malformed("invalid icon entry reserved field"));
        }
        let width = if row[0] == 0 { 256 } else { u32::from(row[0]) };
        let height = if row[1] == 0 { 256 } else { u32::from(row[1]) };
        let offset = u64::from(u32_at(row, 12));
        let length = u64::from(u32_at(row, 8));
        let end = offset + length;
        if offset < directory_end as u64 || length == 0 || end > bytes.len() as u64 {
            return Err(malformed(
                "icon payload outside file or overlapping directory",
            ));
        }
        let data = &bytes[offset as usize..end as usize];
        let (encoding, bits_per_pixel, colors) = payload::inspect(data, width, height)?;
        if row[2] != 0 && u32::from(row[2]) != colors {
            return Err(malformed("icon palette count contradicts payload"));
        }
        let hotspot = if kind == Kind::Cursor {
            let point = (u16_at(row, 4), u16_at(row, 6));
            if u32::from(point.0) >= width || u32::from(point.1) >= height {
                return Err(malformed("cursor hotspot outside entry"));
            }
            Some(point)
        } else {
            if u16_at(row, 4) > 1 || ![0, bits_per_pixel].contains(&u16_at(row, 6)) {
                return Err(malformed("icon planes or depth contradicts payload"));
            }
            None
        };
        spans.push((offset, end));
        entries.push(Entry {
            index: index as u16,
            width,
            height,
            encoding,
            bits_per_pixel,
            hotspot,
            offset,
            length,
        });
    }
    spans.sort_unstable();
    let mut previous = directory_end as u64;
    for (start, end) in spans {
        if start < previous {
            return Err(malformed("overlapping icon payloads"));
        }
        if start != previous {
            return Err(unsupported(
                "icon payload gaps are outside admitted profile",
            ));
        }
        previous = end;
    }
    if previous != bytes.len() as u64 {
        return Err(unsupported("icon suffix is outside admitted profile"));
    }
    Ok(Directory { kind, entries })
}

/// Selected entry pixels and retained directory metadata. Converting this DTO
/// into a flat document must acknowledge `losses`, including cursor metadata.
#[derive(Debug)]
pub struct Decoded {
    pub entry: Entry,
    pub rgba: Vec<u8>,
    pub losses: Vec<String>,
}
impl Decoded {
    /// Entry selection contributes to document identity, even for duplicate sizes.
    /// This does not acknowledge conversion losses or publish any editor state.
    pub fn document(&self, source_sha256: &str) -> Result<paintcraft_document::Document> {
        paintcraft_document::import_image(
            self.entry.width,
            self.entry.height,
            &self.rgba,
            &format!("{source_sha256}:icon-entry:{}", self.entry.index),
        )
    }
}

/// Decode an explicitly selected zero-based entry, never an implicit largest size.
/// Only the selected payload is copied into the upstream directory reader.
pub fn decode_entry(bytes: &[u8], index: u16, pixels: u64) -> Result<Decoded> {
    let directory = inspect(bytes)?;
    let entry = directory.entries.get(usize::from(index)).ok_or_else(|| {
        Error::new(
            ErrorCode::InputSchema,
            "icon selected entry outside directory",
        )
    })?;
    if u64::from(entry.width) * u64::from(entry.height) > pixels {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "icon selected entry exceeds pixel budget",
        ));
    }
    let payload = &bytes[entry.offset as usize..(entry.offset + entry.length) as usize];
    let mut losses = Vec::new();
    let rgba = if entry.encoding == Encoding::Png {
        let decoded = crate::image::decode_with_pixel_limit(payload, pixels)?;
        losses.extend(decoded.losses);
        if payload[25] == 3 {
            losses.push("icon-palette-expanded".into());
        }
        decoded.rgba
    } else {
        // Upstream ignores the AND mask for 32-bit DIBs. Until the legacy alpha
        // cases are independently qualified, refuse rather than erase pixels.
        if entry.bits_per_pixel == 32 {
            let xor_end = 40 + (entry.width * entry.height * 4) as usize;
            let alpha_present = payload[40..xor_end]
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] != 0);
            let mask_matches = (0..entry.height as usize).all(|row| {
                let stride = entry.width.div_ceil(32) as usize * 4;
                (0..entry.width as usize).all(|column| {
                    payload[xor_end + row * stride + column / 8] & (0x80 >> (column % 8)) == 0
                        || payload[40 + (row * entry.width as usize + column) * 4 + 3] == 0
                })
            });
            if !alpha_present || !mask_matches {
                return Err(unsupported(
                    "legacy or masked 32-bit icon alpha is not qualified",
                ));
            }
        }
        let mut selected = Vec::with_capacity(22 + payload.len());
        selected.extend_from_slice(&bytes[..6]);
        selected[4..6].copy_from_slice(&1u16.to_le_bytes());
        let row = 6 + usize::from(index) * 16;
        selected.extend_from_slice(&bytes[row..row + 16]);
        selected[18..22].copy_from_slice(&22u32.to_le_bytes());
        selected.extend_from_slice(payload);
        let icon = ico::IconDir::read(std::io::Cursor::new(selected))
            .and_then(|directory| directory.entries()[0].decode())
            .map_err(|error| malformed(&format!("icon bitmap decode failed: {error}")))?;
        if entry.bits_per_pixel <= 8 {
            losses.push("icon-palette-expanded".into());
        }
        icon.rgba_data().to_vec()
    };
    if directory.entries.len() > 1 {
        losses.push("icon-other-entries-not-imported".into());
    }
    if entry.hotspot.is_some() {
        losses.push("cursor-hotspot-not-retained-in-flat-document".into());
    }
    Ok(Decoded {
        entry: entry.clone(),
        rgba,
        losses,
    })
}
