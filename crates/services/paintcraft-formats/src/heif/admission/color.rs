//! Initial explicit-color admission inside the isolated helper, not a public gate.
use super::*;
#[derive(Debug)]
pub struct ColorAdmitted<'a> {
    source: &'a [u8],
    size: (u32, u32),
    encoding: Encoding,
    chroma: u32,
    chroma_location: Option<u32>,
    alpha: bool,
}
impl ColorAdmitted<'_> {
    pub fn source(&self) -> &[u8] {
        self.source
    }
    pub fn size(&self) -> (u32, u32) {
        self.size
    }
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }
    pub fn chroma(&self) -> u32 {
        self.chroma
    }
    pub fn chroma_location(&self) -> Option<u32> {
        self.chroma_location
    }
    pub fn alpha(&self) -> bool {
        self.alpha
    }
}
pub(super) fn geometry((width, height): (u32, u32), chroma: u32) -> Result<()> {
    if chroma == 1 && (width % 2 != 0 || height % 2 != 0) {
        return Err(unsupported("odd 4:2:0 HEIF geometry is not qualified"));
    }
    Ok(())
}
pub(super) fn profile(item: &Item<'_>, coded: Option<Encoding>) -> Result<Encoding> {
    let mut declared = None;
    for association in &item.properties {
        match association.property {
            Property::Color(Color::Nclx {
                primaries,
                transfer,
                matrix,
                full_range,
            }) => {
                if declared.is_some() {
                    return Err(unsupported("multiple HEIF color profiles"));
                }
                declared = Some(Encoding {
                    primaries,
                    transfer,
                    matrix,
                    full_range,
                });
            }
            Property::Color(Color::Icc { .. }) => {
                return Err(unsupported("HEIF ICC transform not qualified"));
            }
            _ => {}
        }
    }
    let declared = declared.ok_or_else(|| unsupported("HEIF color requires explicit NCLX"))?;
    if Some(declared) != coded {
        return Err(unsupported("HEIF NCLX and coded VUI must agree explicitly"));
    }
    if declared.primaries != 1 || declared.transfer != 13 || !matches!(declared.matrix, 0 | 1) {
        return Err(unsupported(
            "HEIF color is not a qualified explicit sRGB encoding",
        ));
    }
    if declared.matrix == 0 && !declared.full_range {
        return Err(unsupported("limited-range identity HEIF is not qualified"));
    }
    Ok(declared)
}
/// Only invoke under the codec worker's allocator and deadline boundary. Color
/// admission currently requires a direct primary and optional direct straight
/// monochrome8 alpha. Grids/identity chains await independent color oracles.
pub fn admit_srgb8<'a>(
    bytes: &'a [u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<ColorAdmitted<'a>> {
    let value = inspect_checked(bytes, pixels, check)?;
    let primary = value
        .primary()
        .ok_or_else(|| invalid("HEIF primary missing"))?;
    if primary.kind != *b"hvc1" {
        return Err(unsupported("HEIF color derived image is not yet qualified"));
    }
    let alpha = value.alpha();
    if value.premultiplied_alpha() {
        return Err(unsupported("premultiplied HEIF color is not qualified"));
    }
    if !value
        .references
        .iter()
        .all(|reference| reference.kind == *b"auxl")
    {
        return Err(unsupported("HEIF color reference graph is not qualified"));
    }
    let mut aggregate = 0u64;
    let mut color = None;
    for item in &value.items {
        check()?;
        if item.id != primary.id && alpha.is_none_or(|alpha| alpha.id != item.id) {
            return Err(unsupported(
                "HEIF extra image or metadata items are not qualified",
            ));
        }
        for association in &item.properties {
            match association.property {
                Property::Color(Color::Icc { .. })
                | Property::Mirror { .. }
                | Property::CleanAperture { .. }
                | Property::Rotation {
                    quarter_turns_ccw: 1..,
                } => return Err(unsupported("HEIF ICC/display transform is not qualified")),
                _ => {}
            }
        }
        if item.kind != *b"hvc1" {
            return Err(unsupported("HEIF alpha must be directly coded"));
        }
        let admitted = coded_profile(item, pixels.min(PIXELS_MAX), check, item.id == primary.id)?;
        aggregate = aggregate
            .checked_add(admitted.raw_pixels)
            .ok_or_else(|| resource("HEIF coded aggregate overflow"))?;
        if aggregate > pixels.min(PIXELS_MAX).saturating_mul(2) {
            return Err(resource("HEIF color coded aggregate exceeds allowance"));
        }
        if item.id == primary.id {
            color = Some(admitted);
        } else if admitted.size != spatial(primary)? {
            return Err(unsupported("HEIF alpha geometry differs from primary"));
        }
    }
    let color = color.ok_or_else(|| invalid("HEIF primary was not admitted"))?;
    check()?;
    Ok(ColorAdmitted {
        source: bytes,
        size: color.size,
        encoding: color
            .encoding
            .ok_or_else(|| invalid("HEIF color missing"))?,
        chroma: color.chroma,
        chroma_location: color.chroma_location,
        alpha: alpha.is_some(),
    })
}

#[cfg(test)]
mod tests;
