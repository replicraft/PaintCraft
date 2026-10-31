//! Priced prepared-worker registry, separate from general PCND decoding support.
//! Registration is source-staged until the complete Export7 graph is coherent.
use paintcraft_core::types::{Error, ErrorCode, Result, filters::Descriptor};
use paintcraft_document::Document;
use sha2::{Digest, Sha256};
mod decoders;
const PAIRS_MAX: usize = 257;
pub const CONTROL_BYTES: u64 =
    crate::pcnd::REQUIREMENTS_CONTROL_BYTES + std::mem::size_of::<ResolvedRequirements>() as u64;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Execution {
    DirectPoint,
    RegionBox,
    PreparedLevels,
    PreparedExposure,
    PreparedCurves,
}
#[derive(Clone, Copy, Debug)]
pub struct Registration {
    profile: &'static str,
    codec: &'static str,
    decode: fn(&[u8]) -> Result<Descriptor>,
    execution: Execution,
    resource: &'static str,
    revision: u32,
}
impl Registration {
    pub fn profile(self) -> &'static str {
        self.profile
    }
    pub fn codec(self) -> &'static str {
        self.codec
    }
    pub fn execution(self) -> Execution {
        self.execution
    }
    pub fn resource_contract(self) -> (&'static str, u32) {
        (self.resource, self.revision)
    }
    pub fn decode(self, bytes: &[u8]) -> Result<Descriptor> {
        (self.decode)(bytes)
    }
}
// Every entry names an actual strict decoder, execution route and priced model.
// A new general reader arm cannot extend this table implicitly.
static REGISTRY: [Registration; 10] = [
    entry(
        "invert-rgb-v1",
        decoders::invert,
        Execution::DirectPoint,
        "direct-rgba8-point-v1",
    ),
    entry(
        "brightness-contrast-v1",
        decoders::brightness,
        Execution::DirectPoint,
        "direct-rgba8-point-v1",
    ),
    entry(
        "black-white-rgb-v1",
        decoders::black_white,
        Execution::DirectPoint,
        "direct-rgba8-point-v1",
    ),
    entry(
        "threshold-luma-v1",
        decoders::threshold,
        Execution::DirectPoint,
        "direct-rgba8-point-v1",
    ),
    entry(
        "posterize-rgb-v1",
        decoders::posterize,
        Execution::DirectPoint,
        "direct-rgba8-point-v1",
    ),
    entry(
        "solarize-rgb-v1",
        decoders::solarize,
        Execution::DirectPoint,
        "direct-rgba8-point-v1",
    ),
    entry(
        "box-blur-v1",
        decoders::box_blur,
        Execution::RegionBox,
        "premultiplied-region-rgba8-v1",
    ),
    entry(
        "levels-rgb-v1",
        decoders::levels,
        Execution::PreparedLevels,
        "prepared-levels-rgba8-v1",
    ),
    entry(
        "exposure-srgb-v1",
        decoders::exposure,
        Execution::PreparedExposure,
        "prepared-exposure-rgba8-v1",
    ),
    entry(
        "curves-rgb-v1",
        decoders::curves,
        Execution::PreparedCurves,
        "prepared-curves-rgba8-v1",
    ),
];
const fn entry(
    profile: &'static str,
    decode: fn(&[u8]) -> Result<Descriptor>,
    execution: Execution,
    resource: &'static str,
) -> Registration {
    Registration {
        profile,
        codec: "params-v1",
        decode,
        execution,
        resource,
        revision: 1,
    }
}
pub fn resolve(profile: &str, codec: &str) -> Result<&'static Registration> {
    REGISTRY
        .iter()
        .find(|entry| entry.profile == profile && entry.codec == codec)
        .ok_or_else(|| {
            Error::new(
                ErrorCode::CapabilityUnsupported,
                "Prepared worker implementation or pricing is unavailable",
            )
        })
}
pub fn registrations() -> &'static [Registration] {
    &REGISTRY
}
/// Strict execution resolution plus minimal-profile selection for a prospective
/// descriptor. The initial-eight predicate is only legacy routing, not registry
/// authority; future priced registrations can use the same generic profile.
pub fn descriptor_requires_generic(descriptor: Descriptor) -> Result<bool> {
    let registered = resolve(descriptor.id(), "params-v1")?;
    let (bytes, length) = descriptor.semantic_parameter_bytes();
    if registered.decode(&bytes[..length])? != descriptor {
        return Err(invalid());
    }
    Ok(!matches!(
        descriptor.id(),
        "invert-rgb-v1"
            | "brightness-contrast-v1"
            | "black-white-rgb-v1"
            | "threshold-luma-v1"
            | "posterize-rgb-v1"
            | "solarize-rgb-v1"
            | "box-blur-v1"
            | "levels-rgb-v1"
    ))
}
/// Inspect actual stored intentions, including disabled/default descriptors.
pub fn requires_generic(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<bool> {
    let mut generic = false;
    for layer in doc.layers.values() {
        for instance in layer.filters.iter() {
            check()?;
            generic |= descriptor_requires_generic(instance.descriptor())?;
        }
    }
    Ok(generic)
}
#[derive(Clone, Debug)]
pub struct Requirements {
    entries: [Option<&'static Registration>; PAIRS_MAX],
    len: usize,
}
/// Charged borrowed assessment inventory. Its parent account survives the value.
#[derive(Debug)]
pub struct ResolvedRequirements {
    value: Requirements,
    _account: paintcraft_core::types::resources::BudgetAccount,
}
impl ResolvedRequirements {
    pub fn requirements(&self) -> &Requirements {
        &self.value
    }
}
impl Default for Requirements {
    fn default() -> Self {
        Self {
            entries: [None; PAIRS_MAX],
            len: 0,
        }
    }
}
impl Requirements {
    pub fn pairs(&self) -> impl ExactSizeIterator<Item = (&'static str, &'static str)> + '_ {
        self.entries[..self.len].iter().map(|entry| {
            let entry = entry.expect("initialized requirement prefix");
            (entry.profile, entry.codec)
        })
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn insert(&mut self, profile: &str, codec: &str) -> Result<()> {
        let entry = resolve(profile, codec)?;
        if self.entries[..self.len]
            .iter()
            .flatten()
            .any(|old| old.profile == profile && old.codec == codec)
        {
            return Ok(());
        }
        if self.len == PAIRS_MAX {
            return Err(limit());
        }
        self.entries[self.len] = Some(entry);
        self.len += 1;
        self.entries[..self.len].sort_unstable_by_key(|entry| {
            let entry = entry.expect("initialized requirement prefix");
            (entry.profile, entry.codec)
        });
        Ok(())
    }
    pub fn capture(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        let mut value = Self::default();
        for layer in doc.layers.values() {
            for instance in layer.filters.iter() {
                check()?;
                let descriptor = instance.descriptor();
                let entry = resolve(descriptor.id(), "params-v1")?;
                let (bytes, length) = descriptor.semantic_parameter_bytes();
                if entry.decode(&bytes[..length])? != descriptor {
                    return Err(invalid());
                }
                value.insert(entry.profile, entry.codec)?;
            }
        }
        if value.len > 256 {
            return Err(limit());
        }
        Ok(value)
    }
    /// Resolve the authenticated source without retaining its data. The caller
    /// already reserved CONTROL_BYTES; the scanner owns its own account slice.
    pub fn from_source(
        source: &crate::pcnd::CheckedContainer<'_>,
        limits: crate::pcnd::RequirementsLimits,
        scope: &paintcraft_core::types::resources::BudgetScope,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<ResolvedRequirements> {
        let mut controls = scope.account();
        controls.resize(std::mem::size_of::<ResolvedRequirements>() as u64)?;
        let mut value = Self::default();
        source.resolve_requirements_checked(limits, scope, check, |profile, codec| {
            value.insert(profile, codec)
        })?;
        if value.len > 256 {
            return Err(limit());
        }
        Ok(ResolvedRequirements {
            value,
            _account: controls,
        })
    }
    /// Source5's immutable source plus single-operation candidate requirement union.
    pub fn union(&self, other: &Self, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        let mut value = self.clone();
        for entry in other.entries[..other.len].iter().flatten() {
            check()?;
            value.insert(entry.profile, entry.codec)?;
        }
        Ok(value)
    }
    pub fn digests(&self, check: &dyn Fn() -> Result<()>) -> Result<([u8; 32], [u8; 32])> {
        let mut required = Sha256::new();
        let mut resolution = Sha256::new();
        required.update(b"paintcraft.worker-required/v1\0");
        required.update((self.len as u16).to_be_bytes());
        resolution.update(b"paintcraft.worker-resolution/v1\0");
        resolution.update(1u16.to_be_bytes());
        resolution.update((self.len as u16).to_be_bytes());
        for entry in self.entries[..self.len].iter().flatten() {
            check()?;
            for id in [entry.profile, entry.codec] {
                let length = (id.len() as u16).to_be_bytes();
                required.update(length);
                required.update(id.as_bytes());
                resolution.update(length);
                resolution.update(id.as_bytes());
            }
            resolution.update((entry.resource.len() as u16).to_be_bytes());
            resolution.update(entry.resource.as_bytes());
            resolution.update(entry.revision.to_be_bytes());
        }
        Ok((required.finalize().into(), resolution.finalize().into()))
    }
}
fn invalid() -> Error {
    Error::new(
        ErrorCode::CodecFailed,
        "Prepared registry typed decoder mismatch",
    )
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Prepared worker requirement inventory allowance",
    )
}

#[cfg(test)]
mod tests;
