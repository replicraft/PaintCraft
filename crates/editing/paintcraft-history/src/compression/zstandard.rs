//! Native Zstandard codec. Browser sessions use raw/LZ4 history; importing native
//! history archives is not a browser feature and never silently changes codecs.
use paintcraft_core::types::Result;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use crate::archive::{corrupt, limit};

    pub struct Encoder(zstd::bulk::Compressor<'static>);
    impl Encoder {
        pub fn new(level: i32) -> Result<Self> {
            let mut encoder = zstd::bulk::Compressor::new(level)
                .map_err(|_| limit("cannot create history encoder"))?;
            encoder
                .set_parameter(zstd::zstd_safe::CParameter::WindowLog(18))
                .map_err(|_| limit("cannot bound history encoder window"))?;
            Ok(Self(encoder))
        }
        pub fn encode(&mut self, source: &[u8], out: &mut [u8]) -> Result<usize> {
            self.0
                .compress_to_buffer(source, out)
                .map_err(|_| limit("history compression failed"))
        }
        pub fn workspace(&mut self) -> usize {
            self.0.context_mut().sizeof()
        }
    }
    pub fn bound() -> Result<usize> {
        Ok(zstd::compress_bound(
            paintcraft_raster::RasterLayer::TILE_BYTES,
        ))
    }
    pub fn decode(bytes: &[u8], out: &mut [u8]) -> Result<usize> {
        if zstd::zstd_safe::get_frame_content_size(bytes).map_err(|_| corrupt())?
            != Some(out.len() as u64)
            || zstd::zstd_safe::find_frame_compressed_size(bytes).map_err(|_| corrupt())?
                != bytes.len()
        {
            return Err(corrupt());
        }
        let mut decoder =
            zstd::bulk::Decompressor::new().map_err(|_| limit("cannot create history decoder"))?;
        decoder
            .set_parameter(zstd::zstd_safe::DParameter::WindowLogMax(18))
            .map_err(|_| corrupt())?;
        decoder
            .decompress_to_buffer(bytes, out)
            .map_err(|_| corrupt())
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use paintcraft_core::types::{Error, ErrorCode};
    fn unavailable<T>() -> Result<T> {
        Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Zstandard history is unavailable in this browser build",
        ))
    }
    pub struct Encoder;
    impl Encoder {
        pub fn new(_level: i32) -> Result<Self> {
            unavailable()
        }
        pub fn encode(&mut self, _source: &[u8], _out: &mut [u8]) -> Result<usize> {
            unavailable()
        }
        pub fn workspace(&mut self) -> usize {
            0
        }
    }
    pub fn bound() -> Result<usize> {
        unavailable()
    }
    pub fn decode(_bytes: &[u8], _out: &mut [u8]) -> Result<usize> {
        unavailable()
    }
}
#[cfg(target_arch = "wasm32")]
pub(super) use browser::*;
#[cfg(not(target_arch = "wasm32"))]
pub(super) use native::*;
