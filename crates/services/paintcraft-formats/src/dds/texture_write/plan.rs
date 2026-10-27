use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MipFilter {
    /// Sample the source pixel containing each destination pixel's center.
    Nearest,
    /// Area weights in stored sample space; straight RGB is alpha weighted.
    BoxStored,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MipCount {
    Levels(u8),
    Full,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureQuality {
    Normal,
    High,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureOptions {
    pub encoding: Encoding,
    pub transfer: Transfer,
    pub alpha: SourceAlpha,
    pub mips: MipCount,
    pub filter: MipFilter,
    pub quality: TextureQuality,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TexturePlan {
    pub(super) options: TextureOptions,
    pub(super) levels: Vec<MipLevel>,
    source_bytes: u64,
    encoded_bytes: u64,
    mip_bytes: u64,
    codec_bytes: u64,
    filter_visits: u64,
}
impl TexturePlan {
    pub fn new(width: u32, height: u32, options: TextureOptions) -> Result<Self> {
        if width == 0 || height == 0 || width > 65_536 || height > 65_536 {
            return Err(resource(
                "DDS texture dimensions exceed the admitted extent",
            ));
        }
        let pixels = u64::from(width) * u64::from(height);
        if pixels > PIXELS_MAX {
            return Err(resource("DDS texture exceeds the pixel allowance"));
        }
        if !matches!(
            options.encoding,
            Encoding::Rgba8 | Encoding::Bc1 | Encoding::Bc2 | Encoding::Bc3
        ) || options.transfer == Transfer::Unspecified
            || !matches!(options.alpha, SourceAlpha::Straight | SourceAlpha::Opaque)
            || (options.encoding == Encoding::Bc1 && options.alpha != SourceAlpha::Opaque)
        {
            return Err(unsupported(
                "DDS texture needs explicit qualified encoding/transfer/alpha; BC1 cutout is not qualified",
            ));
        }
        let maximum = (32 - width.max(height).leading_zeros()) as u8;
        let count = match options.mips {
            MipCount::Full => maximum,
            MipCount::Levels(count) if count > 0 && count <= maximum => count,
            _ => return Err(codec("DDS requested mip count exceeds its geometry")),
        };
        let mut levels = Vec::new();
        levels
            .try_reserve_exact(usize::from(count))
            .map_err(|_| resource("DDS texture plan allocation"))?;
        let mut offset = 148u64;
        let mut mip_bytes = 0;
        let mut visits = 0u64;
        let (mut w, mut h) = (width, height);
        for index in 0..count {
            let byte_len = mip::level_bytes(w, h, options.encoding)?;
            levels.push(MipLevel {
                width: w,
                height: h,
                byte_offset: offset,
                byte_len,
            });
            offset = offset
                .checked_add(byte_len)
                .ok_or_else(|| resource("DDS encoded extent overflow"))?;
            if index > 0 {
                mip_bytes = mip_bytes.max(u64::from(w) * u64::from(h) * 4);
                // A scaled source interval intersects at most ceil(src/dst)+1
                // source cells per axis. This bounds exact area-filter work.
                let work = match options.filter {
                    MipFilter::Nearest => u64::from(w) * u64::from(h),
                    MipFilter::BoxStored => {
                        u64::from(w)
                            * u64::from(h)
                            * (u64::from(width).div_ceil(u64::from(w)) + 1)
                            * (u64::from(height).div_ceil(u64::from(h)) + 1)
                    }
                };
                visits = visits
                    .checked_add(work)
                    .ok_or_else(|| resource("DDS mip work overflow"))?;
            }
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        if offset > BYTES_MAX as u64 || visits > 256_000_000 {
            return Err(resource(
                "DDS texture output or filter-work allowance exceeded",
            ));
        }
        // Selected dds 0.2 BC path: 4 rows of [f32;4], one encoded block
        // row, and a 4x4 partial-block buffer. No auto-resize/parallel path.
        let codec_bytes = if options.encoding == Encoding::Rgba8 {
            0
        } else {
            u64::from(width) * 64 + u64::from(width).div_ceil(4) * 16 + 256
        };
        Ok(Self {
            options,
            levels,
            source_bytes: pixels * 4,
            encoded_bytes: offset,
            mip_bytes,
            codec_bytes,
            filter_visits: visits,
        })
    }
    pub fn options(&self) -> TextureOptions {
        self.options
    }
    pub fn levels(&self) -> &[MipLevel] {
        &self.levels
    }
    pub fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
    pub fn encoded_bytes(&self) -> u64 {
        self.encoded_bytes
    }
    pub fn filter_visits(&self) -> u64 {
        self.filter_visits
    }
    /// Logical live payload pricing, including borrowed source exactly once.
    /// Codec Vec allocation overhead/stack/system RSS are not a hard bound.
    pub fn reservation_bytes(&self) -> Result<u64> {
        self.source_bytes
            .checked_add(self.encoded_bytes)
            .and_then(|n| n.checked_add(self.mip_bytes))
            .and_then(|n| n.checked_add(self.codec_bytes))
            .and_then(|n| {
                n.checked_add(self.levels.len() as u64 * std::mem::size_of::<MipLevel>() as u64)
            })
            .ok_or_else(|| resource("DDS texture reservation overflow"))
    }
    /// Definite encoding losses. Source-dependent Box hidden-RGB loss is
    /// additionally reported by the completed EncodedTexture::losses().
    pub fn losses(&self) -> &[&'static str] {
        if self.options.encoding == Encoding::Rgba8 {
            &[]
        } else {
            &[BLOCK_COMPRESSION_LOSS]
        }
    }
}
