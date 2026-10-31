//! Source-bound advanced Web Studio DDS preparation. Hosts admit the exact route.
//! Native codec protocols and the legacy default DDS writer are independent.
use super::*;
use crate::export::web_snapshot::{Limits, Plan as SnapshotPlan};
use crate::{codec_worker::Owner, export, pcnd};
use paintcraft_document::Document;
use sha2::{Digest, Sha256};
use uuid::Uuid;
mod catalog;
mod programs;
mod request;
mod response;
pub mod v5;
pub mod v6;
pub mod v7;
mod worker;
pub use request::Header;
pub use response::{Frame, decode_response};
pub use worker::execute;
pub const EXPORT_VARIANT: &str = "web-dds-texture-worker-v4";
pub const PROTOCOL_VERSION: u16 = 4;
pub const PROFILE: &str = "web-dds-pcnd32mib-4mp-v4";
pub const PIXELS_MAX: u64 = 4_194_304;
pub const LAYERS_MAX: usize = 8;
pub const STAGED_LAYER_PIXELS_MAX: u64 = 8_388_608;
pub const RESIDENT_BYTES_MAX: u64 = 24 << 20;
pub const SOURCE_BYTES_MAX: usize = 32 << 20;
pub const ENCODED_BYTES_MAX: usize = (24 << 20) + 4096;
pub const PREVIEW_SIDE: u32 = 192;
pub const PREVIEW_BYTES_MAX: usize = 192 * 192 * 4;
pub const REQUEST_HEADER: usize = 256;
pub const RESPONSE_HEADER: usize = 416;
pub const RESPONSE_BYTES_MAX: usize =
    RESPONSE_HEADER + ENCODED_BYTES_MAX + PREVIEW_BYTES_MAX + 4096;
