//! Explicit 2D fresh textures. No public consumer/registry activation.
use super::*;
use ::dds::{
    CancellationToken, CompressionQuality, Dithering, EncodeOptions, Encoder, ErrorMetric,
    ImageView, Progress, Size,
};
use std::io::Write;
mod filter;
mod plan;
pub use plan::{MipCount, MipFilter, TextureOptions, TexturePlan, TextureQuality};
pub const UNIFORM_LOSSY_POLICY: &str = "dds-uniform-lossy-samples-v1";
pub const BLOCK_COMPRESSION_LOSS: &str = "dds-block-compression";
pub const MIP_HIDDEN_RGB_LOSS: &str = "dds-mip-hidden-rgb-normalized";
#[derive(Debug)]
pub struct EncodedTexture {
    plan: TexturePlan,
    bytes: Vec<u8>,
    hidden_rgb_normalized: bool,
}
impl EncodedTexture {
    pub fn plan(&self) -> &TexturePlan {
        &self.plan
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    /// Fixed error objective; no universal maximum/RMS error guarantee.
    pub fn compression_policy(&self) -> Option<&'static str> {
        (self.plan.options.encoding != Encoding::Rgba8).then_some(UNIFORM_LOSSY_POLICY)
    }
    pub fn losses(&self) -> &[&'static str] {
        match (
            self.plan.options.encoding != Encoding::Rgba8,
            self.hidden_rgb_normalized,
        ) {
            (false, false) => &[],
            (true, false) => &[BLOCK_COMPRESSION_LOSS],
            (false, true) => &[MIP_HIDDEN_RGB_LOSS],
            (true, true) => &[BLOCK_COMPRESSION_LOSS, MIP_HIDDEN_RGB_LOSS],
        }
    }
}
struct Sink<'a> {
    bytes: Vec<u8>,
    cap: usize,
    control: &'a Control<'a>,
}
impl Write for Sink<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        (self.control.check)().map_err(|e| self.control.io_failure(e))?;
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > self.cap)
        {
            return Err(self
                .control
                .io_failure(resource("DDS texture output exceeded its plan")));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        (self.control.check)().map_err(|e| self.control.io_failure(e))
    }
}
/// Admission precedes source scanning, mip allocation, headers or codec work.
/// Caller retains that admission while returned bytes remain live. Source is
/// packed straight RGBA8 ALREADY in the declared transfer; it is never mutated.
/// This API applies no transfer conversion. A display/sRGB document cannot be
/// labeled Linear by a caller without actual conversion or assignment consent.
/// Explicit Normal/High effort, Uniform/no dithering/no parallel; no alpha matte or swizzle.
pub fn encode_texture_checked(
    width: u32,
    height: u32,
    rgba: &[u8],
    options: TextureOptions,
    admit: impl FnOnce(&TexturePlan) -> Result<()>,
    check: &dyn Fn() -> Result<()>,
) -> Result<EncodedTexture> {
    check()?;
    let plan = TexturePlan::new(width, height, options)?;
    if rgba.len() as u64 != plan.source_bytes() {
        return Err(codec("DDS texture source length disagrees with geometry"));
    }
    admit(&plan)?;
    let check_hidden = options.filter == MipFilter::BoxStored && plan.levels.len() > 1;
    let mut hidden_rgb_normalized = false;
    if options.alpha == SourceAlpha::Opaque || check_hidden {
        for (index, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
            if index.is_multiple_of(1024) {
                check()?;
            }
            if options.alpha == SourceAlpha::Opaque && pixel[3] != 255 {
                return Err(unsupported(
                    "DDS declared opaque source contains nonopaque pixels",
                ));
            }
            hidden_rgb_normalized |= check_hidden && pixel[3] == 0 && pixel[..3] != [0, 0, 0];
        }
    }
    let (format, dxgi) = match (options.encoding, options.transfer) {
        (Encoding::Rgba8, Transfer::Linear) => (Format::R8G8B8A8_UNORM, DxgiFormat::R8G8B8A8_UNORM),
        (Encoding::Rgba8, Transfer::Srgb) => {
            (Format::R8G8B8A8_UNORM, DxgiFormat::R8G8B8A8_UNORM_SRGB)
        }
        (Encoding::Bc1, Transfer::Linear) => (Format::BC1_UNORM, DxgiFormat::BC1_UNORM),
        (Encoding::Bc1, Transfer::Srgb) => (Format::BC1_UNORM, DxgiFormat::BC1_UNORM_SRGB),
        (Encoding::Bc2, Transfer::Linear) => (Format::BC2_UNORM, DxgiFormat::BC2_UNORM),
        (Encoding::Bc2, Transfer::Srgb) => (Format::BC2_UNORM, DxgiFormat::BC2_UNORM_SRGB),
        (Encoding::Bc3, Transfer::Linear) => (Format::BC3_UNORM, DxgiFormat::BC3_UNORM),
        (Encoding::Bc3, Transfer::Srgb) => (Format::BC3_UNORM, DxgiFormat::BC3_UNORM_SRGB),
        _ => return Err(unsupported("DDS texture profile is not admitted")),
    };
    let header =
        Header::new_image(width, height, format).with_mipmap_count(plan.levels.len() as u32);
    let header = header
        .to_dx10()
        .ok_or_else(|| codec("DDS texture DX10 header"))?
        .with_dxgi_format(dxgi)
        .with_alpha_mode(if options.alpha == SourceAlpha::Opaque {
            AlphaMode::Opaque
        } else {
            AlphaMode::Straight
        });
    let control = Control::new(check);
    let mut output = Vec::new();
    output
        .try_reserve_exact(plan.encoded_bytes() as usize)
        .map_err(|_| resource("DDS texture output allocation"))?;
    let mut sink = Sink {
        bytes: output,
        cap: plan.encoded_bytes() as usize,
        control: &control,
    };
    let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
        let mut encoder = Encoder::new(&mut sink, format, &Header::Dx10(header))
            .map_err(|e| control.error(codec(format!("DDS texture header encode: {e}"))))?;
        let mut encoding = EncodeOptions::default();
        encoding.quality = match options.quality {
            TextureQuality::Normal => CompressionQuality::Normal,
            TextureQuality::High => CompressionQuality::High,
        };
        encoding.error_metric = ErrorMetric::Uniform;
        encoding.dithering = Dithering::None;
        encoding.parallel = false;
        encoder.encoding = encoding;
        encoder.mipmaps.generate = false;
        let token = CancellationToken::new();
        let mut report = |_| {
            if let Err(error) = check() {
                let _ = control.io_failure(error);
                token.cancel();
            }
        };
        let mut progress = Progress::new_single_threaded(&mut report).with_cancellation(&token);
        for (index, level) in plan.levels.iter().enumerate() {
            check()?;
            let mip;
            let pixels = if index == 0 {
                rgba
            } else {
                mip = filter::generate(rgba, width, height, level, options.filter, check)?;
                &mip
            };
            let view = ImageView::new(
                pixels,
                Size::new(level.width, level.height),
                ColorFormat::RGBA_U8,
            )
            .ok_or_else(|| codec("DDS texture mip view"))?;
            encoder
                .write_surface_with_progress(view, &mut progress)
                .map_err(|e| control.error(codec(format!("DDS texture surface encode: {e}"))))?;
        }
        encoder
            .finish()
            .map_err(|e| control.error(codec(format!("DDS texture finish: {e}"))))?;
        Ok(())
    }));
    operation.map_err(|_| codec("DDS selected encoder panicked"))??;
    check()?;
    if sink.bytes.len() as u64 != plan.encoded_bytes() {
        return Err(codec("DDS texture encoded extent differs from plan"));
    }
    Ok(EncodedTexture {
        plan,
        bytes: sink.bytes,
        hidden_rgb_normalized,
    })
}

#[cfg(test)]
mod tests;
