//! Explicit authority for omitting retained source facts from one exact copy.
//! A receipt/loss name is descriptive and never constructs this authority.
use crate::registry::{Family, Operation};
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;
use uuid::Uuid;

pub const OMITTED_REASON: &str = "source-metadata-omitted";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Intent {
    #[default]
    Refuse,
    Omit,
}
impl std::str::FromStr for Intent {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self> {
        match value {
            "refuse" => Ok(Self::Refuse),
            "omit" => Ok(Self::Omit),
            _ => Err(Error::new(
                ErrorCode::InputSchema,
                "source metadata policy must be refuse or omit",
            )),
        }
    }
}

/// Host-selected target. Backends validate against their own hardcoded target,
/// rather than accepting this value as evidence that an operation is supported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub family: Family,
    pub operation: Operation,
}
impl Target {
    pub const fn new(family: Family, operation: Operation) -> Self {
        Self { family, operation }
    }
    pub(super) fn is_foreign_copy(self) -> bool {
        self.family != Family::Pcnd
            && match self.operation {
                Operation::ExportFlat | Operation::SaveAsCopy => true,
                Operation::ExportAsset => {
                    matches!(self.family, Family::Ico | Family::Cur | Family::Icns)
                }
                _ => false,
            }
    }
    fn copy_allowed(self) -> Result<()> {
        if !self.is_foreign_copy() {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Source metadata omission requires a foreign export copy; PCND and preserving operations retain it",
            ));
        }
        Ok(())
    }
}

/// Private fields prevent arbitrary caller-provided IDs/digests from minting a
/// witness. Capture consumes an explicit intent against the actual frozen export
/// snapshot, which may be different from the live editor while text is drafted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OmissionWitness {
    document: Uuid,
    state: Uuid,
    metadata: Option<[u8; 32]>,
    target: Target,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Policy {
    #[default]
    Refuse,
    Omit(OmissionWitness),
}
fn metadata(doc: &Document) -> Option<[u8; 32]> {
    doc.source_metadata
        .as_ref()
        .map(|metadata| metadata.digest())
}
impl Policy {
    /// Capture once after freezing the real export document and final target.
    /// Owner/session freshness checks remain the responsibility of the host.
    pub fn capture(doc: &Document, target: Target, intent: Intent) -> Result<Self> {
        match intent {
            Intent::Refuse => Ok(Self::Refuse),
            Intent::Omit => {
                target.copy_allowed()?;
                Ok(Self::Omit(OmissionWitness {
                    document: doc.document_id,
                    state: doc.state_id,
                    metadata: metadata(doc),
                    target,
                }))
            }
        }
    }
    /// Return the actual omission, not a requested or hypothetical loss. An
    /// explicit no-metadata witness still rejects changed state/target/metadata.
    pub fn validate(self, doc: &Document, expected: Target) -> Result<bool> {
        match self {
            Self::Refuse => {
                if doc.source_metadata.is_some() {
                    return Err(Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "Export requires an explicit source metadata retention or privacy omission policy",
                    ));
                }
                Ok(false)
            }
            Self::Omit(witness) => {
                expected.copy_allowed()?;
                if witness.document != doc.document_id
                    || witness.state != doc.state_id
                    || witness.metadata != metadata(doc)
                    || witness.target != expected
                {
                    return Err(Error::new(
                        ErrorCode::ConflictStaleBase,
                        "Source metadata omission witness does not match this frozen document and export target",
                    ));
                }
                Ok(witness.metadata.is_some())
            }
        }
    }
}

#[cfg(test)]
mod tests;
