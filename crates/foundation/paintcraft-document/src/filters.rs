//! Immutable ordered point-filter intentions; masks and host actions are separate.
use paintcraft_core::types::{Error, ErrorCode, Result, filters::Descriptor};
use sha2::{Digest, Sha256};
use std::mem::size_of;
use uuid::Uuid;

pub const FILTERS_MAX: usize = 256;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterInstance {
    id: Uuid,
    enabled: bool,
    descriptor: Descriptor,
}
impl FilterInstance {
    pub fn new(id: Uuid, enabled: bool, descriptor: Descriptor) -> Self {
        Self {
            id,
            enabled,
            descriptor,
        }
    }
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn descriptor(&self) -> Descriptor {
        self.descriptor
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterStack {
    instances: Vec<FilterInstance>,
    digest: [u8; 32],
}
impl Default for FilterStack {
    fn default() -> Self {
        Self {
            instances: Vec::new(),
            digest: hash(&[]),
        }
    }
}
impl FilterStack {
    /// Validate count and per-layer IDs before retaining bounded owned storage.
    /// Nil IDs are allowed; IDs need only be unique within this layer's stack.
    pub fn new(mut instances: Vec<FilterInstance>) -> Result<Self> {
        if instances.len() > FILTERS_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Layer filter count exceeds 256",
            ));
        }
        for (index, instance) in instances.iter().enumerate() {
            if instances[..index]
                .iter()
                .any(|prior| prior.id == instance.id)
            {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "Duplicate layer filter ID",
                ));
            }
        }
        if instances.capacity() > FILTERS_MAX {
            let mut bounded = Vec::new();
            bounded.try_reserve_exact(instances.len()).map_err(|_| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "Cannot allocate layer filter stack",
                )
            })?;
            bounded.extend(instances);
            instances = bounded;
        }
        let digest = hash(&instances);
        Ok(Self { instances, digest })
    }
    pub fn from_instances(instances: Vec<FilterInstance>) -> Result<Self> {
        Self::new(instances)
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &FilterInstance> + DoubleEndedIterator {
        self.instances.iter()
    }
    pub fn as_slice(&self) -> &[FilterInstance] {
        &self.instances
    }
    pub fn len(&self) -> usize {
        self.instances.len()
    }
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }
    /// Cached at construction; no source pixels, placement or per-view hashing.
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Owned allocation only. The stack's inline cached digest/Vec is already
    /// included by size_of::<Layer>() / size_of::<SavedLayer>().
    pub fn storage_bytes(&self) -> usize {
        self.instances.capacity() * size_of::<FilterInstance>()
    }
}
fn hash(instances: &[FilterInstance]) -> [u8; 32] {
    // Canonical v1: domain, u16BE count; ordered UUID bytes, enabled u8,
    // u16BE descriptor-ID length + UTF8 ID, then BC i16BE brightness/contrast.
    // IDs already carry arithmetic versions; there are no masks in this schema.
    let mut digest = Sha256::new();
    digest.update(b"paintcraft.filter-stack/v1\0");
    digest.update((instances.len() as u16).to_be_bytes());
    for instance in instances {
        digest.update(instance.id.as_bytes());
        digest.update([u8::from(instance.enabled)]);
        let id = instance.descriptor.id();
        digest.update((id.len() as u16).to_be_bytes());
        digest.update(id.as_bytes());
        if let Descriptor::BrightnessContrastV1(parameters) = instance.descriptor {
            digest.update(parameters.brightness().to_be_bytes());
            digest.update(parameters.contrast().to_be_bytes());
        }
    }
    digest.finalize().into()
}

#[cfg(test)]
mod tests;
