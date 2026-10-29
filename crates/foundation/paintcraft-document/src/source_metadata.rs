//! Immutable original-source facts. Retaining a profile never activates it.
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{BudgetLease, SharedBudget},
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
mod icc;
mod reader;
mod wire;
pub use icc::{IccHeader, inspect_icc};
pub const BYTES_MAX: u64 = 8 << 20;
pub const WIRE_HEADER_BYTES: usize = 192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SourceFormat {
    Png = 1,
    Jpeg,
    Gif,
    Bmp,
    Tiff,
    Webp,
    Jxl,
    Avif,
    Heif,
    Jxr,
    Other,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rational {
    pub numerator: i64,
    pub denominator: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ResolutionUnit {
    Unspecified = 0,
    Inch,
    Centimetre,
    Metre,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub horizontal: Rational,
    pub vertical: Rational,
    pub unit: ResolutionUnit,
    /// Original container declaration, including unknown or nonphysical codes.
    pub raw_unit: Option<u32>,
}
impl Resolution {
    /// Original nonphysical BMP declarations remain provenance, never DPI.
    pub fn physical(&self) -> Option<Self> {
        (self.horizontal.numerator > 0
            && self.vertical.numerator > 0
            && self.horizontal.denominator > 0
            && self.vertical.denominator > 0
            && self.unit != ResolutionUnit::Unspecified)
            .then_some(*self)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum OrientationPolicy {
    Unmodified = 0,
    ExifMaterializedV1 = 1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceDescriptor {
    pub format: SourceFormat,
    pub source_model: SourceColorModel,
    pub source_sha256: [u8; 32],
    pub source_bytes: u64,
    pub selection: Option<u32>,
    pub source_dimensions: (u32, u32),
    pub output_dimensions: (u32, u32),
    pub orientation: Option<u8>,
    pub orientation_policy: OrientationPolicy,
    pub resolution: Option<Resolution>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkingColor {
    UnmanagedRgba8,
}
/// Original sample interpretation, distinct from both ICC header declarations
/// and the editor's unmanaged RGBA8 working samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SourceColorModel {
    Unknown = 0,
    Gray,
    Rgb,
    Cmyk,
    YCbCr,
    Ycck,
    Indexed,
}
#[derive(Clone)]
pub struct SourceMetadata(Arc<Inner>);
struct Inner {
    descriptor: SourceDescriptor,
    profile: Vec<u8>,
    profile_header: Option<IccHeader>,
    profile_sha256: [u8; 32],
    digest: [u8; 32],
    charged_bytes: u64,
    _lease: BudgetLease,
    _capacity_lease: Option<BudgetLease>,
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
impl SourceDescriptor {
    fn validate(&self) -> Result<()> {
        for (w, h) in [self.source_dimensions, self.output_dimensions] {
            if w == 0 || h == 0 || w > crate::CANVAS_SIDE_MAX || h > crate::CANVAS_SIDE_MAX {
                return Err(invalid("Source metadata geometry"));
            }
        }
        if self.source_bytes == 0 || self.orientation.is_some_and(|o| !(1..=8).contains(&o)) {
            return Err(invalid("Source metadata origin/orientation"));
        }
        if self.orientation_policy == OrientationPolicy::ExifMaterializedV1
            && self.orientation.is_none()
        {
            return Err(invalid(
                "Materialized source orientation is missing its original tag",
            ));
        }
        let expected = if self.orientation_policy == OrientationPolicy::ExifMaterializedV1
            && self.orientation.is_some_and(|o| o >= 5)
        {
            (self.source_dimensions.1, self.source_dimensions.0)
        } else {
            self.source_dimensions
        };
        if expected != self.output_dimensions {
            return Err(invalid("Source metadata materialized geometry"));
        }
        if self
            .resolution
            .is_some_and(|r| r.horizontal.denominator == 0 || r.vertical.denominator == 0)
        {
            return Err(invalid("Source resolution denominator"));
        }
        Ok(())
    }
}
impl SourceMetadata {
    /// Capture exact ICC bytes under an ownership lease. This validates only
    /// ICC container/header/tag bounds, not transform suitability or CMS output.
    pub fn capture(
        descriptor: SourceDescriptor,
        profile: Option<&[u8]>,
        limit: u64,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        check()?;
        descriptor.validate()?;
        let bytes = profile.map_or(0, |p| p.len() as u64);
        let total = bytes
            .checked_add(WIRE_HEADER_BYTES as u64)
            .filter(|n| *n <= limit.min(BYTES_MAX))
            .ok_or_else(|| resource("Source metadata allowance"))?;
        let charge = total
            .checked_add(std::mem::size_of::<Inner>() as u64 + 32)
            .ok_or_else(|| resource("Source metadata storage extent"))?;
        let lease = budget.reserve(charge)?;
        let header = profile.map(|p| inspect_icc(p, check)).transpose()?;
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(bytes as usize)
            .map_err(|_| resource("Cannot allocate retained ICC"))?;
        let excess = retained.capacity() as u64 - bytes;
        let capacity_lease = (excess != 0).then(|| budget.reserve(excess)).transpose()?;
        let mut hash = Sha256::new();
        if let Some(profile) = profile {
            for chunk in profile.chunks(65536) {
                check()?;
                retained.extend_from_slice(chunk);
                hash.update(chunk);
            }
        }
        let profile_sha256 = if profile.is_some() {
            hash.finalize().into()
        } else {
            [0; 32]
        };
        let inner = Inner {
            descriptor,
            profile: retained,
            profile_header: header,
            profile_sha256,
            digest: [0; 32],
            charged_bytes: charge + excess,
            _lease: lease,
            _capacity_lease: capacity_lease,
        };
        reader::finish(inner, check)
    }
    pub fn descriptor(&self) -> &SourceDescriptor {
        &self.0.descriptor
    }
    pub fn profile(&self) -> Option<&[u8]> {
        self.0
            .profile_header
            .as_ref()
            .map(|_| self.0.profile.as_slice())
    }
    /// Structurally validated original header declarations. Curves/LUTs and
    /// transform suitability are not interpreted or qualified by retention.
    pub fn profile_header(&self) -> Option<&IccHeader> {
        self.0.profile_header.as_ref()
    }
    pub fn digest(&self) -> [u8; 32] {
        self.0.digest
    }
    pub fn working_color(&self) -> WorkingColor {
        WorkingColor::UnmanagedRgba8
    }
    /// Unique conservative retention charge, including fixed descriptors and
    /// actual retained vector capacity. This is not allocator/RSS interception.
    pub fn storage_usage(&self) -> (usize, u64) {
        (Arc::as_ptr(&self.0) as usize, self.0.charged_bytes)
    }
    pub fn wire_header(&self) -> [u8; WIRE_HEADER_BYTES] {
        wire::header(&self.0)
    }
    pub fn from_wire(
        header: &[u8],
        profile: &[u8],
        limit: u64,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        wire::read(header, profile, limit, budget, check)
    }
}
impl PartialEq for SourceMetadata {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || self.0.descriptor == other.0.descriptor
                && self.0.profile == other.0.profile
                && self.0.profile_header == other.0.profile_header
    }
}
impl Eq for SourceMetadata {}
impl std::fmt::Debug for SourceMetadata {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceMetadata")
            .field("descriptor", &self.0.descriptor)
            .field("structural_icc_header", &self.0.profile_header)
            .field("profile_bytes", &self.0.profile.len())
            .field("profile_sha256", &crate::hex(&self.0.profile_sha256))
            .field("digest", &crate::hex(&self.0.digest))
            .finish()
    }
}

#[cfg(test)]
mod tests;
