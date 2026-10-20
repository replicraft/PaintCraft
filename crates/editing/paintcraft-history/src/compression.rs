//! Independent, fixed-size blocks; no dictionaries, frame streams or worker pool.
use crate::archive::{corrupt, limit};
use paintcraft_core::types::Result;
use paintcraft_raster::RasterLayer;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Compression {
    #[default]
    None,
    Lz4,
    Zstd1,
    Zstd3,
}

pub(crate) struct Encoder {
    policy: Compression,
    scratch: Vec<u8>,
    zstd: Option<zstd::bulk::Compressor<'static>>,
    initialized: bool,
    pub workspace_peak: usize,
}
impl Encoder {
    pub fn new(policy: Compression) -> Self {
        Self {
            policy,
            scratch: Vec::new(),
            zstd: None,
            initialized: false,
            workspace_peak: 0,
        }
    }
    pub fn policy(&self) -> Compression {
        self.policy
    }
    pub fn scratch_bytes(&self) -> usize {
        self.scratch.capacity()
    }
    pub fn encode(
        &mut self,
        source: &[u8],
        smaller_than: usize,
        budget: &mut usize,
        check: &impl Fn() -> Result<()>,
    ) -> Result<Option<Box<[u8]>>> {
        if self.policy == Compression::None {
            return Ok(None);
        }
        check()?;
        if source.len() != RasterLayer::TILE_BYTES {
            return Err(corrupt());
        }
        if !self.initialized {
            // Reserve conservative codec headroom before creating native state.
            // The observed Zstd context size is checked after each bounded block;
            // this reservation is not an allocator-enforced native memory cap.
            let workspace = if self.policy == Compression::Lz4 {
                16 << 10
            } else {
                8 << 20
            };
            let bound = match self.policy {
                Compression::Lz4 => {
                    lz4_flex::block::get_maximum_output_size(RasterLayer::TILE_BYTES)
                }
                _ => zstd::compress_bound(RasterLayer::TILE_BYTES),
            };
            if workspace + bound > *budget {
                return Err(limit("history codec staging admission exceeded"));
            }
            self.scratch
                .try_reserve_exact(bound)
                .map_err(|_| limit("cannot allocate history codec buffer"))?;
            if workspace + self.scratch.capacity() > *budget {
                return Err(limit("history codec buffer capacity exceeded"));
            }
            *budget -= workspace + self.scratch.capacity();
            self.scratch.resize(bound, 0);
            if self.policy != Compression::Lz4 {
                let level = if self.policy == Compression::Zstd1 {
                    1
                } else {
                    3
                };
                let mut encoder = zstd::bulk::Compressor::new(level)
                    .map_err(|_| limit("cannot create history encoder"))?;
                encoder
                    .set_parameter(zstd::zstd_safe::CParameter::WindowLog(18))
                    .map_err(|_| limit("cannot bound history encoder window"))?;
                self.zstd = Some(encoder);
            } else {
                self.workspace_peak = workspace;
            }
            self.initialized = true;
        }
        let size = if let Some(encoder) = &mut self.zstd {
            let size = encoder
                .compress_to_buffer(source, self.scratch.as_mut_slice())
                .map_err(|_| limit("history compression failed"))?;
            self.workspace_peak = self.workspace_peak.max(encoder.context_mut().sizeof());
            if self.workspace_peak > 8 << 20 {
                return Err(limit("history encoder exceeded reserved headroom"));
            }
            size
        } else {
            lz4_flex::block::compress_into(source, &mut self.scratch)
                .map_err(|_| limit("history compression failed"))?
        };
        check()?;
        if size >= smaller_than {
            return Ok(None);
        }
        if size > *budget {
            return Err(limit("history encoded payload admission exceeded"));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|_| limit("cannot allocate encoded history payload"))?;
        if bytes.capacity() > *budget {
            return Err(limit("history encoded capacity exceeded"));
        }
        *budget -= bytes.capacity();
        bytes.extend_from_slice(&self.scratch[..size]);
        Ok(Some(bytes.into_boxed_slice()))
    }
}

pub(crate) fn decode(codec: Compression, bytes: &[u8], out: &mut [u8]) -> Result<()> {
    if out.len() != RasterLayer::TILE_BYTES || bytes.len() >= out.len() {
        return Err(corrupt());
    }
    let size = match codec {
        Compression::Lz4 => lz4_flex::block::decompress_into(bytes, out).map_err(|_| corrupt())?,
        Compression::Zstd1 | Compression::Zstd3 => {
            if zstd::zstd_safe::get_frame_content_size(bytes).map_err(|_| corrupt())?
                != Some(out.len() as u64)
                || zstd::zstd_safe::find_frame_compressed_size(bytes).map_err(|_| corrupt())?
                    != bytes.len()
            {
                return Err(corrupt());
            }
            let mut decoder = zstd::bulk::Decompressor::new()
                .map_err(|_| limit("cannot create history decoder"))?;
            decoder
                .set_parameter(zstd::zstd_safe::DParameter::WindowLogMax(18))
                .map_err(|_| corrupt())?;
            decoder
                .decompress_to_buffer(bytes, out)
                .map_err(|_| corrupt())?
        }
        Compression::None => return Err(corrupt()),
    };
    if size != out.len() {
        return Err(corrupt());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