/// Declares known copies and the entire 256MiB Worker linear-memory ceiling.
/// This is logical payload admission, not aggregate browser/GPU/allocator RSS.
pub const RESERVATION_BYTES: u64 = 640 << 20;
/// Existing isolated preparation workspace ceiling, including retained source facts.
pub const WORKER_PAYLOAD_BYTES_MAX: u64 = 192 << 20;
pub const LOSSES: [&str; 7] = [
    "flatten",
    "hidden-rgb",
    BLOCK_COMPRESSION_LOSS,
    MIP_HIDDEN_RGB_LOSS,
    export::dds::LINEAR_QUANTIZATION_LOSS,
    export::metadata_policy::OMITTED_REASON,
    export::filter_bake::BAKED_REASON,
];
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
fn failed(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
pub(crate) const SOURCE_LIMITS: Limits = Limits {
    label: "Browser DDS",
    side: 65536,
    pixels: PIXELS_MAX,
    layers: LAYERS_MAX,
    layer_pixels: STAGED_LAYER_PIXELS_MAX,
    resident: RESIDENT_BYTES_MAX,
    metadata: 65536,
    snapshot: SOURCE_BYTES_MAX,
};
#[derive(Clone, Debug)]
pub struct SourcePlan {
    source: SnapshotPlan,
    spatial: export::SourceSpatialContext,
    catalog: Option<export::SourceCatalogContext>,
    programs: Option<export::SourceProgramContext>,
    pub(crate) settings: [u8; 8],
}
impl SourcePlan {
    /// Inspect immutable source before service admission/snapshot allocation.
    pub fn capture(doc: &Document, options: TextureOptions) -> Result<Self> {
        Self::capture_with_metadata_policy(doc, options, export::metadata_policy::Policy::default())
    }
    pub fn capture_with_metadata_policy(
        doc: &Document,
        options: TextureOptions,
        policy: export::metadata_policy::Policy,
    ) -> Result<Self> {
        Self::capture_with_policies(
            doc,
            options,
            export::CopyPolicies {
                source_metadata: policy,
                ..Default::default()
            },
        )
    }
    pub fn capture_with_policies(
        doc: &Document,
        options: TextureOptions,
        policies: export::CopyPolicies,
    ) -> Result<Self> {
        Self::capture_with_policies_checked(doc, options, policies, &|| Ok(()))
    }
    pub fn capture_with_policies_checked(
        doc: &Document,
        options: TextureOptions,
        policies: export::CopyPolicies,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let source = SnapshotPlan::capture_with_policies(
            doc,
            SOURCE_LIMITS,
            policies.source_metadata,
            policies.filter_bake,
            export::metadata_policy::Target::new(
                crate::registry::Family::Dds,
                crate::registry::Operation::ExportFlat,
            ),
        )?;
        let settings = encode_settings(options, doc.canvas)?;
        let mut value = Self {
            source,
            settings,
            spatial: export::SourceSpatialContext::default(),
            catalog: None,
            programs: None,
        };
        if export::prepared_capabilities::requires_generic(doc, check)? {
            value.capture_programs(doc, check)?;
            return Ok(value);
        }
        if doc
            .layers
            .values()
            .any(|l| l.filters.iter().any(|f| f.descriptor().is_catalog_only()))
        {
            value.capture_catalog(doc, check)?;
            return Ok(value);
        }
        value.spatial = export::SourceSpatialContext::capture(doc, 0, check)?;
        if value.spatial.has_spatial() {
            value.spatial = value
                .spatial
                .with_base_payload(value.payload_base(doc, 0)?)?;
            value.spatial.validate(WORKER_PAYLOAD_BYTES_MAX)?;
        }
        Ok(value)
    }
    pub fn protocol_version(&self) -> u16 {
        if self.programs.is_some() {
            7
        } else if self.catalog.is_some() {
            6
        } else if self.spatial.has_spatial() {
            5
        } else {
            4
        }
    }
    /// Finalize private ABI-local resource facts only after frozen PCND bytes exist.
    pub fn spatial_context(&self, source_len: usize) -> Result<[u8; 64]> {
        if self.programs.is_some() {
            return Ok(self.finalized_program_region(source_len)?.encode_programs());
        }
        if self.catalog.is_some() {
            return Ok(self.finalized_catalog_region(source_len)?.encode_catalog());
        }
        if source_len == 0 || source_len > SOURCE_BYTES_MAX {
            return Err(resource("Spatial source extent"));
        }
        if !self.spatial.has_spatial() {
            return Ok([0; 64]);
        }
        Ok(self
            .spatial
            .finalize_source(source_len, WORKER_PAYLOAD_BYTES_MAX)?
            .encode())
    }
    pub(crate) fn payload_base(&self, doc: &Document, source_bytes: usize) -> Result<u64> {
        let resident = doc.layers.values().try_fold(0u64, |n, l| {
            n.checked_add(
                l.raster.resident_tiles() as u64
                    * paintcraft_raster::RasterLayer::TILE_BYTES as u64,
            )
            .ok_or_else(|| resource("Spatial resident extent overflow"))
        })?;
        export::dds::preparation_reservation_bytes(&TexturePlan::new(
            doc.canvas.0,
            doc.canvas.1,
            decode_settings(self.settings, doc.canvas)?,
        )?)?
        .checked_add(resident)
        .and_then(|n| n.checked_add(source_bytes as u64))
        .and_then(|n| n.checked_add(self.metadata_context().retained_charge()))
        .and_then(|n| n.checked_add(self.mask_context().decode_bytes()))
        .and_then(|n| n.checked_add(self.source.filter_inventory().filter_storage_bytes()))
        .and_then(|n| {
            n.checked_add(
                (8 << 20) + (1 << 20) + ENCODED_BYTES_MAX as u64 + 2 * PREVIEW_BYTES_MAX as u64,
            )
        })
        .ok_or_else(|| resource("Spatial export payload overflow"))
    }
    pub fn mask_context(&self) -> export::SourceMaskContext {
        self.source.mask_context()
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
    pub fn settings(&self) -> [u8; 8] {
        self.settings
    }
    pub fn size(&self) -> (u32, u32) {
        self.source.size()
    }
    /// Caller retains RESERVATION_BYTES before raw serialization, never encoding.
    pub fn snapshot(&self, doc: &Document) -> Result<Vec<u8>> {
        let options = decode_settings(self.settings, self.size())?;
        self.source
            .snapshot(doc, |size| encode_settings(options, size).map(|_| ()))
    }
}
pub fn encode_settings(mut options: TextureOptions, size: (u32, u32)) -> Result<[u8; 8]> {
    let plan = TexturePlan::new(size.0, size.1, options)?;
    let levels = plan.levels().len() as u8;
    if levels == 1 {
        options.filter = MipFilter::Nearest;
    }
    if options.encoding == Encoding::Rgba8 {
        options.quality = TextureQuality::Normal;
    }
    let encoding = match options.encoding {
        Encoding::Rgba8 => 0,
        Encoding::Bc1 => 1,
        Encoding::Bc2 => 2,
        Encoding::Bc3 => 3,
        _ => return Err(invalid("Browser DDS encoding")),
    };
    if options.alpha
        != if encoding == 1 {
            SourceAlpha::Opaque
        } else {
            SourceAlpha::Straight
        }
    {
        return Err(invalid("Browser DDS alpha convention"));
    }
    Ok([
        encoding,
        match options.transfer {
            Transfer::Srgb => 0,
            Transfer::Linear => 1,
            _ => return Err(invalid("Browser DDS transfer")),
        },
        u8::from(encoding == 1),
        levels,
        u8::from(options.filter == MipFilter::BoxStored),
        u8::from(options.quality == TextureQuality::High),
        0,
        0,
    ])
}
pub fn decode_settings(bytes: [u8; 8], size: (u32, u32)) -> Result<TextureOptions> {
    if bytes[0] > 3
        || bytes[1] > 1
        || bytes[2] > 1
        || bytes[3] == 0
        || bytes[3] > 17
        || bytes[4] > 1
        || bytes[5] > 1
        || bytes[6..] != [0, 0]
    {
        return Err(invalid("Browser DDS settings fields"));
    }
    let options = TextureOptions {
        encoding: [Encoding::Rgba8, Encoding::Bc1, Encoding::Bc2, Encoding::Bc3]
            [usize::from(bytes[0])],
        transfer: if bytes[1] == 0 {
            Transfer::Srgb
        } else {
            Transfer::Linear
        },
        alpha: if bytes[2] == 0 {
            SourceAlpha::Straight
        } else {
            SourceAlpha::Opaque
        },
        mips: MipCount::Levels(bytes[3]),
        filter: if bytes[4] == 0 {
            MipFilter::Nearest
        } else {
            MipFilter::BoxStored
        },
        quality: if bytes[5] == 0 {
            TextureQuality::Normal
        } else {
            TextureQuality::High
        },
    };
    if encode_settings(options, size)? != bytes {
        return Err(invalid("Noncanonical browser DDS settings"));
    }
    Ok(options)
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
