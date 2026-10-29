//! Bound source facts and explicit copy intent for isolated DDS/MSP preparation.
//! This wire context is descriptive until validated against an actual document;
//! only the host's separately retained Policy authorizes publication.
use super::metadata_policy::{Intent, Policy, Target};
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::{
    Document,
    source_metadata::{BYTES_MAX, WIRE_HEADER_BYTES},
};
use uuid::Uuid;

pub const CONTEXT_BYTES: usize = 64;
// Existing browser snapshot title/layer/control allowance, not a new ICC cap.
const CONTROL_BYTES_MAX: u64 = 65_536;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceMetadataContext {
    document: Uuid,
    target: Target,
    digest: Option<[u8; 32]>,
    omit_intent: bool,
    wire_bytes: u32,
    retained_charge: u64,
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
impl SourceMetadataContext {
    pub(crate) fn capture(doc: &Document, policy: Policy, target: Target) -> Result<Self> {
        policy.validate(doc, target)?;
        let (digest, wire_bytes, retained_charge) = match &doc.source_metadata {
            Some(metadata) => (
                Some(metadata.digest()),
                u32::try_from(WIRE_HEADER_BYTES + metadata.profile().map_or(0, <[u8]>::len))
                    .map_err(|_| invalid("Browser source attachment extent"))?,
                metadata.storage_usage().1,
            ),
            None => (None, 0, 0),
        };
        let context = Self {
            document: doc.document_id,
            target,
            digest,
            omit_intent: matches!(policy, Policy::Omit(_)),
            wire_bytes,
            retained_charge,
        };
        context.validate()?;
        Ok(context)
    }
    pub fn target(&self) -> Target {
        self.target
    }
    pub fn document(&self) -> Uuid {
        self.document
    }
    pub fn metadata_digest(&self) -> Option<[u8; 32]> {
        self.digest
    }
    pub fn omit_intent(&self) -> bool {
        self.omit_intent
    }
    pub fn omitted(&self) -> bool {
        self.omit_intent && self.digest.is_some()
    }
    pub fn wire_bytes(&self) -> u32 {
        self.wire_bytes
    }
    pub fn retained_charge(&self) -> u64 {
        self.retained_charge
    }
    fn validate(&self) -> Result<()> {
        if !matches!(
            self.target.family,
            crate::registry::Family::Dds | crate::registry::Family::Msp
        ) || self.target.operation != crate::registry::Operation::ExportFlat
        {
            return Err(invalid(
                "Browser metadata context target is not DDS/MSP flat export",
            ));
        }
        if self.digest.is_none() {
            if self.wire_bytes != 0 || self.retained_charge != 0 {
                return Err(invalid("Absent browser attachment has nonzero storage"));
            }
        } else if !self.omit_intent
            || u64::from(self.wire_bytes) < WIRE_HEADER_BYTES as u64
            || u64::from(self.wire_bytes) > BYTES_MAX
            || self.retained_charge < u64::from(self.wire_bytes)
            || self.retained_charge > BYTES_MAX + CONTROL_BYTES_MAX
        {
            return Err(invalid("Browser attachment copy policy/storage bounds"));
        }
        Ok(())
    }
    pub fn encode(&self) -> [u8; CONTEXT_BYTES] {
        let mut bytes = [0; CONTEXT_BYTES];
        bytes[..16].copy_from_slice(self.document.as_bytes());
        if let Some(digest) = self.digest {
            bytes[16..48].copy_from_slice(&digest);
        }
        bytes[48] = u8::from(self.digest.is_some()) | u8::from(self.omit_intent) << 1;
        bytes[49] = if self.target.family == crate::registry::Family::Dds {
            1
        } else {
            2
        };
        bytes[50] = 1; // ExportFlat; no preserving/open/PCND operation on this wire.
        bytes[52..56].copy_from_slice(&self.wire_bytes.to_be_bytes());
        bytes[56..64].copy_from_slice(&self.retained_charge.to_be_bytes());
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CONTEXT_BYTES
            || bytes[48] > 3
            || !matches!(bytes[49], 1 | 2)
            || bytes[50] != 1
            || bytes[51] != 0
            || (bytes[48] & 1 == 0 && bytes[16..48] != [0; 32])
        {
            return Err(invalid("Browser source metadata context framing"));
        }
        let context = Self {
            document: Uuid::from_bytes(bytes[..16].try_into().unwrap()),
            target: Target::new(
                if bytes[49] == 1 {
                    crate::registry::Family::Dds
                } else {
                    crate::registry::Family::Msp
                },
                crate::registry::Operation::ExportFlat,
            ),
            digest: (bytes[48] & 1 != 0).then(|| bytes[16..48].try_into().unwrap()),
            omit_intent: bytes[48] & 2 != 0,
            wire_bytes: u32::from_be_bytes(bytes[52..56].try_into().unwrap()),
            retained_charge: u64::from_be_bytes(bytes[56..64].try_into().unwrap()),
        };
        context.validate()?;
        Ok(context)
    }
    /// Must run before metadata capture, with the existing bounded raster/source
    /// workspace quote. The subsequent reader's realm denies understated charge
    /// before allocation. This is logical admission, not an allocator/RSS hook.
    pub fn check_capture_quote(&self, other_bytes: u64, ceiling: u64) -> Result<()> {
        self.validate()?;
        if other_bytes
            .checked_add(self.retained_charge)
            .is_none_or(|n| n > ceiling)
        {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Browser attachment workspace admission",
            ));
        }
        Ok(())
    }
    /// Decode-site intent is not host publication authority. The parent still
    /// validates its original captured Policy and exact response binding.
    pub fn validate_document(&self, doc: &Document, target: Target) -> Result<Policy> {
        self.validate()?;
        let actual_digest = doc.source_metadata.as_ref().map(|m| m.digest());
        let actual_wire = doc.source_metadata.as_ref().map_or(0, |m| {
            WIRE_HEADER_BYTES + m.profile().map_or(0, <[u8]>::len)
        });
        let actual_charge = doc
            .source_metadata
            .as_ref()
            .map_or(0, |m| m.storage_usage().1);
        if target != self.target
            || doc.document_id != self.document
            || actual_digest != self.digest
            || actual_wire as u64 != u64::from(self.wire_bytes)
            || actual_charge > self.retained_charge
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Browser source attachment does not match frozen context",
            ));
        }
        Policy::capture(
            doc,
            target,
            if self.omit_intent {
                Intent::Omit
            } else {
                Intent::Refuse
            },
        )
    }
}
