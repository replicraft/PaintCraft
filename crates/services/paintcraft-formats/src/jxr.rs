//! Bounded JPEG XR header inspection, not an enabled image decoder.
//!
//! The selected experimental codec executes inside the private isolated native
//! helper, with independent developer qualification. There is no inline or public
//! image reader. Header admission does not validate entropy data or bound upstream
//! allocation by itself.
mod import;
pub use import::{ImportAssessment, KNOWN_IMPORT_LOSSES, Resolution, assess_import};
use paintcraft_core::types::{Error, ErrorCode, Result};

pub const INPUT_BYTES_MAX: usize = 64 << 20;
pub const METADATA_BYTES_MAX: usize = 256 << 10;
pub const PIXELS_MAX: u64 = 4_194_304;
const ENTRIES_MAX: usize = 128;
/// ITU-T T.832(2012), AnnexC.2: default only after explicit/other color context
/// is absent. The inspected profile refuses ICC, COLOR_SPACE and PTM_COLOR_INFO.
pub const COLOR_POLICY: &str = "t832-uint-rgb-absent-context-default-srgb";
const RGB8_GUID: [u8; 16] = [
    0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0d,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inspection {
    pub width: u32,
    pub height: u32,
    /// Single RGB8 codestream, with no alpha, orientation or color conversion.
    pub image_offset: usize,
    pub image_bytes: usize,
    pub container_version: u8,
    /// Exact f32 source bits; no density conversion is performed.
    pub resolution_x_bits: Option<u32>,
    pub resolution_y_bits: Option<u32>,
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn take(bytes: &[u8], start: usize, len: usize) -> Result<&[u8]> {
    let end = start
        .checked_add(len)
        .ok_or_else(|| invalid("JPEG XR extent overflows"))?;
    bytes
        .get(start..end)
        .ok_or_else(|| invalid("truncated JPEG XR header or extent"))
}
fn word(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        take(bytes, offset, 2)?.try_into().unwrap(),
    ))
}
fn dword(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        take(bytes, offset, 4)?.try_into().unwrap(),
    ))
}

