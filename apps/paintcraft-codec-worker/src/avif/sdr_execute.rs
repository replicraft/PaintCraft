//! Opt-in V6/code6 producer; identity code5 retains its existing policy.
use super::execute::decode_item;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{avif, codec_worker as wire};
pub(crate) fn execute(
    header: wire::Header,
    source: &[u8],
    peak: &dyn Fn() -> u64,
) -> Result<Vec<u8>> {
    if header.codec != wire::Codec::AvifSdr8 {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "AVIF SDR execution requires code6",
        ));
    }
    header.verify_source(source)?;
    let admitted = avif::sdr::admit_ycbcr8(source, u64::from(header.pixels), &|| Ok(()))?;
    // Full source/native/i32/RGBA/scratch declaration precedes any C decoding.
    avif::sdr::work_layout(&admitted, header.length)?;
    let primary = decode_item(&admitted.primary)?;
    let alpha = admitted.alpha.as_ref().map(decode_item).transpose()?;
    let (width, height) = (admitted.primary.width, admitted.primary.height);
    let count = width as usize * height as usize;
    let rgba = avif::sdr::reconstruct(
        &admitted,
        header.length,
        &primary.planes,
        alpha.as_ref().map(|values| &values.planes[..count]),
        &|| Ok(()),
    )?;
    drop(primary);
    drop(alpha);
    wire::success(
        header,
        wire::Success {
            width,
            height,
            pixels: wire::Pixels::SrgbRgba8(&rgba),
        },
        peak(),
    )
}
