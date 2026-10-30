//! Immutable ordered point-filter intentions and source-local coverage.
use paintcraft_core::types::{Error, ErrorCode, Result, filters::Descriptor};
use sha2::{Digest, Sha256};
use std::mem::size_of;
use uuid::Uuid;

pub const FILTERS_MAX: usize = 256;
/// Immutable source-local coverage, retained even when disabled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterMask {
    enabled: bool,
    coverage: paintcraft_raster::coverage_mask::A8Mask,
}
impl FilterMask {
    pub fn new(enabled: bool, coverage: paintcraft_raster::coverage_mask::A8Mask) -> Self {
        Self { enabled, coverage }
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn coverage(&self) -> &paintcraft_raster::coverage_mask::A8Mask {
        &self.coverage
    }
    pub fn with_enabled(&self, enabled: bool) -> Self {
        Self {
            enabled,
            coverage: self.coverage.clone(),
        }
    }
    pub fn with_coverage(&self, coverage: paintcraft_raster::coverage_mask::A8Mask) -> Self {
        Self {
            enabled: self.enabled,
            coverage,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterInstance {
    id: Uuid,
    enabled: bool,
    descriptor: Descriptor,
    mask: Option<FilterMask>,
}
impl FilterInstance {
    pub fn new(id: Uuid, enabled: bool, descriptor: Descriptor) -> Self {
        Self {
            id,
            enabled,
            descriptor,
            mask: None,
        }
    }
    pub fn mask(&self) -> Option<&FilterMask> {
        self.mask.as_ref()
    }
    pub fn with_mask(&self, mask: Option<FilterMask>) -> Self {
        let mut copy = self.clone();
        copy.mask = mask;
        copy
    }
    pub fn with_descriptor(&self, descriptor: Descriptor) -> Self {
        let mut copy = self.clone();
        copy.descriptor = descriptor;
        copy
    }
    pub fn with_enabled(&self, enabled: bool) -> Self {
        let mut copy = self.clone();
        copy.enabled = enabled;
        copy
    }
    pub fn with_id(&self, id: Uuid) -> Self {
        let mut copy = self.clone();
        copy.id = id;
        copy
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
    pub fn mask_storage_usage(&self) -> impl Iterator<Item = (usize, u64)> + '_ {
        self.iter()
            .filter_map(|f| f.mask())
            .filter_map(|m| m.coverage().storage_usage())
    }
    pub fn storage_bytes(&self) -> usize {
        self.instances.capacity() * size_of::<FilterInstance>()
    }
}
fn hash(instances: &[FilterInstance]) -> [u8; 32] {
    // Canonical v1: domain, u16BE count; ordered UUID bytes, enabled u8,
    // u16BE descriptor-ID length + UTF8 ID, then BC i16BE brightness/contrast.
    // IDs carry arithmetic versions. Unmasked stacks retain exact v1 bytes;
    // only stored mask intent opts into v2 presence/enablement/field hashing.
    let mut digest = Sha256::new();
    let masked = instances.iter().any(|f| f.mask().is_some());
    digest.update(if masked {
        b"paintcraft.filter-stack/v2\0"
    } else {
        b"paintcraft.filter-stack/v1\0"
    });
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
        if masked {
            digest.update([u8::from(instance.mask().is_some())]);
            if let Some(mask) = instance.mask() {
                digest.update([u8::from(mask.enabled())]);
                digest.update(mask.coverage().digest());
            }
        }
    }
    digest.finalize().into()
}

#[cfg(test)]
mod mask_tests;
#[cfg(test)]
mod tests;

/// Read-only ordered inspection. This cannot authorize evaluation or publication.
pub fn inspection(stack: &FilterStack) -> paintcraft_core::types::json::Value {
    use paintcraft_core::types::json::Value;
    Value::Array(
        stack
            .iter()
            .map(|instance| {
                let Value::Object(mut fields) = Value::object([
                    ("id", Value::str(instance.id().to_string())),
                    ("enabled", Value::Bool(instance.enabled())),
                    ("descriptor", instance.descriptor().to_value()),
                ]) else {
                    unreachable!()
                };
                if let Some(mask) = instance.mask() {
                    let r = mask.coverage().domain();
                    let stats = mask.coverage().stats();
                    fields.insert(
                        "mask".into(),
                        Value::object([
                            ("schema", Value::str("a8-bands/v1")),
                            ("enabled", Value::Bool(mask.enabled())),
                            (
                                "domain",
                                Value::Array(
                                    [r.x0, r.y0, r.x1, r.y1]
                                        .into_iter()
                                        .map(Value::Int)
                                        .collect(),
                                ),
                            ),
                            ("bands", Value::Int(stats.bands as i64)),
                            ("runs", Value::Int(stats.runs as i64)),
                            (
                                "digest",
                                Value::str(paintcraft_core::types::policy::hex(
                                    &mask.coverage().digest(),
                                )),
                            ),
                        ]),
                    );
                }
                Value::Object(fields)
            })
            .collect(),
    )
}
