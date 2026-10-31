//! Closed initial-eight program admission for export protocol6, not copy authority.
use super::*;
use paintcraft_core::types::filters::Descriptor;
const WORKSPACE_MAX: u64 = 64 << 20;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceCatalogContext {
    stored: u32,
    enabled: u32,
    unique: u32,
    compile: u64,
    retained: u64,
    preparation: u64,
    evaluation: u64,
    work: u64,
}
fn bad() -> Error {
    Error::new(ErrorCode::InputSchema, "Catalog export framing/facts")
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Catalog export admission exceeds profile",
    )
}
fn initial(profile: &str) -> bool {
    matches!(
        profile,
        "invert-rgb-v1"
            | "brightness-contrast-v1"
            | "black-white-rgb-v1"
            | "threshold-luma-v1"
            | "posterize-rgb-v1"
            | "solarize-rgb-v1"
            | "box-blur-v1"
            | "levels-rgb-v1"
    )
}
impl SourceCatalogContext {
    /// The Worker capability stays closed even if the general PCND6 reader grows.
    pub fn validate_source(source: &[u8], check: &dyn Fn() -> Result<()>) -> Result<()> {
        if !source.starts_with(b"PCND\x06\r\n\x1a") {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Export Worker6 requires PCND6 source",
            ));
        }
        for row in crate::pcnd::catalog_manifest_checked(source, check)? {
            check()?;
            if !initial(&row.profile_id) || row.codec_id != "params-v1" {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Export Worker6 admits only initial eight profiles/params-v1",
                ));
            }
        }
        Ok(())
    }
    pub fn capture(
        doc: &Document,
        region: SourceSpatialContext,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let mut value = Self::default();
        for layer in doc.layers.values() {
            check()?;
            for filter in layer.filters.iter() {
                check()?;
                if !initial(filter.descriptor().id()) {
                    return Err(Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "Export Worker6 numerical profile is not priced",
                    ));
                }
                if matches!(filter.descriptor(), Descriptor::LevelsRgbV1(_)) {
                    value.stored = value.stored.checked_add(1).ok_or_else(limit)?;
                    value.enabled = value
                        .enabled
                        .checked_add(u32::from(filter.enabled()))
                        .ok_or_else(limit)?;
                }
            }
        }
        let quote = paintcraft_render::prepared::quote_document_levels(doc, check)?;
        value.unique = u32::try_from(quote.unique_programs).map_err(|_| limit())?;
        value.compile = quote.compile_work;
        value.retained = quote.retained_bytes;
        value.preparation = quote.preparation_peak_bytes;
        value.evaluation = value
            .retained
            .checked_add(region.workspace_bytes())
            .ok_or_else(limit)?;
        value.work = value.compile.checked_add(region.work()).ok_or_else(limit)?;
        value.validate_region(region)?;
        Ok(value)
    }
    pub fn workspace_peak(self) -> u64 {
        self.preparation.max(self.evaluation)
    }
    pub fn retained_bytes(self) -> u64 {
        self.retained
    }
    pub fn compile_work(self) -> u64 {
        self.compile
    }
    pub fn work(self) -> u64 {
        self.work
    }
    /// Keep prepared ownership through encoding/thumbnail. Extra packed bytes
    /// coexist with retained programs, but not region scratch or compile scratch.
    pub(crate) fn phase_peak(self, region: SourceSpatialContext, encode_extra: u64) -> Result<u64> {
        Ok(self.preparation.max(
            self.retained
                .checked_add(region.workspace_bytes().max(encode_extra))
                .ok_or_else(limit)?,
        ))
    }
    pub fn validate_region(self, region: SourceSpatialContext) -> Result<()> {
        if self.stored == 0
            || self.stored > 8 * 256
            || self.enabled > self.stored
            || self.unique > self.enabled
            || self.unique > 256
            || (self.enabled != 0 && self.unique == 0)
            || (self.unique == 0
                && (self.compile != 0 || self.retained != 0 || self.preparation != 0))
            || (self.unique != 0
                && (self.compile == 0 || self.retained < u64::from(self.unique) * 256))
            || self.preparation < self.retained
            || self.evaluation
                != self
                    .retained
                    .checked_add(region.workspace_bytes())
                    .ok_or_else(limit)?
            || self.work != self.compile.checked_add(region.work()).ok_or_else(limit)?
        {
            return Err(bad());
        }
        if self.workspace_peak() > WORKSPACE_MAX || self.work > SPATIAL_WORK_MAX {
            return Err(limit());
        }
        Ok(())
    }
    pub fn encode(self) -> [u8; 64] {
        let mut bytes = [0; 64];
        bytes[..2].copy_from_slice(&6u16.to_be_bytes());
        bytes[2..4].copy_from_slice(&1u16.to_be_bytes());
        for (at, n) in [(4, self.stored), (8, self.enabled), (12, self.unique)] {
            bytes[at..at + 4].copy_from_slice(&n.to_be_bytes());
        }
        for (at, n) in [
            (16, self.compile),
            (24, self.retained),
            (32, self.preparation),
            (40, self.evaluation),
            (48, self.work),
        ] {
            bytes[at..at + 8].copy_from_slice(&n.to_be_bytes());
        }
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 64
            || bytes[..2] != 6u16.to_be_bytes()
            || bytes[2..4] != 1u16.to_be_bytes()
            || bytes[56..] != [0; 8]
        {
            return Err(bad());
        }
        let n = |at| u64::from_be_bytes(bytes[at..at + 8].try_into().unwrap());
        Ok(Self {
            stored: u32::from_be_bytes(bytes[4..8].try_into().unwrap()),
            enabled: u32::from_be_bytes(bytes[8..12].try_into().unwrap()),
            unique: u32::from_be_bytes(bytes[12..16].try_into().unwrap()),
            compile: n(16),
            retained: n(24),
            preparation: n(32),
            evaluation: n(40),
            work: n(48),
        })
    }
}