pub fn sniff(bytes: &[u8]) -> bool {
    bytes
        .get(..4)
        .is_some_and(|magic| magic == b"II\xbc\0" || magic == b"II\xbc\x01")
}
/// Inspect a deliberately narrow RGB8 baseline without invoking the codec,
/// touching the filesystem, allocating decoded pixels or following metadata.
/// Unsupported GUIDs, ICC/EXIF/unknown fields, alpha, multiple frames, transforms,
/// tiled/frequency/windowed streams and inconsistent dimensions fail closed.
pub fn inspect_checked(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Inspection> {
    check()?;
    if bytes.len() > INPUT_BYTES_MAX {
        return Err(resource("JPEG XR exceeds encoded input limit"));
    }
    if !sniff(bytes) {
        return Err(invalid("invalid JPEG XR container signature/version"));
    }
    let ifd = dword(bytes, 4)? as usize;
    if !(8..=METADATA_BYTES_MAX).contains(&ifd) {
        return Err(resource("JPEG XR directory exceeds metadata limit"));
    }
    let count = usize::from(word(bytes, ifd)?);
    if count == 0 || count > ENTRIES_MAX {
        return Err(resource("JPEG XR directory entry budget exceeded"));
    }
    let table_end = ifd
        .checked_add(2 + count * 12 + 4)
        .ok_or_else(|| invalid("JPEG XR directory overflow"))?;
    if table_end > METADATA_BYTES_MAX {
        return Err(resource("JPEG XR directory exceeds metadata limit"));
    }
    take(bytes, ifd, table_end - ifd)?;
    if dword(bytes, table_end - 4)? != 0 {
        return Err(unsupported(
            "multiple JPEG XR directories are not qualified",
        ));
    }
    let mut seen = [0u16; ENTRIES_MAX];
    let (mut width, mut height, mut offset, mut length, mut guid) = (None, None, None, None, None);
    let (mut resolution_x_bits, mut resolution_y_bits) = (None, None);
    for i in 0..count {
        check()?;
        let pos = ifd + 2 + i * 12;
        let tag = word(bytes, pos)?;
        let ty = word(bytes, pos + 2)?;
        let n = dword(bytes, pos + 4)?;
        let value = dword(bytes, pos + 8)?;
        if seen[..i].contains(&tag) {
            return Err(invalid("duplicate JPEG XR directory field"));
        }
        seen[i] = tag;
        match tag {
            0xbc01 => {
                if ty != 1 || n != 16 {
                    return Err(invalid("invalid JPEG XR pixel-format field"));
                }
                let start = value as usize;
                let end = start
                    .checked_add(16)
                    .ok_or_else(|| invalid("JPEG XR GUID overflows"))?;
                if start < 8 || end > ifd || end > METADATA_BYTES_MAX {
                    return Err(invalid("JPEG XR GUID overlaps directory/header"));
                }
                if take(bytes, start, 16)? != RGB8_GUID {
                    return Err(unsupported(
                        "only straight RGB8 JPEG XR inspection is qualified",
                    ));
                }
                guid = Some(());
            }
            0xbc80 | 0xbc81 | 0xbcc0 | 0xbcc1 | 0xbc02 => {
                if ty != 4 || n != 1 {
                    return Err(invalid("invalid scalar JPEG XR field"));
                }
                match tag {
                    0xbc80 => width = Some(value),
                    0xbc81 => height = Some(value),
                    0xbcc0 => offset = Some(value as usize),
                    0xbcc1 => length = Some(value as usize),
                    _ if value != 0 => {
                        return Err(unsupported("JPEG XR orientation is not qualified"));
                    }
                    _ => {}
                }
            }
            0xbc82 | 0xbc83 => {
                if ty != 11 || n != 1 {
                    return Err(invalid("invalid JPEG XR resolution field"));
                }
                if tag == 0xbc82 {
                    resolution_x_bits = Some(value);
                } else {
                    resolution_y_bits = Some(value);
                }
                let value = f32::from_bits(value);
                if !value.is_finite() || value < 0.0 {
                    return Err(invalid("invalid JPEG XR resolution"));
                }
            }
            _ => {
                return Err(unsupported(
                    "JPEG XR metadata, alpha or unknown directory field is not qualified",
                ));
            }
        }
    }
    guid.ok_or_else(|| invalid("missing JPEG XR pixel format"))?;
    let width = width.ok_or_else(|| invalid("missing JPEG XR width"))?;
    let height = height.ok_or_else(|| invalid("missing JPEG XR height"))?;
    if width == 0
        || height == 0
        || width > 65536
        || height > 65536
        || u64::from(width) * u64::from(height) > pixels.min(PIXELS_MAX)
    {
        return Err(resource("JPEG XR exceeds pixel or dimension budget"));
    }
    let image_offset = offset.ok_or_else(|| invalid("missing JPEG XR codestream offset"))?;
    let image_bytes = length.ok_or_else(|| invalid("missing JPEG XR codestream length"))?;
    if image_offset < table_end || image_offset > METADATA_BYTES_MAX {
        return Err(invalid("JPEG XR codestream overlaps metadata"));
    }
    let stream = take(bytes, image_offset, image_bytes)?;
    if image_offset.checked_add(image_bytes) != Some(bytes.len()) {
        return Err(unsupported("JPEG XR trailing payloads are not qualified"));
    }
    if take(stream, 0, 8)? != b"WMPHOTO\0" {
        return Err(invalid("invalid JPEG XR codestream signature"));
    }
    // Admission subset: v1.1, spatial, no tiles/orientation/index/overlap,
    // abbreviated sizes, no window/alpha, RGB eight-bit source.
    if take(stream, 8, 4)? != [0x11, 0x00, 0xc0, 0x71] {
        return Err(unsupported("JPEG XR codestream mode is not qualified"));
    }
    let sw = u32::from(u16::from_be_bytes(take(stream, 12, 2)?.try_into().unwrap())) + 1;
    let sh = u32::from(u16::from_be_bytes(take(stream, 14, 2)?.try_into().unwrap())) + 1;
    if (sw, sh) != (width, height) {
        return Err(invalid(
            "JPEG XR codestream dimensions disagree with container",
        ));
    }
    if take(stream, 16, 2)? != [0x60, 0x00] {
        return Err(unsupported(
            "JPEG XR image-plane color model is not qualified",
        ));
    }
    check()?;
    Ok(Inspection {
        width,
        height,
        image_offset,
        image_bytes,
        container_version: bytes[3],
        resolution_x_bits,
        resolution_y_bits,
    })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod qualification;
#[cfg(test)]
mod tests;
