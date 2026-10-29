//! Deterministic identities for private converted raster documents.
//! A key binds semantic choices and actual samples; it grants no format,
//! resource, consent, filesystem or editor-publication authority.
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    policy::{CANVAS_SIDE_MAX, STAGED_PIXELS_MAX},
};
use sha2::{Digest, Sha256};

pub const CONVERSION_PROFILE_BYTES_MAX: usize = 128;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterSelection {
    Image,
    Page(u16),
}
#[derive(Clone, Copy, Debug)]
pub struct RasterConversionContext<'a> {
    pub source_sha256: [u8; 32],
    /// Versioned conversion policy, not a filename, arbitrary metadata or receipt.
    pub profile: &'a str,
    pub selection: RasterSelection,
    /// Existing conversion-assessment digest, including settings when applicable.
    /// Supplying a digest here does not validate or adopt that assessment.
    pub assessment_sha256: Option<[u8; 32]>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RasterConversionKey([u8; 32]);
impl RasterConversionKey {
    /// Hash bounded context and exact straight RGBA8 samples incrementally.
    /// The pixel allowance is checked but does not affect identity: an adequate
    /// caller budget must not give the same conversion a different state ID.
    pub fn checked(
        context: RasterConversionContext<'_>,
        width: u32,
        height: u32,
        rgba: &[u8],
        pixels_limit: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if context.profile.is_empty()
            || context.profile.len() > CONVERSION_PROFILE_BYTES_MAX
            || !context.profile.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Invalid raster conversion identity profile",
            ));
        }
        if !(1..=CANVAS_SIDE_MAX).contains(&width) || !(1..=CANVAS_SIDE_MAX).contains(&height) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Raster conversion identity dimensions",
            ));
        }
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "Raster conversion identity extent",
                )
            })?;
        if pixels > pixels_limit.min(STAGED_PIXELS_MAX) {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Raster conversion identity pixel allowance",
            ));
        }
        let bytes = pixels.checked_mul(4).ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "Raster conversion identity byte extent",
            )
        })?;
        if u64::try_from(rgba.len()).ok() != Some(bytes) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Raster conversion identity sample length",
            ));
        }
        let mut hash = Sha256::new();
        hash.update(b"paintcraft.raster-conversion-key/v1\0");
        hash.update((context.profile.len() as u16).to_le_bytes());
        hash.update(context.profile.as_bytes());
        hash.update(context.source_sha256);
        match context.selection {
            RasterSelection::Image => hash.update([0]),
            RasterSelection::Page(index) => {
                hash.update([1]);
                hash.update(index.to_le_bytes());
            }
        }
        match context.assessment_sha256 {
            None => hash.update([0]),
            Some(value) => {
                hash.update([1]);
                hash.update(value);
            }
        }
        hash.update(width.to_le_bytes());
        hash.update(height.to_le_bytes());
        hash.update(bytes.to_le_bytes());
        for chunk in rgba.chunks(65536) {
            check()?;
            hash.update(chunk);
        }
        let value = Self(hash.finalize().into());
        check()?;
        Ok(value)
    }
    pub fn digest(self) -> [u8; 32] {
        self.0
    }
    /// Only a document-construction key; never label this the original file SHA.
    pub fn hex(self) -> String {
        paintcraft_core::types::policy::hex(&self.0)
    }
}
#[cfg(test)]
#[path = "conversion_identity/tests.rs"]
mod tests;
