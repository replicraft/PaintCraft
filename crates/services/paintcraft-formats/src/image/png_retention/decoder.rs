//! Same selected PNG backend/EXPAND/STRIP_16 policy, without ICC/text copies.
use super::*;
use ::image::{ColorType, ImageDecoder, ImageError, ImageResult};
use std::io::Cursor;

struct Decoder<'a> {
    reader: png::Reader<Cursor<&'a [u8]>>,
    color: ColorType,
}
fn error(error: png::DecodingError) -> ImageError {
    match error {
        png::DecodingError::IoError(error) => ImageError::IoError(error),
        png::DecodingError::LimitsExceeded => {
            ImageError::Limits(::image::error::LimitError::from_kind(
                ::image::error::LimitErrorKind::InsufficientMemory,
            ))
        }
        other => ImageError::Decoding(::image::error::DecodingError::new(
            ::image::ImageFormat::Png.into(),
            other,
        )),
    }
}
impl ImageDecoder for Decoder<'_> {
    fn dimensions(&self) -> (u32, u32) {
        self.reader.info().size()
    }
    fn color_type(&self) -> ColorType {
        self.color
    }
    fn read_image(mut self, buffer: &mut [u8]) -> ImageResult<()> {
        if buffer.len() as u64 != self.total_bytes() {
            return Err(ImageError::Decoding(::image::error::DecodingError::new(
                ::image::ImageFormat::Png.into(),
                "PNG output buffer extent",
            )));
        }
        self.reader.next_frame(buffer).map_err(error)?;
        // ImageDecoder's16-bit contract is native endian. The PNG bytes are BE.
        if self.color.bytes_per_pixel() / self.color.channel_count() == 2 {
            for sample in buffer.as_chunks_mut::<2>().0 {
                *sample = u16::from_be_bytes(*sample).to_ne_bytes();
            }
        }
        Ok(())
    }
    fn read_image_boxed(self: Box<Self>, buffer: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buffer)
    }
}
pub(super) fn decode(
    bytes: &[u8],
    source: &png_metadata::Descriptor,
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    check()?;
    let allowance = pixels
        .min(super::super::DECODE_PIXELS_MAX)
        .saturating_mul(8);
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: usize::try_from(allowance).unwrap_or(usize::MAX),
        },
    );
    decoder.ignore_checksums(false);
    decoder.set_ignore_iccp_chunk(true);
    decoder.set_ignore_text_chunk(true);
    decoder.set_transformations(png::Transformations::EXPAND);
    let reader = decoder
        .read_info()
        .map_err(|e| super::super::image_error(error(e)))?;
    if reader.info().size() != (source.width, source.height) {
        return Err(codec("PNG source geometry differs from pixel decoder"));
    }
    let animated = reader.info().animation_control.is_some();
    let color = match reader.output_color_type() {
        (png::ColorType::Grayscale, png::BitDepth::Eight) => ColorType::L8,
        (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight) => ColorType::La8,
        (png::ColorType::Rgb, png::BitDepth::Eight) => ColorType::Rgb8,
        (png::ColorType::Rgba, png::BitDepth::Eight) => ColorType::Rgba8,
        (png::ColorType::Grayscale, png::BitDepth::Sixteen) => ColorType::L16,
        (png::ColorType::GrayscaleAlpha, png::BitDepth::Sixteen) => ColorType::La16,
        (png::ColorType::Rgb, png::BitDepth::Sixteen) => ColorType::Rgb16,
        (png::ColorType::Rgba, png::BitDepth::Sixteen) => ColorType::Rgba16,
        _ => return Err(codec("PNG expanded sample model differs")),
    };
    let mut losses: Vec<String> = source.losses().into_iter().map(str::to_owned).collect();
    if animated {
        losses.push("animation-first-frame-only".to_owned());
    }
    check()?;
    let decoded = super::super::decode_pixels_metadata(
        Decoder { reader, color },
        ImageFormat::Png,
        losses,
        pixels,
        false,
    )?;
    check()?;
    Ok(decoded)
}
