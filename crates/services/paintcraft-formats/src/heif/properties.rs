use super::{metadata::Reader, *};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Color<'a> {
    Nclx {
        primaries: u16,
        transfer: u16,
        matrix: u16,
        full_range: bool,
    },
    /// Borrowed original ICC bytes; no validation or conversion is implied.
    Icc { restricted: bool, bytes: &'a [u8] },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Property<'a> {
    SpatialExtent {
        width: u32,
        height: u32,
    },
    /// Container-declared bits, not a substitute for validating coded SPS data.
    PixelInformation {
        bits: &'a [u8],
    },
    Color(Color<'a>),
    HevcConfig {
        length_bytes: u8,
        chroma: u8,
        luma_bits: u8,
        chroma_bits: u8,
        bytes: &'a [u8],
    },
    Auxiliary {
        kind: &'a str,
        subtype: &'a [u8],
    },
    Rotation {
        quarter_turns_ccw: u8,
    },
    Mirror {
        axis: u8,
    },
    CleanAperture {
        width: (u32, u32),
        height: (u32, u32),
        horizontal_offset: (i32, u32),
        vertical_offset: (i32, u32),
    },
    Unknown {
        kind: [u8; 4],
        bytes: &'a [u8],
    },
}
fn parse<'a>(
    info: &BoxInfo<'a>,
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Property<'a>> {
    let b = info.payload;
    let mut r = Reader::new(b);
    match &info.kind {
        b"ispe" => {
            let (v, f, body) = full(b)?;
            if v != 0 || f != 0 {
                return Err(unsupported("HEIF spatial extent version/flags"));
            }
            r = Reader::new(body);
            let width = r.u32()?;
            let height = r.u32()?;
            r.end()?;
            if width == 0
                || height == 0
                || width > 16384
                || height > 16384
                || u64::from(width) * u64::from(height) > pixels
            {
                return Err(resource("HEIF declared dimensions exceed allowance"));
            }
            Ok(Property::SpatialExtent { width, height })
        }
        b"pixi" => {
            let (v, f, body) = full(b)?;
            if v != 0 || f != 0 {
                return Err(unsupported("HEIF pixel info version/flags"));
            }
            r = Reader::new(body);
            let n = usize::from(r.u8()?);
            if !matches!(n, 1 | 3 | 4) {
                return Err(unsupported("HEIF pixel channel count"));
            }
            let bits = r.take(n)?;
            r.end()?;
            if bits.iter().any(|b| !matches!(b, 8 | 10 | 12)) {
                return Err(unsupported("HEIF declared pixel depth"));
            }
            Ok(Property::PixelInformation { bits })
        }
        b"colr" => {
            let kind = r.take(4)?;
            let color = match kind {
                b"nclx" => {
                    let primaries = r.u16()?;
                    let transfer = r.u16()?;
                    let matrix = r.u16()?;
                    let range = r.u8()?;
                    if range & 0x7f != 0 {
                        return Err(invalid("HEIF nclx reserved bits"));
                    }
                    r.end()?;
                    Color::Nclx {
                        primaries,
                        transfer,
                        matrix,
                        full_range: range & 0x80 != 0,
                    }
                }
                b"prof" | b"rICC" => {
                    let bytes = r.take(b.len() - 4)?;
                    if bytes.is_empty() {
                        return Err(invalid("empty HEIF ICC profile"));
                    }
                    Color::Icc {
                        restricted: kind == b"rICC",
                        bytes,
                    }
                }
                _ => return Err(unsupported("HEIF color profile kind")),
            };
            Ok(Property::Color(color))
        }
        b"auxC" => {
            let (v, f, body) = full(b)?;
            if v != 0 || f != 0 {
                return Err(unsupported("HEIF auxiliary version/flags"));
            }
            r = Reader::new(body);
            let kind = r.string()?;
            let subtype = r.take(body.len() - r.pos)?;
            Ok(Property::Auxiliary { kind, subtype })
        }
        b"irot" | b"imir" => {
            let value = r.u8()?;
            r.end()?;
            let mask = if info.kind == *b"irot" { 3 } else { 1 };
            if value & !mask != 0 {
                return Err(invalid("HEIF transform reserved bits"));
            }
            Ok(if info.kind == *b"irot" {
                Property::Rotation {
                    quarter_turns_ccw: value,
                }
            } else {
                Property::Mirror { axis: value }
            })
        }
        b"clap" => {
            let mut values = [0; 8];
            for v in &mut values {
                *v = r.u32()?;
            }
            r.end()?;
            if [1, 3, 5, 7].iter().any(|i| values[*i] == 0) || values[0] == 0 || values[2] == 0 {
                return Err(invalid("HEIF aperture zero extent/denominator"));
            }
            Ok(Property::CleanAperture {
                width: (values[0], values[1]),
                height: (values[2], values[3]),
                horizontal_offset: (values[4] as i32, values[5]),
                vertical_offset: (values[6] as i32, values[7]),
            })
        }
        b"hvcC" => {
            let header = r.take(23)?;
            if header[0] != 1 {
                return Err(unsupported("HEIF HEVC configuration version"));
            }
            // Check the array framing without decoding any HEVC parameter set.
            let mut nals = 0usize;
            for _ in 0..header[22] {
                check()?;
                let array = r.u8()?;
                if array & 0x40 != 0 {
                    return Err(invalid("HEIF HEVC array reserved bit"));
                }
                let count = usize::from(r.u16()?);
                nals = nals
                    .checked_add(count)
                    .ok_or_else(|| resource("HEIF HEVC NAL count overflow"))?;
                if nals > 1024 {
                    return Err(resource("HEIF HEVC NAL arrays exceed allowance"));
                }
                for _ in 0..count {
                    check()?;
                    let n = usize::from(r.u16()?);
                    let nal = r.take(n)?;
                    if n < 2
                        || nal[0] & 0x80 != 0
                        || (nal[0] >> 1) & 0x3f != array & 0x3f
                        || nal[1] & 7 == 0
                    {
                        return Err(invalid("HEIF HEVC configuration NAL header"));
                    }
                }
            }
            r.end()?;
            Ok(Property::HevcConfig {
                length_bytes: (header[21] & 3) + 1,
                chroma: header[16] & 3,
                luma_bits: 8 + (header[17] & 7),
                chroma_bits: 8 + (header[18] & 7),
                bytes: b,
            })
        }
        _ => Ok(Property::Unknown {
            kind: info.kind,
            bytes: b,
        }),
    }
}
pub(super) fn associations<'a>(
    tree: &BoxTree<'a>,
    iprp: usize,
    items: &mut [Item<'a>],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    crate::item_graph::properties::associations(
        tree,
        iprp,
        items,
        &|info| parse(info, pixels, check),
        &|property| !matches!(property, Property::Unknown { .. }),
        check,
    )?;
    let mut aggregate = 0u64;
    for item in items {
        check()?;
        if matches!(&item.kind, b"hvc1" | b"grid" | b"iden") {
            let mut extents = item.properties.iter().filter_map(|p| {
                if let Property::SpatialExtent { width, height } = p.property {
                    Some((width, height))
                } else {
                    None
                }
            });
            let (width, height) = extents
                .next()
                .ok_or_else(|| invalid("HEIF image lacks declared spatial extent"))?;
            if extents.next().is_some() {
                return Err(invalid("HEIF image requires one declared spatial extent"));
            }
            aggregate = aggregate
                .checked_add(u64::from(width) * u64::from(height))
                .ok_or_else(|| resource("HEIF declared pixel aggregate overflow"))?;
            if aggregate > pixels.saturating_mul(4) {
                return Err(resource("HEIF declared pixel aggregate exceeds allowance"));
            }
            if item.kind == *b"hvc1"
                && item
                    .properties
                    .iter()
                    .filter(|p| matches!(p.property, Property::HevcConfig { .. }))
                    .count()
                    != 1
            {
                return Err(invalid("HEIF coded image requires one configuration"));
            }
        }
    }
    Ok(())
}
