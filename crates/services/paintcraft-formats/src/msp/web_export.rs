//! Bounded opt-in browser MSP2 preparation for the exact Web Studio export route.
//! The legacy MSP1 route and native codec protocols are independent.
use super::{Error, ErrorCode, Result, RgbaLayout, Version};
use crate::export::web_snapshot::{Limits, Plan as SnapshotPlan};
use crate::{codec_worker::Owner, export, pcnd};
use paintcraft_document::Document;
use sha2::{Digest, Sha256};
use uuid::Uuid;
mod request;
mod response;
mod worker;
pub use request::Header;
pub use response::{Frame, decode_response};
pub use worker::execute;
/// Exact opt-in Web Studio route; registry admission is qualified separately.
pub const EXPORT_VARIANT: &str = "web-msp2-row-rle-worker-v3";
pub const PROTOCOL_VERSION: u16 = 3;
pub const PROFILE: &str = "web-msp2-pcnd32mib-4mp-v3";
pub const PIXELS_MAX: u64 = 4_194_304;
pub const LAYERS_MAX: usize = 8;
pub const STAGED_LAYER_PIXELS_MAX: u64 = 8_388_608;
pub const RESIDENT_BYTES_MAX: u64 = 64 << 20;
pub const SOURCE_BYTES_MAX: usize = (32 << 20) + (128 << 10);
pub const ENCODED_BYTES_MAX: usize = 1 << 20;
pub const PREVIEW_SIDE: u32 = 192;
pub const PREVIEW_BYTES_MAX: usize = 192 * 192 * 4;
pub const REQUEST_HEADER: usize = 224;
pub const RESPONSE_HEADER: usize = 384;
pub const RESPONSE_BYTES_MAX: usize =
    RESPONSE_HEADER + ENCODED_BYTES_MAX + PREVIEW_BYTES_MAX + 4096;
/// Logical copies plus the full 256MiB Worker ceiling; not browser/GPU/RSS.
pub const RESERVATION_BYTES: u64 = 512 << 20;
pub const WORKER_PAYLOAD_BYTES_MAX: u64 = 160 << 20;
pub const LOSSES: [&str; 4] = [
    "flatten",
    "hidden-rgb",
    export::metadata_policy::OMITTED_REASON,
    export::filter_bake::BAKED_REASON,
];
pub(crate) const SOURCE_LIMITS: Limits = Limits {
    label: "Browser MSP2",
    side: 65535,
    pixels: PIXELS_MAX,
    layers: LAYERS_MAX,
    layer_pixels: STAGED_LAYER_PIXELS_MAX,
    resident: RESIDENT_BYTES_MAX,
    metadata: 65536,
    snapshot: SOURCE_BYTES_MAX,
};
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
fn failed(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
#[derive(Clone, Debug)]
pub struct SourcePlan {
    source: SnapshotPlan,
}
impl SourcePlan {
    pub fn capture(doc: &Document) -> Result<Self> {
        Self::capture_with_metadata_policy(doc, export::metadata_policy::Policy::default())
    }
    pub fn capture_with_metadata_policy(
        doc: &Document,
        policy: export::metadata_policy::Policy,
    ) -> Result<Self> {
        Self::capture_with_policies(
            doc,
            export::CopyPolicies {
                source_metadata: policy,
                ..Default::default()
            },
        )
    }
    pub fn capture_with_policies(doc: &Document, policies: export::CopyPolicies) -> Result<Self> {
        let source = SnapshotPlan::capture_with_policies(
            doc,
            SOURCE_LIMITS,
            policies.source_metadata,
            policies.filter_bake,
            export::metadata_policy::Target::new(
                crate::registry::Family::Msp,
                crate::registry::Operation::ExportFlat,
            ),
        )?;
        RgbaLayout::new(Version::RowRleV2, doc.canvas.0, doc.canvas.1)?;
        Ok(Self { source })
    }
    pub fn size(&self) -> (u32, u32) {
        self.source.size()
    }
    pub fn metadata_context(&self) -> export::SourceMetadataContext {
        self.source.metadata_context()
    }
    pub fn validate_metadata_policy(&self, doc: &Document) -> Result<()> {
        self.validate_policies(doc)
    }
    pub fn validate_policies(&self, doc: &Document) -> Result<()> {
        self.source.validate_policy(doc)
    }
    pub fn bake_intent(&self) -> u8 {
        self.source.bake_intent()
    }
    pub fn filter_digest(&self) -> [u8; 32] {
        self.source.filter_inventory().digest()
    }
    pub fn source_flags(&self) -> u8 {
        self.source.flags()
    }
    /// Caller must retain RESERVATION_BYTES before raw serialization.
    pub fn snapshot(&self, doc: &Document) -> Result<Vec<u8>> {
        self.source.snapshot(doc, |size| {
            RgbaLayout::new(Version::RowRleV2, size.0, size.1).map(|_| ())
        })
    }
}
pub fn preview_size(size: (u32, u32)) -> (u32, u32) {
    let side = size.0.max(size.1);
    if side <= PREVIEW_SIDE {
        size
    } else {
        (
            (u64::from(size.0) * u64::from(PREVIEW_SIDE) / u64::from(side)).max(1) as u32,
            (u64::from(size.1) * u64::from(PREVIEW_SIDE) / u64::from(side)).max(1) as u32,
        )
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod filter_tests;
