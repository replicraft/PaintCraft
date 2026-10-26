//! Native-only experimental decoder, behind this executable's allocator/lease.
//! Caller owns the private seekable backing until supervised child reap.
use jpegxr::{BitDepth, ChannelOrder, ColorFormat, PixelInterpretation, SampleFormat};
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{codec_worker as wire, jxr};
use std::{fs, io::Read, path::Path};
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn storage() -> Error {
    Error::new(ErrorCode::StorageFailed, "Cannot read private JXR source")
}
fn verify_file(path: &Path, source: &[u8]) -> Result<()> {
    if !path.is_absolute()
        || path.extension().is_none_or(|ext| ext != "jxr")
        || !fs::symlink_metadata(path)
            .map_err(|_| storage())?
            .file_type()
            .is_file()
    {
        return Err(invalid(
            "JXR source must be an absolute regular private .jxr file",
        ));
    }
    let mut file = fs::File::open(path).map_err(|_| storage())?;
    let metadata = file.metadata().map_err(|_| storage())?;
    if !metadata.is_file() || metadata.len() != source.len() as u64 {
        return Err(invalid(
            "Private JXR source extent differs from captured bytes",
        ));
    }
    let mut block = [0; 65536];
    for expected in source.chunks(block.len()) {
        file.read_exact(&mut block[..expected.len()])
            .map_err(|_| storage())?;
        if &block[..expected.len()] != expected {
            return Err(invalid("Private JXR source differs from captured bytes"));
        }
    }
    if file.read(&mut block[..1]).map_err(|_| storage())? != 0 {
        return Err(invalid("Private JXR source gained trailing bytes"));
    }
    Ok(())
}
pub fn execute(
    header: wire::Header,
    source: &[u8],
    path: &Path,
    peak: &dyn Fn() -> u64,
) -> Result<Vec<u8>> {
    header.verify_source(source)?;
    let info = jxr::inspect_checked(source, u64::from(header.pixels), &|| Ok(()))?;
    verify_file(path, source)?;
    // Audited open -> factory -> create_ws_file_owned(Read) -> decoder Initialize.
    // None of these calls uses the upstream shared-temp convenience APIs.
    let mut decoder = jpegxr::Decoder::open(path)
        .map_err(|_| invalid("Selected JXR decoder initialization failed"))?;
    if decoder
        .dimensions()
        .map_err(|_| invalid("JXR decoded dimensions unavailable"))?
        != (info.width, info.height)
    {
        return Err(invalid("JXR decoded dimensions differ from admission"));
    }
    let format = decoder
        .pixel_format()
        .map_err(|_| invalid("JXR decoded format unavailable"))?;
    if format.bits_per_pixel() != 24
        || format.channel_count() != 3
        || format.bits_per_sample() != 8
        || format.sample_count() != 3
        || format.bit_depth() != BitDepth::Eight
        || format.channel_order() != ChannelOrder::Rgb
        || format.color_format() != ColorFormat::Rgb
        || format.interpretation() != PixelInterpretation::Rgb
        || format.sample_format() != SampleFormat::UnsignedInteger
        || format.has_alpha()
        || format.premultiplied_alpha()
    {
        return Err(invalid(
            "JXR decoded precision/model differs from admitted RGB8",
        ));
    }
    let stride = usize::try_from(info.width)
        .ok()
        .and_then(|width| width.checked_mul(3))
        .ok_or_else(|| invalid("JXR output stride overflow"))?;
    let length = stride
        .checked_mul(info.height as usize)
        .ok_or_else(|| invalid("JXR output length overflow"))?;
    let mut rgb = Vec::new();
    rgb.try_reserve_exact(length)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "JXR RGB output allocation"))?;
    rgb.resize(length, 0);
    decoder
        .decode_into(&mut rgb, stride)
        .map_err(|_| invalid("Selected JXR decoder refused entropy payload"))?;
    drop(decoder);
    verify_file(path, source)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(length / 3 * 4)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "JXR RGBA output allocation"))?;
    for pixel in rgb.as_chunks::<3>().0 {
        rgba.extend([pixel[0], pixel[1], pixel[2], 255]);
    }
    // sRGB is the T.832 AnnexC.2 default AFTER strict preflight excludes all
    // embedded color contexts; RGB channel layout alone is not sufficient.
    wire::success(
        header,
        wire::Success {
            width: info.width,
            height: info.height,
            pixels: wire::Pixels::SrgbRgba8(&rgba),
        },
        peak(),
    )
}
