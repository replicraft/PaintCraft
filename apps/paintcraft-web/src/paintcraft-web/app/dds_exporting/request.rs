//! A captured private request never changes protocol after source finalization.
use super::*;
use paintcraft_formats::codec_worker::Owner;
#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
#[derive(Clone, Debug)]
pub(super) enum Request {
    V4(wire::Header),
    V5(wire::v5::Header),
}
impl Request {
    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn capture(
        plan: &wire::SourcePlan,
        source: &[u8],
        owner: Owner,
        nonce: uuid::Uuid,
    ) -> Result<Self> {
        match plan.protocol_version() {
            4 => Ok(Self::V4(wire::Header::capture(plan, source, owner, nonce)?)),
            5 => Ok(Self::V5(wire::v5::Header::capture(
                plan, source, owner, nonce,
            )?)),
            _ => Err(failed("Unsupported captured export protocol")),
        }
    }
    #[cfg(test)]
    pub(super) fn encode(&self) -> Vec<u8> {
        match self {
            Self::V4(h) => h.encode().to_vec(),
            Self::V5(h) => h.encode().to_vec(),
        }
    }
    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn validate_echo(&self, bytes: &[u8]) -> Result<()> {
        let equal = match self {
            Self::V4(h) => wire::Header::decode(bytes)? == *h,
            Self::V5(h) => wire::v5::Header::decode(bytes)? == *h,
        };
        if !equal {
            return Err(failed("Request echo differs from captured source/options"));
        }
        Ok(())
    }
    pub(super) fn protocol_version(&self) -> u16 {
        match self {
            Self::V4(_) => 4,
            Self::V5(_) => 5,
        }
    }
    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn header_bytes(&self) -> usize {
        match self {
            Self::V4(_) => wire::REQUEST_HEADER,
            Self::V5(_) => wire::v5::REQUEST_HEADER,
        }
    }
    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn response_header(&self) -> usize {
        match self {
            Self::V4(_) => wire::RESPONSE_HEADER,
            Self::V5(_) => wire::v5::RESPONSE_HEADER,
        }
    }
    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn response_max(&self) -> usize {
        match self {
            Self::V4(_) => wire::RESPONSE_BYTES_MAX,
            Self::V5(_) => wire::v5::RESPONSE_BYTES_MAX,
        }
    }
    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn source_sha256(&self) -> [u8; 32] {
        match self {
            Self::V4(h) => h.source_sha256,
            Self::V5(h) => h.source_sha256,
        }
    }
    pub(super) fn owner(&self) -> Owner {
        match self {
            Self::V4(h) => h.owner,
            Self::V5(h) => h.owner,
        }
    }
    pub(super) fn metadata(&self) -> paintcraft_formats::export::SourceMetadataContext {
        match self {
            Self::V4(h) => h.metadata,
            Self::V5(h) => h.metadata,
        }
    }
    pub(super) fn settings(&self) -> [u8; 8] {
        match self {
            Self::V4(h) => h.settings,
            Self::V5(h) => h.settings,
        }
    }
    pub(super) fn size(&self) -> (u32, u32) {
        match self {
            Self::V4(h) => h.size,
            Self::V5(h) => h.size,
        }
    }
    pub(super) fn matches_plan(&self, plan: &wire::SourcePlan) -> bool {
        if self.protocol_version() != plan.protocol_version() {
            return false;
        }
        match self {
            Self::V4(h) => {
                h.metadata == plan.metadata_context()
                    && h.masks == plan.mask_context()
                    && h.filter_digest == plan.filter_digest()
                    && h.bake_intent == plan.bake_intent()
            }
            Self::V5(h) => {
                h.metadata == plan.metadata_context()
                    && h.masks == plan.mask_context()
                    && h.filter_digest == plan.filter_digest()
                    && h.bake_intent == plan.bake_intent()
                    && plan
                        .spatial_context(h.source_len as usize)
                        .is_ok_and(|actual| actual == h.spatial.encode())
            }
        }
    }
}
