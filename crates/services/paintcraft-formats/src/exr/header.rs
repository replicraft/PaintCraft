use super::*;
use crate::precision::{ColorEvidence, ColorMetadata, LinearSpace, SourceAlpha};
#[derive(Clone, Debug)]
pub struct Attribute<'a> {
    pub name: &'a str,
    pub kind: &'a str,
    pub value: &'a [u8],
}
#[derive(Debug)]
pub struct Header<'a> {
    pub width: u32,
    pub height: u32,
    pub origin: [i32; 2],
    pub channels: Vec<Channel<'a>>,
    pub compression: u8,
    pub attributes: Vec<Attribute<'a>>,
    pub chromaticities: Option<[f32; 8]>,
    pub color_interop_id: Option<&'a str>,
}
impl Header<'_> {
    pub fn source_alpha(&self) -> SourceAlpha {
        if self.channels.iter().any(|c| c.name == "A") {
            SourceAlpha::Premultiplied
        } else {
            SourceAlpha::Opaque
        }
    }
    pub fn color_metadata(&self) -> ColorMetadata {
        let chromaticity = self.chromaticities.and_then(|actual| {
            [
                (
                    [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290],
                    LinearSpace::Rec709D65,
                ),
                (
                    [0.64, 0.33, 0.29, 0.60, 0.15, 0.06, 1.0 / 3.0, 1.0 / 3.0],
                    LinearSpace::RadianceEqualEnergy,
                ),
            ]
            .into_iter()
            .find(|(known, _)| actual == *known)
            .map(|(_, space)| space)
        });
        match self.color_interop_id {
            Some("lin_rec709_scene")
                if self.chromaticities.is_none()
                    || chromaticity == Some(LinearSpace::Rec709D65) =>
            {
                ColorMetadata::Declared {
                    space: LinearSpace::Rec709D65,
                    evidence: ColorEvidence::InteropId,
                }
            }
            Some(_) => ColorMetadata::Unspecified,
            None => chromaticity
                .map(|space| ColorMetadata::Declared {
                    space,
                    evidence: ColorEvidence::Chromaticities,
                })
                .unwrap_or(ColorMetadata::Unspecified),
        }
    }
}
fn attr<'a>(attributes: &[Attribute<'a>], name: &str, kind: &str, len: usize) -> Result<&'a [u8]> {
    let a = attributes
        .iter()
        .find(|a| a.name == name)
        .ok_or_else(|| invalid("EXR required attribute missing"))?;
    if a.kind != kind || a.value.len() != len {
        return Err(invalid("EXR attribute type/length disagrees"));
    }
    Ok(a.value)
}
fn window(data: &[u8]) -> [i32; 4] {
    std::array::from_fn(|i| i32::from_le_bytes(data[i * 4..i * 4 + 4].try_into().unwrap()))
}
pub(super) fn parse<'a>(
    bytes: &mut Bytes<'a>,
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Header<'a>> {
    let mut attributes: Vec<Attribute<'a>> = Vec::new();
    loop {
        check()?;
        if bytes.at > HEADER_BYTES_MAX {
            return Err(resource("EXR header exceeds byte allowance"));
        }
        let name = bytes.text()?;
        if bytes.at > HEADER_BYTES_MAX {
            return Err(resource("EXR header terminator exceeds byte allowance"));
        }
        if name.is_empty() {
            break;
        }
        if attributes.len() == ATTRIBUTES_MAX {
            return Err(resource("EXR attribute count exceeds allowance"));
        }
        if attributes.iter().any(|a| a.name == name) {
            return Err(invalid("Duplicate EXR attribute"));
        }
        let kind = bytes.text()?;
        if kind.is_empty() {
            return Err(invalid("EXR empty attribute type"));
        }
        let size = bytes.i32()?;
        if size < 0 {
            return Err(invalid("EXR negative attribute extent"));
        }
        let end = bytes
            .at
            .checked_add(size as usize)
            .ok_or_else(|| resource("EXR attribute extent overflow"))?;
        if end > HEADER_BYTES_MAX {
            return Err(resource("EXR attribute/header exceeds allowance"));
        }
        let value = bytes.take(size as usize)?;
        attributes
            .try_reserve(1)
            .map_err(|_| resource("EXR attribute inventory allocation"))?;
        attributes.push(Attribute { name, kind, value });
    }
    let channels = attributes
        .iter()
        .find(|a| a.name == "channels")
        .ok_or_else(|| invalid("EXR channels missing"))?;
    if channels.kind != "chlist" {
        return Err(invalid("EXR channels type mismatch"));
    }
    let channels = channels::parse(channels.value)?;
    let compression = attr(&attributes, "compression", "compression", 1)?[0];
    if compression > 3 {
        return Err(unsupported(
            "EXR compression not qualified; initial lossless NONE/RLE/ZIPS/ZIP only",
        ));
    }
    if attr(&attributes, "lineOrder", "lineOrder", 1)? != [0] {
        return Err(unsupported(
            "EXR initial profile requires increasing scanlines",
        ));
    }
    let data = window(attr(&attributes, "dataWindow", "box2i", 16)?);
    if data != window(attr(&attributes, "displayWindow", "box2i", 16)?) {
        return Err(unsupported(
            "EXR differing data/display windows require explicit canvas policy",
        ));
    }
    let width = i64::from(data[2]) - i64::from(data[0]) + 1;
    let height = i64::from(data[3]) - i64::from(data[1]) + 1;
    if width <= 0
        || height <= 0
        || width > 65536
        || height > 65536
        || (width as u64) * (height as u64) > pixels.min(PIXELS_MAX)
    {
        return Err(resource("EXR windows exceed pixel/side allowance"));
    }
    let aspect = f32::from_le_bytes(
        attr(&attributes, "pixelAspectRatio", "float", 4)?
            .try_into()
            .unwrap(),
    );
    let center = attr(&attributes, "screenWindowCenter", "v2f", 8)?;
    let screen = f32::from_le_bytes(
        attr(&attributes, "screenWindowWidth", "float", 4)?
            .try_into()
            .unwrap(),
    );
    if !aspect.is_finite()
        || aspect <= 0.0
        || !screen.is_finite()
        || screen <= 0.0
        || center
            .as_chunks::<4>()
            .0
            .iter()
            .any(|v| !f32::from_le_bytes(*v).is_finite())
    {
        return Err(unsupported("EXR invalid screen/pixel geometry metadata"));
    }
    let mut chromaticities = None;
    let mut color_interop_id = None;
    for a in &attributes {
        match a.name {
            "chromaticities" => {
                let raw = attr(&attributes, a.name, "chromaticities", 32)?;
                let coords = std::array::from_fn(|i| {
                    f32::from_le_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap())
                });
                if coords.iter().any(|v| !v.is_finite()) {
                    return Err(unsupported("EXR nonfinite chromaticities"));
                }
                chromaticities = Some(coords);
            }
            "colorInteropID" => {
                if a.kind != "string" {
                    return Err(invalid("EXR colorInteropID type mismatch"));
                }
                color_interop_id = Some(
                    std::str::from_utf8(a.value)
                        .map_err(|_| unsupported("EXR colorInteropID encoding unqualified"))?,
                );
            }
            "type" => {
                if a.kind != "string" || a.value != b"scanlineimage" {
                    return Err(unsupported("EXR part type unqualified"));
                }
            }
            "tiles" | "chunkCount" | "maxSamplesPerPixel" | "version" | "name" | "multiView" => {
                return Err(unsupported(
                    "EXR extra part/tile/deep/view semantics unqualified",
                ));
            }
            _ => {} // Retain exact bounded metadata; future import must report loss.
        }
    }
    Ok(Header {
        width: width as u32,
        height: height as u32,
        origin: [data[0], data[1]],
        channels,
        compression,
        attributes,
        chromaticities,
        color_interop_id,
    })
}
