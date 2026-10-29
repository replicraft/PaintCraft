//! Original PNG declarations before expansion into the editor's straight RGBA8.
//! Inspection does not apply a transfer function or retain metadata in a document.
use super::raster_source_metadata::{self as facts, Digest, Icc, Palette, Resolution};
use paintcraft_core::types::Result;
use sha2::{Digest as _, Sha256};
use std::io::Cursor;
mod chunks;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorDeclaration {
    Unspecified,
    GammaChromaticities,
    Srgb,
    Icc,
    Cicp,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descriptor {
    pub source: Digest,
    pub width: u32,
    pub height: u32,
    pub color_type: u8,
    pub bit_depth: u8,
    pub interlaced: bool,
    pub palette: Palette,
    /// Original tRNS bytes, including gray/RGB transparent sample declarations.
    pub transparency: Vec<u8>,
    pub gamma_scaled: Option<u32>,
    pub chromaticities_scaled: Option<[u32; 8]>,
    pub srgb_intent: Option<u8>,
    pub cicp: Option<[u8; 4]>,
    pub mastering_display: Option<[u8; 24]>,
    pub content_light: Option<[u8; 8]>,
    pub icc: Option<Icc>,
    /// Original eXIf payload identity only; no EXIF interpretation or retention.
    pub exif: Option<Digest>,
    pub resolution: Option<Resolution>,
    pub significant_bits: Vec<u8>,
    pub omitted_ancillary_count: u64,
    pub omitted_ancillary_bytes: u64,
    pub omitted_ancillary_sha256: [u8; 32],
    pub other_metadata: bool,
}
impl Descriptor {
    /// Nominal packed source-sample row, excluding the filter byte. Adam7 has
    /// distinct pass rows; its compressed/interlaced storage has no single stride.
    pub fn packed_row_bytes(&self) -> Option<u64> {
        if self.interlaced {
            return None;
        }
        let channels = match self.color_type {
            0 | 3 => 1,
            2 => 3,
            4 => 2,
            6 => 4,
            _ => return None,
        };
        Some((u64::from(self.width) * channels * u64::from(self.bit_depth)).div_ceil(8))
    }
    /// PNG3 color-chunk priority. This records authority, not a CMS conversion.
    pub fn color_declaration(&self) -> ColorDeclaration {
        if self.cicp.is_some() {
            ColorDeclaration::Cicp
        } else if self.icc.is_some() {
            ColorDeclaration::Icc
        } else if self.srgb_intent.is_some() {
            ColorDeclaration::Srgb
        } else if self.gamma_scaled.is_some() || self.chromaticities_scaled.is_some() {
            ColorDeclaration::GammaChromaticities
        } else {
            ColorDeclaration::Unspecified
        }
    }
    pub fn losses(&self) -> Vec<&'static str> {
        let mut losses = Vec::new();
        if self.color_type == 3 {
            losses.push("palette-indices-expanded");
        }
        if self.icc.is_some() {
            losses.push("icc-profile-ignored");
        }
        if self.exif.is_some() {
            losses.push("exif-metadata-ignored");
        }
        if self.gamma_scaled.is_some()
            || self.chromaticities_scaled.is_some()
            || self.srgb_intent.is_some()
            || self.cicp.is_some()
            || self.mastering_display.is_some()
            || self.content_light.is_some()
        {
            losses.push("png-color-metadata-ignored");
        }
        if self.resolution.is_some() {
            losses.push("resolution-metadata-ignored");
        }
        if self.other_metadata || !self.significant_bits.is_empty() {
            losses.push("png-metadata-ignored");
        }
        losses
    }
}
pub fn inspect(bytes: &[u8], pixels: u64, check: &dyn Fn() -> Result<()>) -> Result<Descriptor> {
    let allowance = facts::allowance(bytes, pixels)?;
    inspect_with_metadata_limit(bytes, pixels, allowance, check)
}
/// An explicit caller limit, not a new default format admission restriction.
pub fn inspect_with_metadata_limit(
    bytes: &[u8],
    pixels: u64,
    metadata_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Descriptor> {
    check()?;
    let allowance = facts::allowance(bytes, pixels)?;
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: allowance as usize,
        },
    );
    let header = decoder
        .read_header_info()
        .map_err(|e| facts::invalid(&format!("PNG source header: {e}")))?;
    super::check_size(u64::from(header.width), u64::from(header.height))?;
    if u64::from(header.width) * u64::from(header.height) > pixels.min(super::DECODE_PIXELS_MAX) {
        return Err(facts::resource("PNG metadata pixel allowance"));
    }
    let mut result = Descriptor {
        source: facts::digest(bytes, check)?,
        width: header.width,
        height: header.height,
        color_type: header.color_type as u8,
        bit_depth: header.bit_depth as u8,
        interlaced: header.interlaced,
        palette: Palette::default(),
        transparency: Vec::new(),
        gamma_scaled: None,
        chromaticities_scaled: None,
        srgb_intent: None,
        cicp: None,
        mastering_display: None,
        content_light: None,
        icc: None,
        exif: None,
        resolution: None,
        significant_bits: Vec::new(),
        omitted_ancillary_count: 0,
        omitted_ancillary_bytes: 0,
        omitted_ancillary_sha256: [0; 32],
        other_metadata: false,
    };
    let mut omitted = Sha256::new();
    chunks::inspect(
        bytes,
        metadata_limit.min(allowance),
        &mut result,
        &mut omitted,
        check,
    )?;
    result.omitted_ancillary_sha256 = omitted.finalize().into();
    check()?;
    Ok(result)
}
#[cfg(test)]
mod tests;
