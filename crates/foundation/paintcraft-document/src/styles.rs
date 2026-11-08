//! Immutable style vocabulary only; no Layer field or evaluation authority.
//! These policies remain separate from numerical filter descriptors/catalogs.
use paintcraft_core::types::{Error, ErrorCode, Result, styles::ColorOverlayRgbV1};
use sha2::{Digest, Sha256};
use std::mem::size_of;
use uuid::Uuid;

pub const STYLES_MAX: usize = 256;
const DOMAIN: &[u8] = b"paintcraft.layer-style-stack/v1\0";

/// Closed style namespace. Adding a variant requires a new qualified profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StyleDescriptor {
    ColorOverlayRgbV1(ColorOverlayRgbV1),
}
impl StyleDescriptor {
    pub const fn profile_id(self) -> &'static str {
        match self {
            Self::ColorOverlayRgbV1(_) => ColorOverlayRgbV1::PROFILE,
        }
    }
    pub const fn semantic_parameter_bytes(self) -> [u8; 5] {
        match self {
            Self::ColorOverlayRgbV1(policy) => policy.semantic_bytes(),
        }
    }
}

/// Stable nonnil identity, scoped to one eventual layer's style stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StyleInstance {
    id: Uuid,
    enabled: bool,
    descriptor: StyleDescriptor,
}
impl StyleInstance {
    pub fn new(id: Uuid, enabled: bool, descriptor: StyleDescriptor) -> Result<Self> {
        valid_id(id)?;
        Ok(Self {
            id,
            enabled,
            descriptor,
        })
    }
    pub const fn id(&self) -> Uuid {
        self.id
    }
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
    pub const fn descriptor(&self) -> StyleDescriptor {
        self.descriptor
    }
    pub fn with_descriptor(&self, descriptor: StyleDescriptor) -> Self {
        Self {
            descriptor,
            ..*self
        }
    }
    pub fn with_enabled(&self, enabled: bool) -> Self {
        Self { enabled, ..*self }
    }
    /// Duplicating a style must supply its new nonnil ID explicitly.
    pub fn with_id(&self, id: Uuid) -> Result<Self> {
        Self::new(id, self.enabled, self.descriptor)
    }
}

/// Immutable bounded intentions, including disabled and zero-opacity entries.
/// No source pixels, masks, history owners or execution grants are retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleStack {
    instances: Vec<StyleInstance>,
    digest: [u8; 32],
}
impl Default for StyleStack {
    fn default() -> Self {
        Self::new(Vec::new()).expect("an empty style stack is valid")
    }
}
impl StyleStack {
    pub fn new(instances: Vec<StyleInstance>) -> Result<Self> {
        Self::new_checked(instances, &|| Ok(()))
    }
    /// Cancellation is not a resource grant or a complete work-accounting model.
    /// Admitted callers must price validation/hash/clone/allocation separately.
    pub fn new_checked(
        mut instances: Vec<StyleInstance>,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        if instances.len() > STYLES_MAX {
            return Err(limit("Layer style count exceeds 256"));
        }
        for (index, instance) in instances.iter().enumerate() {
            check()?;
            valid_id(instance.id)?;
            if instances[..index]
                .iter()
                .any(|prior| prior.id == instance.id)
            {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "Duplicate layer style ID",
                ));
            }
        }
        check()?;
        if instances.capacity() > STYLES_MAX {
            let mut bounded = Vec::new();
            bounded
                .try_reserve_exact(instances.len())
                .map_err(|_| limit("Cannot allocate layer style stack"))?;
            if bounded.capacity() > STYLES_MAX {
                return Err(limit("Layer style stack capacity exceeds bound"));
            }
            bounded.extend(instances);
            instances = bounded;
        }
        let digest = hash(&instances, check)?;
        Ok(Self { instances, digest })
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &StyleInstance> + DoubleEndedIterator {
        self.instances.iter()
    }
    pub fn as_slice(&self) -> &[StyleInstance] {
        &self.instances
    }
    pub fn len(&self) -> usize {
        self.instances.len()
    }
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }
    /// Cached complete ordered intent; independent of raster or transport bytes.
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Owned vector payload only; containing struct accounts for inline fields.
    /// This is declared capacity, not allocator/RSS or shared-budget admission.
    pub fn storage_bytes(&self) -> usize {
        self.instances.capacity() * size_of::<StyleInstance>()
    }
}
fn valid_id(id: Uuid) -> Result<()> {
    if id.is_nil() {
        Err(Error::new(
            ErrorCode::InputSchema,
            "Layer style ID must not be nil",
        ))
    } else {
        Ok(())
    }
}
fn limit(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn hash(instances: &[StyleInstance], check: &dyn Fn() -> Result<()>) -> Result<[u8; 32]> {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((instances.len() as u16).to_be_bytes());
    for instance in instances {
        check()?;
        hash.update(instance.id.as_bytes());
        hash.update([u8::from(instance.enabled)]);
        let profile = instance.descriptor.profile_id();
        hash.update((profile.len() as u16).to_be_bytes());
        hash.update(profile.as_bytes());
        let parameters = instance.descriptor.semantic_parameter_bytes();
        hash.update((parameters.len() as u32).to_be_bytes());
        hash.update(parameters);
    }
    check()?;
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod tests;
