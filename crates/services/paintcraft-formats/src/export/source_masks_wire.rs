//! Protocol4 expanded A8 decode facts. No copy/publication authority is carried.
use crate::pcnd::{self, CoverageFacts};
use paintcraft_core::types::{Error, ErrorCode, Result, resources::SharedBudget};
use paintcraft_document::Document;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceMaskContext {
    facts: CoverageFacts,
}
fn invalid() -> Error {
    Error::new(
        ErrorCode::InputSchema,
        "Invalid browser expanded-mask context",
    )
}
impl SourceMaskContext {
    pub fn capture(doc: &Document) -> Result<Self> {
        let context = Self {
            facts: pcnd::coverage_facts(doc, &|| Ok(()))?,
        };
        context.validate()?;
        Ok(context)
    }
    fn validate(self) -> Result<()> {
        let f = self.facts;
        let limits = paintcraft_raster::coverage_mask::Limits::default();
        if f.masks > 8 * 256
            || f.bands > limits.bands as u64
            || f.runs > limits.runs as u64
            || f.decode_bytes > 256 << 20
            || f.bands > f.runs
            || (f.bands == 0) != (f.runs == 0)
            || (f.masks == 0 && (f.bands != 0 || f.runs != 0 || f.decode_bytes != 0))
            || (f.bands == 0 && f.decode_bytes != 0)
            || (f.bands > 0 && f.decode_bytes == 0)
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn has_masks(self) -> bool {
        self.facts.masks != 0
    }
    pub fn decode_bytes(self) -> u64 {
        self.facts.decode_bytes
    }
    pub fn encode(self) -> [u8; 32] {
        let mut bytes = [0; 32];
        for (at, value) in [0usize, 8, 16, 24].into_iter().zip([
            self.facts.masks,
            self.facts.bands,
            self.facts.runs,
            self.facts.decode_bytes,
        ]) {
            bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 {
            return Err(invalid());
        }
        let value = |at| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap());
        let context = Self {
            facts: CoverageFacts {
                masks: value(0),
                bands: value(8),
                runs: value(16),
                decode_bytes: value(24),
            },
        };
        context.validate()?;
        Ok(context)
    }
    pub(crate) fn check_flags(self, flags: u8) -> Result<()> {
        self.validate()?;
        if (flags & 8 != 0) != self.has_masks() || (self.has_masks() && flags & 4 == 0) {
            return Err(invalid());
        }
        Ok(())
    }
    pub(crate) fn pre_read_quote(self, base: u64) -> Result<u64> {
        base.checked_add(self.facts.decode_bytes)
            .ok_or_else(invalid)
    }
    pub(crate) fn admission<'a>(
        self,
        metadata: pcnd::MetadataAdmission<'a>,
        coverage: &'a SharedBudget,
    ) -> pcnd::ReadAdmission<'a> {
        pcnd::ReadAdmission {
            metadata,
            coverage: pcnd::CoverageAdmission {
                bytes: self.facts.decode_bytes,
                budget: coverage,
                limits: paintcraft_raster::coverage_mask::Limits {
                    bands: self.facts.bands as usize,
                    runs: self.facts.runs as usize,
                    ..Default::default()
                },
            },
        }
    }
    pub(crate) fn validate_inventory(self, doc: &Document) -> Result<()> {
        let actual = pcnd::coverage_facts(doc, &|| Ok(()))?;
        if actual.masks != self.facts.masks
            || actual.bands != self.facts.bands
            || actual.runs != self.facts.runs
            || actual.decode_bytes > self.facts.decode_bytes
        {
            return Err(Error::new(
                ErrorCode::CodecFailed,
                "Browser source mask inventory disagrees with request",
            ));
        }
        Ok(())
    }
    pub(crate) fn validate_decoded(self, doc: &Document) -> Result<()> {
        self.validate_inventory(doc)?;
        let bytes = doc
            .layers
            .values()
            .flat_map(|l| l.filters.mask_storage_usage())
            .try_fold(0u64, |n, (_, bytes)| {
                n.checked_add(bytes).ok_or_else(invalid)
            })?;
        if bytes > self.facts.decode_bytes {
            return Err(invalid());
        }
        Ok(())
    }
}
