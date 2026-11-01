//! Additive Source7 metadata, independent of legacy numerical inventory layouts.
use super::prepared_capabilities::{self as numerical, Execution, RegistryContract, Requirements};
use paintcraft_core::types::{Error, ErrorCode, Result};
use sha2::{Digest, Sha256};
mod admission;
mod framing;
mod value;
pub use crate::pcnd::layer_appearance::filter_digest_checked;
pub use admission::{Admission7, Mode7};
pub use framing::{
    CoverageAbi, CoverageHasher, CoverageKey, CoverageRow, Fact7, IdentityHasher, Inventory7,
    InventoryHasher, LayerMasks7, Union7,
};
pub const ALGORITHM_MAX: usize = 256;
pub const UNION_MAX: usize = 257;
pub const OWN_MAX: u32 = 100_000;
pub const OCCURRENCE_MAX: u32 = OWN_MAX * 257;
/// A distinct appearance capability, never an invented numerical pair or table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppearanceRegistration {
    pub feature_id: &'static str,
    pub resource_contract_id: &'static str,
    pub resource_contract_revision: u16,
    pub query_revision: u16,
    pub alpha_revision: u16,
    pub workload_class_id: &'static str,
    pub workload_class_revision: u16,
}
pub const fn appearance_registration() -> AppearanceRegistration {
    AppearanceRegistration {
        feature_id: "layer-masks.v1",
        resource_contract_id: "layer-mask-final-alpha-rgba8-v1",
        resource_contract_revision: 1,
        query_revision: 1,
        alpha_revision: 1,
        workload_class_id: "base-work-v1",
        workload_class_revision: 1,
    }
}
impl AppearanceRegistration {
    pub fn validate(self) -> Result<()> {
        if self != appearance_registration() {
            return Err(unsupported());
        }
        Ok(())
    }
    fn append(self, hash: &mut Sha256) -> Result<()> {
        self.validate()?;
        for id in [self.feature_id, self.resource_contract_id] {
            text(hash, id)?;
        }
        hash.update(self.resource_contract_revision.to_be_bytes());
        hash.update(self.query_revision.to_be_bytes());
        hash.update(self.alpha_revision.to_be_bytes());
        text(hash, self.workload_class_id)?;
        hash.update(self.workload_class_revision.to_be_bytes());
        Ok(())
    }
}
/// Fixed numerical set plus a mandatory separate appearance capability.
/// No added field in existing Requirements/Registration and no execution grant.
#[derive(Clone, Debug)]
pub struct Requirements7 {
    numerical: Requirements,
    appearance: AppearanceRegistration,
}
impl Requirements7 {
    pub fn new(
        numerical: Requirements,
        appearance: AppearanceRegistration,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        appearance.validate()?;
        numerical.validate_contract(RegistryContract::LayerAppearance3, check)?;
        if numerical.len() > UNION_MAX {
            return Err(limit());
        }
        Ok(Self {
            numerical,
            appearance,
        })
    }
    pub fn requirements(&self) -> &Requirements {
        &self.numerical
    }
    pub fn appearance(&self) -> AppearanceRegistration {
        self.appearance
    }
    pub fn requires_noise(&self) -> Result<bool> {
        let mut noise = false;
        for (profile, codec) in self.numerical.pairs() {
            noise |= numerical::resolve_for(RegistryContract::LayerAppearance3, profile, codec)?
                .execution()
                == Execution::DirectNoise;
        }
        Ok(noise)
    }
    pub fn union(&self, other: &Self, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        Self::new(
            self.numerical.union(&other.numerical, check)?,
            self.appearance,
            check,
        )
    }
}
fn text(hash: &mut Sha256, value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 96 || !value.is_ascii() {
        return Err(unsupported());
    }
    hash.update(
        u16::try_from(value.len())
            .map_err(|_| limit())?
            .to_be_bytes(),
    );
    hash.update(value.as_bytes());
    Ok(())
}
fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Source7 metadata bound")
}
fn invalid() -> Error {
    Error::new(ErrorCode::InputSchema, "Invalid Source7 metadata")
}
fn unsupported() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Source7 appearance capability unavailable",
    )
}
#[cfg(test)]
mod tests;

/// Conservative Source7 metadata framing slots for ONE Fact7::capture or
/// Union7::capture, including required/resolution/admission hashes. Precharge
/// BEFORE sealing the remaining admission, then use raw cancellation callbacks.
/// This models bounded input framing/hash bytes, not CPU instructions or timing.
pub fn framing_work_quote(required: &Requirements7) -> Result<u64> {
    required.appearance.validate()?;
    let mut bytes = 1024u64;
    let mut entries = 0u64;
    for (profile, codec) in required.numerical.pairs() {
        let entry = numerical::resolve_for(RegistryContract::LayerAppearance3, profile, codec)?;
        let (resource, _) = entry.resource_contract();
        let length = 32usize
            .checked_add(profile.len().checked_mul(2).ok_or_else(limit)?)
            .and_then(|n| n.checked_add(codec.len().checked_mul(2)?))
            .and_then(|n| n.checked_add(resource.len()))
            .ok_or_else(limit)?;
        bytes = bytes.checked_add(length as u64).ok_or_else(limit)?;
        entries += 1;
    }
    bytes
        .checked_mul(16)
        .and_then(|n| n.checked_add(entries.checked_mul(512)?))
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(limit)
}

/// One standalone cached admission SHA, in addition to admission hashes included
/// inside Fact/Union framing quotes. Precharge before sealing the final limits.
pub const fn admission_work_quote() -> u64 {
    let bytes = b"paintcraft.source7-admission/v1\0".len() as u64 + 1 + 4 * 8 + 2 + 2 + 2 * 8;
    4096 + 16 * bytes
}
