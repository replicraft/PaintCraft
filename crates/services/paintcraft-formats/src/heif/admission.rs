//! Header admission for isolated HEIF worker profiles. Native gray is wired;
//! explicit color remains internally qualified until its consumer gates pass.
//! Decoder and parameter parsing require the process allocator/deadline boundary.
use super::*;
mod color;
pub use color::{ColorAdmitted, admit_srgb8};
use heic_decoder::hevc::{nal::parse_nal_header, params::parse_sps};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encoding {
    pub primaries: u16,
    pub transfer: u16,
    pub matrix: u16,
    pub full_range: bool,
}
#[derive(Debug)]
pub struct Admitted<'a> {
    source: &'a [u8],
    width: u32,
    height: u32,
    alpha: bool,
    encoding: Encoding,
}
impl Admitted<'_> {
    pub fn source(&self) -> &[u8] {
        self.source
    }
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn alpha(&self) -> bool {
        self.alpha
    }
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }
}
#[derive(Debug, Clone, Copy)]
struct Coded {
    id: u32,
    size: (u32, u32),
    raw_pixels: u64,
    encoding: Option<Encoding>,
    chroma: u32,
    chroma_location: Option<u32>,
}
fn map(error: heic_decoder::HeifError) -> Error {
    match error {
        heic_decoder::HeifError::LimitExceeded(_) => resource("HEVC parameter resource allowance"),
        heic_decoder::HeifError::Unsupported(_) => unsupported("HEVC parameter subtype"),
        _ => invalid("invalid HEVC parameter set"),
    }
}
fn spatial(item: &Item<'_>) -> Result<(u32, u32)> {
    item.properties
        .iter()
        .find_map(|a| {
            if let Property::SpatialExtent { width, height } = a.property {
                Some((width, height))
            } else {
                None
            }
        })
        .ok_or_else(|| invalid("HEIF image has no spatial extent"))
}
fn profile(item: &Item<'_>, fallback: Option<Encoding>) -> Result<Encoding> {
    let mut color = None;
    for property in &item.properties {
        match property.property {
            Property::Color(Color::Icc { .. }) => {
                return Err(unsupported("HEIF ICC interpretation is not qualified"));
            }
            Property::Color(Color::Nclx {
                primaries,
                transfer,
                matrix,
                full_range,
            }) => {
                if color.is_some() {
                    return Err(unsupported("multiple HEIF NCLX profiles"));
                }
                color = Some(Encoding {
                    primaries,
                    transfer,
                    matrix,
                    full_range,
                });
            }
            Property::Rotation {
                quarter_turns_ccw: 0,
            } => {}
            Property::Rotation { .. }
            | Property::Mirror { .. }
            | Property::CleanAperture { .. } => {
                return Err(unsupported(
                    "HEIF display transformations are not yet qualified",
                ));
            }
            _ => {}
        }
    }
    if color
        .zip(fallback)
        .is_some_and(|(declared, coded)| declared != coded)
    {
        return Err(unsupported(
            "HEIF container and coded color declarations disagree",
        ));
    }
    let color = color
        .or(fallback)
        .ok_or_else(|| unsupported("unlabelled HEIF gray range is not qualified"))?;
    if !color.full_range
        || !matches!(color.primaries, 1 | 2)
        || !matches!(color.transfer, 2 | 13)
        || !matches!(color.matrix, 0..=2)
    {
        return Err(unsupported(
            "only full-range native gray8 encoding is qualified",
        ));
    }
    Ok(color)
}
fn cropped_size(
    width: u32,
    height: u32,
    [left, right, top, bottom]: [u32; 4],
) -> Result<(u32, u32)> {
    let width = width
        .checked_sub(left)
        .and_then(|n| n.checked_sub(right))
        .filter(|n| *n != 0)
        .ok_or_else(|| invalid("HEVC horizontal conformance crop exceeds coded width"))?;
    let height = height
        .checked_sub(top)
        .and_then(|n| n.checked_sub(bottom))
        .filter(|n| *n != 0)
        .ok_or_else(|| invalid("HEVC vertical conformance crop exceeds coded height"))?;
    Ok((width, height))
}
fn coded(item: &Item<'_>, pixels: u64, check: &dyn Fn() -> Result<()>) -> Result<Coded> {
    coded_profile(item, pixels, check, false)
}
fn coded_profile(
    item: &Item<'_>,
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
    color: bool,
) -> Result<Coded> {
    let (luma, chroma, format, bytes) = item
        .properties
        .iter()
        .find_map(|a| {
            if let Property::HevcConfig {
                luma_bits,
                chroma_bits,
                chroma,
                bytes,
                ..
            } = a.property
            {
                Some((luma_bits, chroma_bits, chroma, bytes))
            } else {
                None
            }
        })
        .ok_or_else(|| invalid("missing HEVC configuration"))?;
    if luma != 8
        || chroma != 8
        || if color {
            !matches!(format, 1 | 3)
        } else {
            format != 0
        }
    {
        return Err(unsupported(if color {
            "isolated HEIF color requires 8-bit 4:4:4 or 4:2:0"
        } else {
            "isolated HEIF gray requires native monochrome8"
        }));
    }
    let mut r = metadata::Reader::new(bytes);
    let header = r.take(23)?;
    let mut found = None;
    let mut vps = false;
    let mut pps = false;
    let mut raw_pixels = 0u64;
    let mut sampling = None;
    for _ in 0..header[22] {
        check()?;
        let kind = r.u8()? & 63;
        let count = r.u16()?;
        for _ in 0..count {
            check()?;
            let n = usize::from(r.u16()?);
            let nal = r.take(n)?;
            let (h, body) = parse_nal_header(nal).map_err(map)?;
            if h.nal_unit_type != kind || h.nuh_layer_id != 0 || h.nuh_temporal_id_plus1 != 1 {
                return Err(unsupported("HEIF multilayer/temporal parameters"));
            }
            match kind {
                32 => vps = true,
                34 => pps = true,
                33 => {
                    // Input is bounded by the u16 NAL size and whole metadata
                    // allowance. This audited parser can still OOM/panic, so this
                    // function belongs ONLY inside the isolated codec process.
                    let rbsp = heic_decoder::bitreader::strip_emulation_prevention(body);
                    let s = parse_sps(&rbsp).map_err(map)?;
                    check()?;
                    if s.bit_depth_luma != 8
                        || s.bit_depth_chroma != 8
                        || s.chroma_format_idc != u32::from(format)
                    {
                        return Err(unsupported(
                            "coded SPS precision/chroma differs from admitted configuration",
                        ));
                    }
                    let raw = u64::from(s.pic_width_in_luma_samples)
                        * u64::from(s.pic_height_in_luma_samples);
                    if s.pic_width_in_luma_samples > 16384
                        || s.pic_height_in_luma_samples > 16384
                        || raw > pixels
                    {
                        return Err(resource("coded SPS dimensions exceed allowance"));
                    }
                    raw_pixels = raw_pixels.max(raw);
                    let size = cropped_size(
                        s.pic_width_in_luma_samples,
                        s.pic_height_in_luma_samples,
                        s.conformance_window,
                    )?;
                    if color {
                        color::geometry(size, u32::from(format))?;
                    }
                    if spatial(item)? != size {
                        return Err(invalid("ispe disagrees with coded SPS crop"));
                    }
                    for property in &item.properties {
                        if let Property::PixelInformation { bits } = property.property
                            && bits != if color { &[8, 8, 8][..] } else { &[8][..] }
                        {
                            return Err(invalid("pixi disagrees with coded 8-bit SPS"));
                        }
                    }
                    let encoding = s.color_info.map(|c| Encoding {
                        primaries: c.color_primaries,
                        transfer: c.transfer_characteristics,
                        matrix: c.matrix_coefficients,
                        full_range: c.full_range,
                    });
                    let encoding = if color {
                        color::profile(item, encoding)?
                    } else {
                        profile(item, encoding)?
                    };
                    if color && encoding.matrix == 0 && format != 3 {
                        return Err(unsupported("HEIF identity matrix requires 4:4:4"));
                    }
                    if color && (s.conformance_window[0] != 0 || s.conformance_window[2] != 0) {
                        return Err(unsupported(
                            "HEIF shifted conformance crop is not qualified",
                        ));
                    }
                    if color && format == 1 && s.chroma_location.unwrap_or(0) != 0 {
                        return Err(unsupported("HEIF 4:2:0 chroma siting is not qualified"));
                    }
                    let current_sampling = (s.chroma_format_idc, s.chroma_location);
                    if sampling.is_some_and(|previous| previous != current_sampling) {
                        return Err(unsupported(
                            "multiple incompatible SPS sampling declarations",
                        ));
                    }
                    sampling = Some(current_sampling);
                    if found.is_some_and(|prior: (u32, u32, Encoding)| {
                        prior != (size.0, size.1, encoding)
                    }) {
                        return Err(unsupported("multiple incompatible SPS declarations"));
                    }
                    found = Some((size.0, size.1, encoding));
                }
                _ => {
                    return Err(unsupported(
                        "HEIF configuration contains unqualified parameter kinds",
                    ));
                }
            }
        }
    }
    r.end()?;
    if !vps || !pps {
        return Err(invalid("HEIF configuration lacks VPS/PPS"));
    }
    let (w, h, encoding) = found.ok_or_else(|| invalid("HEIF configuration lacks SPS"))?;
    validate_payload(item, header[21] & 3, check)?;
    Ok(Coded {
        id: item.id,
        size: (w, h),
        raw_pixels,
        encoding: Some(encoding),
        chroma: u32::from(format),
        chroma_location: sampling.and_then(|value| value.1),
    })
}
struct Cursor<'a, 'b> {
    parts: &'b [&'a [u8]],
    part: usize,
    offset: usize,
    left: usize,
}
impl Cursor<'_, '_> {
    fn byte(&mut self) -> Result<u8> {
        while self.part < self.parts.len() && self.offset == self.parts[self.part].len() {
            self.part += 1;
            self.offset = 0;
        }
        let b = *self
            .parts
            .get(self.part)
            .and_then(|p| p.get(self.offset))
            .ok_or_else(|| invalid("truncated HEIF item NAL"))?;
        self.offset += 1;
        self.left -= 1;
        Ok(b)
    }
    fn skip(&mut self, mut n: usize) -> Result<()> {
        if n > self.left {
            return Err(invalid("HEIF NAL exceeds item payload"));
        }
        while n > 0 {
            while self.part < self.parts.len() && self.offset == self.parts[self.part].len() {
                self.part += 1;
                self.offset = 0;
            }
            let take = n.min(self.parts[self.part].len() - self.offset);
            self.offset += take;
            self.left -= take;
            n -= take;
        }
        Ok(())
    }
}
fn validate_payload(
    item: &Item<'_>,
    length_minus_one: u8,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let payload = item
        .payload
        .as_ref()
        .ok_or_else(|| invalid("HEIF coded item lacks admitted storage"))?;
    let mut cursor = Cursor {
        parts: &payload.parts,
        part: 0,
        offset: 0,
        left: payload.byte_len,
    };
    let mut slices = 0;
    while cursor.left > 0 {
        check()?;
        let mut n = 0u32;
        for _ in 0..=length_minus_one {
            n = n
                .checked_shl(8)
                .ok_or_else(|| invalid("HEIF NAL length overflow"))?
                | u32::from(cursor.byte()?);
        }
        let n = usize::try_from(n).map_err(|_| resource("HEIF NAL exceeds address space"))?;
        if n < 2 || n > cursor.left {
            return Err(invalid("HEIF item NAL length"));
        }
        let header = [cursor.byte()?, cursor.byte()?];
        let (h, _) = parse_nal_header(&header).map_err(map)?;
        // First qualified still-frame fixture: a single base-layer IDR_N_LP.
        // Broader slice/SEI/sub-layer combinations require their own oracles.
        if h.nal_unit_type != 20 || h.nuh_layer_id != 0 || h.nuh_temporal_id_plus1 != 1 {
            return Err(unsupported("HEIF item is not a qualified single still IDR"));
        }
        slices += 1;
        if slices > 1 {
            return Err(unsupported("multiple HEIF slices are not qualified"));
        }
        cursor.skip(n - 2)?;
    }
    if slices != 1 {
        return Err(invalid("HEIF image has no qualified slice"));
    }
    Ok(())
}
fn image_info(
    value: &Inspection<'_>,
    id: u32,
    coded: &[Coded],
    check: &dyn Fn() -> Result<()>,
) -> Result<Coded> {
    let mut current = id;
    for _ in 0..=16 {
        check()?;
        let item = value
            .item(current)
            .ok_or_else(|| invalid("HEIF image reference disappeared"))?;
        match &item.kind {
            b"hvc1" => {
                return coded
                    .iter()
                    .find(|c| c.id == current)
                    .copied()
                    .ok_or_else(|| invalid("HEIF coded image was not admitted"));
            }
            b"grid" => {
                let data = item
                    .payload
                    .as_ref()
                    .and_then(|p| p.grid.as_ref())
                    .ok_or_else(|| invalid("HEIF grid descriptor not admitted"))?;
                let ids = value
                    .references(*b"dimg", current)
                    .ok_or_else(|| invalid("HEIF grid has no tiles"))?;
                let mut first = None;
                for id in ids {
                    let tile = coded
                        .iter()
                        .find(|c| c.id == *id)
                        .copied()
                        .ok_or_else(|| invalid("HEIF grid tile not admitted"))?;
                    if first.is_some_and(|prior: Coded| {
                        prior.size != tile.size || prior.encoding != tile.encoding
                    }) {
                        return Err(unsupported("HEIF grid tile size/encoding disagreement"));
                    }
                    first = Some(tile);
                }
                let encoding = profile(item, first.and_then(|tile| tile.encoding))?;
                return Ok(Coded {
                    id: current,
                    size: (data.width, data.height),
                    raw_pixels: 0,
                    encoding: Some(encoding),
                    chroma: first.map_or(0, |value| value.chroma),
                    chroma_location: first.and_then(|value| value.chroma_location),
                });
            }
            b"iden" => {
                let Some([source]) = value.references(*b"dimg", current) else {
                    return Err(invalid("HEIF identity has no unique source"));
                };
                current = *source;
            }
            _ => return Err(unsupported("HEIF derived image type")),
        }
    }
    Err(resource("HEIF identity chain exceeds allowance"))
}
/// Do not invoke in GUI/CLI processes: the selected parser has no allocator or
/// cancellation hooks. The real codec-worker provides those execution bounds.
pub fn admit_gray8<'a>(
    bytes: &'a [u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Admitted<'a>> {
    let pixels = pixels.min(PIXELS_MAX);
    let value = inspect_checked(bytes, pixels, check)?;
    let mut coded_images = Vec::new();
    let mut aggregate = 0u64;
    for item in &value.items {
        check()?;
        for property in &item.properties {
            match property.property {
                Property::Color(Color::Icc { .. })
                | Property::Mirror { .. }
                | Property::CleanAperture { .. }
                | Property::Rotation {
                    quarter_turns_ccw: 1..,
                } => {
                    return Err(unsupported(
                        "HEIF profile requires no ICC/display transformations",
                    ));
                }
                _ => {}
            }
        }
        if item.kind == *b"hvc1" {
            let info = coded(item, pixels, check)?;
            aggregate = aggregate
                .checked_add(info.raw_pixels)
                .ok_or_else(|| resource("HEIF coded pixel aggregate overflow"))?;
            if aggregate > pixels.saturating_mul(4) {
                return Err(resource("HEIF coded pixel aggregate exceeds allowance"));
            }
            push(&mut coded_images, info, ITEMS_MAX)?;
        }
    }
    let info = image_info(&value, value.primary_id, &coded_images, check)?;
    if spatial(
        value
            .primary()
            .ok_or_else(|| invalid("HEIF primary missing"))?,
    )? != info.size
    {
        return Err(invalid("HEIF identity extent disagrees with source"));
    }
    let alpha = if let Some(alpha) = value.alpha() {
        let alpha = image_info(&value, alpha.id, &coded_images, check)?;
        if alpha.size != info.size {
            return Err(unsupported("HEIF alpha extent disagrees with primary"));
        }
        true
    } else {
        false
    };
    if value.premultiplied_alpha() {
        return Err(unsupported(
            "premultiplied HEIF gray has not been qualified",
        ));
    }
    let encoding = info
        .encoding
        .ok_or_else(|| unsupported("HEIF native encoding unavailable"))?;
    check()?;
    Ok(Admitted {
        source: bytes,
        width: info.size.0,
        height: info.size.1,
        alpha,
        encoding,
    })
}

#[cfg(test)]
mod tests;
