//! Source-bound advanced Web Studio DDS preparation. Hosts admit the exact route.
//! Native codec protocols and the legacy default DDS writer are independent.
use super::*;
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
pub const PROTOCOL_VERSION: u16 = 1;
pub const PROFILE: &str = "web-dds-pcnd32mib-4mp-v1";
pub const PIXELS_MAX: u64 = 4_194_304;
pub const LAYERS_MAX: usize = 8;
pub const STAGED_LAYER_PIXELS_MAX: u64 = 8_388_608;
pub const RESIDENT_BYTES_MAX: u64 = 24 << 20;
pub const SOURCE_BYTES_MAX: usize = 32 << 20;
pub const ENCODED_BYTES_MAX: usize = (24 << 20) + 4096;
pub const PREVIEW_SIDE: u32 = 192;
pub const PREVIEW_BYTES_MAX: usize = 192 * 192 * 4;
pub const REQUEST_HEADER: usize = 128;
pub const RESPONSE_HEADER: usize = 288;
pub const RESPONSE_BYTES_MAX: usize =
    RESPONSE_HEADER + ENCODED_BYTES_MAX + PREVIEW_BYTES_MAX + 4096;
/// Declares known copies and the entire 256MiB Worker linear-memory ceiling.
/// This is logical payload admission, not aggregate browser/GPU/allocator RSS.
pub const RESERVATION_BYTES: u64 = 640 << 20;
pub const LOSSES: [&str; 5] = [
    "flatten",
    "hidden-rgb",
    BLOCK_COMPRESSION_LOSS,
    MIP_HIDDEN_RGB_LOSS,
    export::dds::LINEAR_QUANTIZATION_LOSS,
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
#[derive(Clone, Debug)]
pub struct SourcePlan {
    pub(crate) document: Uuid,
    pub(crate) state: Uuid,
    pub(crate) size: (u32, u32),
    pub(crate) settings: [u8; 8],
    pub(crate) source_flags: u8,
}
impl SourcePlan {
    /// Inspect bounded immutable document data before reserving/serializing a
    /// snapshot. The host must reserve RESERVATION_BYTES before snapshot().
    pub fn capture(doc: &Document, options: TextureOptions) -> Result<Self> {
        if doc.canvas.0 == 0
            || doc.canvas.1 == 0
            || doc.canvas.0 > 65536
            || doc.canvas.1 > 65536
            || u64::from(doc.canvas.0) * u64::from(doc.canvas.1) > PIXELS_MAX
            || doc.layers.len() > LAYERS_MAX
            || doc.roots.len() != doc.layers.len()
        {
            return Err(resource("Browser DDS document geometry/layer profile"));
        }
        let mut pixels = 0u64;
        let mut resident = 0u64;
        let mut metadata = doc.title.as_ref().map_or(0, |s| s.len() as u64);
        for layer in doc.layers.values() {
            pixels = pixels
                .checked_add(u64::from(layer.raster.width()) * u64::from(layer.raster.height()))
                .ok_or_else(|| resource("Browser DDS layer extent overflow"))?;
            resident = resident
                .checked_add(
                    (layer.raster.resident_tiles() as u64)
                        .checked_mul(paintcraft_raster::RasterLayer::TILE_BYTES as u64)
                        .ok_or_else(|| resource("Browser DDS tile extent overflow"))?,
                )
                .ok_or_else(|| resource("Browser DDS resident overflow"))?;
            metadata = metadata
                .checked_add(layer.name.len() as u64 + 256)
                .ok_or_else(|| resource("Browser DDS metadata overflow"))?;
        }
        if pixels > STAGED_LAYER_PIXELS_MAX || resident > RESIDENT_BYTES_MAX || metadata > 65536 {
            return Err(resource("Browser DDS source workspace profile"));
        }
        let settings = encode_settings(options, doc.canvas)?;
        let source_flags = u8::from(doc.roots.len() > 1)
            | u8::from(doc.layers.values().any(|l| l.raster.has_hidden_rgb())) << 1;
        Ok(Self {
            document: doc.document_id,
            state: doc.state_id,
            size: doc.canvas,
            settings,
            source_flags,
        })
    }
    pub fn source_flags(&self) -> u8 {
        self.source_flags
    }
    pub fn settings(&self) -> [u8; 8] {
        self.settings
    }
    pub fn size(&self) -> (u32, u32) {
        self.size
    }
    /// Raw, uncompressed serialization is intentionally bounded. This is not
    /// the DDS encoder. No compact/tile compression runs on the UI thread.
    pub fn snapshot(&self, doc: &Document) -> Result<Vec<u8>> {
        let fresh = Self::capture(doc, decode_settings(self.settings, self.size)?)?;
        if fresh.document != self.document
            || fresh.state != self.state
            || fresh.size != self.size
            || fresh.source_flags != self.source_flags
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Browser DDS snapshot changed",
            ));
        }
        struct Sink(Vec<u8>);
        impl std::io::Write for Sink {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self
                    .0
                    .len()
                    .checked_add(bytes.len())
                    .is_none_or(|n| n > SOURCE_BYTES_MAX)
                {
                    return Err(std::io::Error::other("snapshot allowance"));
                }
                self.0.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(SOURCE_BYTES_MAX)
            .map_err(|_| resource("Browser DDS snapshot allocation"))?;
        let mut sink = Sink(bytes);
        pcnd::write_to(doc, &mut sink).map_err(|_| resource("Browser DDS snapshot allowance"))?;
        Ok(sink.0)
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
