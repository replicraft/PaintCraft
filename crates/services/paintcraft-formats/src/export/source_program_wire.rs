//! Export7 generic prepared facts; decoding support alone never grants execution.
use super::prepared_capabilities::Requirements;
use super::*;
const WORKSPACE_MAX: u64 = 64 << 20;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceProgramContext {
    required: u32,
    stored: u32,
    enabled: u32,
    unique: u32,
    compile: u64,
    retained: u64,
    preparation: u64,
    evaluation: u64,
    work: u64,
    required_digest: [u8; 32],
    resolution_digest: [u8; 32],
}
fn bad() -> Error {
    Error::new(ErrorCode::InputSchema, "Prepared export framing/facts")
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Prepared export admission exceeds profile",
    )
}
impl SourceProgramContext {
    /// Decoded/captured context128 plus echoed response context128. This is a
    /// logical control-copy allowance, NOT the protocol7 wire delta from4.
    /// Existing parser/framing controls include the encoded headers/region facts.
    pub const CONTROL_COPY_BYTES: u64 = 256;
    pub fn capture(
        doc: &Document,
        region: SourceSpatialContext,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let requirements = Requirements::capture(doc, check)?;
        let (required_digest, resolution_digest) = requirements.digests(check)?;
        let mut value = Self {
            required: u32::try_from(requirements.len()).map_err(|_| limit())?,
            required_digest,
            resolution_digest,
            ..Self::default()
        };
        for layer in doc.layers.values() {
            check()?;
            for instance in layer.filters.iter() {
                check()?;
                value.stored = value.stored.checked_add(1).ok_or_else(limit)?;
                value.enabled = value
                    .enabled
                    .checked_add(u32::from(instance.enabled()))
                    .ok_or_else(limit)?;
            }
        }
        let quote = paintcraft_render::prepared::quote_document_programs(doc, check)?;
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
    pub fn validate_requirements(
        self,
        requirements: &Requirements,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        let (required, resolution) = requirements.digests(check)?;
        if self.required as usize != requirements.len()
            || self.required_digest != required
            || self.resolution_digest != resolution
        {
            return Err(bad());
        }
        Ok(())
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
    pub fn required_count(self) -> u32 {
        self.required
    }
    pub fn required_digest(self) -> [u8; 32] {
        self.required_digest
    }
    pub fn resolution_digest(self) -> [u8; 32] {
        self.resolution_digest
    }
    pub(crate) fn phase_peak(self, region: SourceSpatialContext, encode_extra: u64) -> Result<u64> {
        Ok(self.preparation.max(
            self.retained
                .checked_add(region.workspace_bytes().max(encode_extra))
                .ok_or_else(limit)?,
        ))
    }
    pub fn validate_region(self, region: SourceSpatialContext) -> Result<()> {
        if self.required > 256
            || self.stored > 8 * 256
            || self.enabled > self.stored
            || self.required > self.stored
            || self.unique > self.enabled
            || self.unique > 256
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
        // Enabled direct-evaluation filters need not own any prepared programs.
        if self.workspace_peak() > WORKSPACE_MAX || self.work > SPATIAL_WORK_MAX {
            return Err(limit());
        }
        Ok(())
    }
    pub fn encode(self) -> [u8; 128] {
        let mut bytes = [0; 128];
        bytes[..2].copy_from_slice(&1u16.to_be_bytes());
        bytes[2..4].copy_from_slice(&1u16.to_be_bytes());
        for (at, n) in [
            (4, self.required),
            (8, self.stored),
            (12, self.enabled),
            (16, self.unique),
        ] {
            bytes[at..at + 4].copy_from_slice(&n.to_be_bytes());
        }
        for (at, n) in [
            (24, self.compile),
            (32, self.retained),
            (40, self.preparation),
            (48, self.evaluation),
            (56, self.work),
        ] {
            bytes[at..at + 8].copy_from_slice(&n.to_be_bytes());
        }
        bytes[64..96].copy_from_slice(&self.required_digest);
        bytes[96..128].copy_from_slice(&self.resolution_digest);
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 128
            || bytes[..2] != 1u16.to_be_bytes()
            || bytes[2..4] != 1u16.to_be_bytes()
            || bytes[20..24] != [0; 4]
        {
            return Err(bad());
        }
        let n32 = |at| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
        let n64 = |at| u64::from_be_bytes(bytes[at..at + 8].try_into().unwrap());
        Ok(Self {
            required: n32(4),
            stored: n32(8),
            enabled: n32(12),
            unique: n32(16),
            compile: n64(24),
            retained: n64(32),
            preparation: n64(40),
            evaluation: n64(48),
            work: n64(56),
            required_digest: bytes[64..96].try_into().unwrap(),
            resolution_digest: bytes[96..128].try_into().unwrap(),
        })
    }
}

#[cfg(test)]
mod tests;
