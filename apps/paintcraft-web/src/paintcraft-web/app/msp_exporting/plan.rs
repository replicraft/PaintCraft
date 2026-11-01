//! Host-only adaptive owner; no fields added to the old Formats SourcePlan.
use super::*;
#[derive(Clone, Debug)]
pub(super) enum Plan {
    Legacy(wire::SourcePlan),
    Noise(wire::v8::NoiseSourcePlan),
}
impl From<wire::SourcePlan> for Plan {
    fn from(p: wire::SourcePlan) -> Self {
        Self::Legacy(p)
    }
}
impl Plan {
    #[cfg(target_arch = "wasm32")]
    pub(super) fn size(&self) -> (u32, u32) {
        match self {
            Self::Legacy(p) => p.size(),
            Self::Noise(p) => p.size(),
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) fn metadata_context(&self) -> paintcraft_formats::export::SourceMetadataContext {
        match self {
            Self::Legacy(p) => p.metadata_context(),
            Self::Noise(p) => p.metadata_context(),
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) fn mask_context(&self) -> paintcraft_formats::export::SourceMaskContext {
        match self {
            Self::Legacy(p) => p.mask_context(),
            Self::Noise(p) => p.mask_context(),
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) fn filter_digest(&self) -> [u8; 32] {
        match self {
            Self::Legacy(p) => p.filter_digest(),
            Self::Noise(p) => p.filter_digest(),
        }
    }
    pub(super) fn bake_intent(&self) -> u8 {
        match self {
            Self::Legacy(p) => p.bake_intent(),
            Self::Noise(p) => p.bake_intent(),
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) fn source_flags(&self) -> u8 {
        match self {
            Self::Legacy(p) => p.source_flags(),
            Self::Noise(p) => p.source_flags(),
        }
    }
    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn snapshot(&self, doc: &paintcraft_document::Document) -> Result<Vec<u8>> {
        match self {
            Self::Legacy(p) => p.snapshot(doc),
            Self::Noise(p) => p.snapshot(doc),
        }
    }
    pub(super) fn validate_policies(&self, doc: &paintcraft_document::Document) -> Result<()> {
        match self {
            Self::Legacy(p) => p.validate_policies(doc),
            Self::Noise(p) => p.validate_policies(doc),
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) fn spatial_context(&self, len: usize) -> Result<[u8; 64]> {
        match self {
            Self::Legacy(p) => p.spatial_context(len),
            Self::Noise(_) => Err(failed("MSP8 is not a legacy context")),
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) fn catalog_context(&self, len: usize) -> Result<[u8; 64]> {
        match self {
            Self::Legacy(p) => p.catalog_context(len),
            Self::Noise(_) => Err(failed("MSP8 is not a legacy context")),
        }
    }
    pub(super) fn capture(
        doc: &paintcraft_document::Document,
        version: Version,
        policies: paintcraft_formats::export::CopyPolicies,
    ) -> Result<Self> {
        if paintcraft_formats::export::prepared_capabilities::requires_scoped_noise(
            doc,
            &|| Ok(()),
        )? {
            let workload = wire::v8::default_workload()?;
            Ok(Self::Noise(
                wire::v8::NoiseSourcePlan::capture_with_policies_checked(
                    doc,
                    codec_version(version),
                    policies,
                    workload,
                    &|| Ok(()),
                )?,
            ))
        } else {
            if version != Version::RowRleV2 {
                return Err(stale());
            }
            Ok(Self::Legacy(wire::SourcePlan::capture_with_policies(
                doc, policies,
            )?))
        }
    }
    pub(super) fn version(&self) -> paintcraft_formats::msp::Version {
        match self {
            Self::Legacy(_) => paintcraft_formats::msp::Version::RowRleV2,
            Self::Noise(p) => p.version(),
        }
    }
    pub(super) fn protocol_version(&self) -> u16 {
        match self {
            Self::Legacy(p) => p.protocol_version(),
            Self::Noise(p) => p.protocol_version(),
        }
    }
}

pub(super) fn codec_version(version: Version) -> paintcraft_formats::msp::Version {
    match version {
        Version::UncompressedV1 => paintcraft_formats::msp::Version::UncompressedV1,
        Version::RowRleV2 => paintcraft_formats::msp::Version::RowRleV2,
    }
}
