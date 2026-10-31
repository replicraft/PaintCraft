//! New generic program-region contract2. Existing region contracts stay unchanged.
use super::*;
impl SourceSpatialContext {
    pub fn capture_programs(
        doc: &Document,
        base_payload: u64,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        // Same explicit region evaluator and conservative point fallback units;
        // acceptance/pricing of numerical profiles is independent in the registry.
        Self::capture_catalog(doc, base_payload, check)
    }
    pub fn encode_programs(self) -> [u8; 64] {
        let mut bytes = self.encode_catalog();
        bytes[2..4].copy_from_slice(&2u16.to_be_bytes());
        bytes
    }
    pub fn decode_programs(bytes: &[u8], cap: u64) -> Result<Self> {
        if bytes.len() != 64 || bytes[2..4] != 2u16.to_be_bytes() {
            return Err(bad("Generic region contract"));
        }
        let mut normalized: [u8; 64] = bytes.try_into().unwrap();
        normalized[2..4].copy_from_slice(&1u16.to_be_bytes());
        Self::decode_catalog(&normalized, cap)
    }
    pub fn validate_programs(self, cap: u64) -> Result<()> {
        self.validate_catalog(cap)
    }
    pub(crate) fn programs_payload(self, payload: u64) -> Result<Self> {
        self.catalog_payload(payload)
    }
    pub fn finalize_programs_source(self, source_bytes: usize, cap: u64) -> Result<Self> {
        self.finalize_catalog_source(source_bytes, cap)
    }
}
