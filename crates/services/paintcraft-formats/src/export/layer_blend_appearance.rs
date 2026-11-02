//! Source8 metadata only: no adoption, prepared owner or execution grant.
use super::layer_appearance::{
    AppearanceRegistration, CoverageAbi, Inventory7, LayerMasks7, Mode7, Origin7,
};
use super::prepared_capabilities::{Execution, RegistryContract, Requirements};
use paintcraft_core::types::{Error, ErrorCode, Result};
use sha2::{Digest, Sha256};
mod admission;
mod framing;
mod value;
mod work;
pub use admission::Admission8;
pub use framing::{Fact8, NumericalFrames8, Union8};
pub use work::{admission_work_quote, framing_work_quote};
pub const ALGORITHM_MAX: usize = 256;
pub const UNION_MAX: usize = 257;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlendRegistration {
    pub feature_id: &'static str,
    pub resource_contract_id: &'static str,
    pub resource_contract_revision: u16,
    pub arithmetic_revision: u16,
    pub work_revision: u16,
    pub context_setup_per_root: u64,
    pub opaque_extra_work: u64,
    pub general_work: u64,
    pub workload_class_id: &'static str,
    pub workload_class_revision: u16,
}
pub const fn blend_registration() -> BlendRegistration {
    BlendRegistration {
        feature_id: "layer-blend-modes.v1",
        resource_contract_id: "ordered-layer-blend-rgba8-v1",
        resource_contract_revision: 1,
        arithmetic_revision: 1,
        work_revision: 1,
        context_setup_per_root: 256,
        opaque_extra_work: 32,
        general_work: 256,
        workload_class_id: "base-work-v1",
        workload_class_revision: 1,
    }
}
impl BlendRegistration {
    pub fn validate(self) -> Result<()> {
        if self != blend_registration() {
            return Err(unsupported());
        }
        Ok(())
    }
    fn append(self, h: &mut Sha256) -> Result<()> {
        self.validate()?;
        text(h, self.feature_id)?;
        text(h, self.resource_contract_id)?;
        for n in [
            self.resource_contract_revision,
            self.arithmetic_revision,
            self.work_revision,
        ] {
            h.update(n.to_be_bytes());
        }
        for n in [
            self.context_setup_per_root,
            self.opaque_extra_work,
            self.general_work,
        ] {
            h.update(n.to_be_bytes());
        }
        text(h, self.workload_class_id)?;
        h.update(self.workload_class_revision.to_be_bytes());
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct Requirements8 {
    numerical: Requirements,
    mask: AppearanceRegistration,
    blend: BlendRegistration,
}
impl Requirements8 {
    pub fn new(
        numerical: Requirements,
        mask: AppearanceRegistration,
        blend: BlendRegistration,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        mask.validate()?;
        blend.validate()?;
        numerical.validate_contract(RegistryContract::LayerAppearance4, check)?;
        if numerical.len() > UNION_MAX {
            return Err(limit());
        }
        Ok(Self {
            numerical,
            mask,
            blend,
        })
    }
    pub fn requirements(&self) -> &Requirements {
        &self.numerical
    }
    pub fn mask_appearance(&self) -> AppearanceRegistration {
        self.mask
    }
    pub fn blend_appearance(&self) -> BlendRegistration {
        self.blend
    }
    pub fn requires_noise(&self) -> Result<bool> {
        let mut noise = false;
        for (p, c) in self.numerical.pairs() {
            noise |= super::prepared_capabilities::resolve_for(
                RegistryContract::LayerAppearance4,
                p,
                c,
            )?
            .execution()
                == Execution::DirectNoise;
        }
        Ok(noise)
    }
    pub fn union(&self, other: &Self, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        Self::new(
            self.numerical.union(&other.numerical, check)?,
            self.mask,
            self.blend,
            check,
        )
    }
}
fn text(h: &mut Sha256, v: &str) -> Result<()> {
    if v.is_empty() || v.len() > 96 || !v.is_ascii() {
        return Err(unsupported());
    }
    h.update(u16::try_from(v.len()).map_err(|_| limit())?.to_be_bytes());
    h.update(v.as_bytes());
    Ok(())
}
fn mask_append(mask: AppearanceRegistration, h: &mut Sha256) -> Result<()> {
    mask.validate()?;
    text(h, mask.feature_id)?;
    text(h, mask.resource_contract_id)?;
    for n in [
        mask.resource_contract_revision,
        mask.query_revision,
        mask.alpha_revision,
    ] {
        h.update(n.to_be_bytes());
    }
    text(h, mask.workload_class_id)?;
    h.update(mask.workload_class_revision.to_be_bytes());
    Ok(())
}
fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Source8 metadata bound")
}
fn invalid() -> Error {
    Error::new(ErrorCode::InputSchema, "Invalid Source8 metadata")
}
fn unsupported() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Source8 appearance unavailable",
    )
}
#[cfg(test)]
mod tests;
