//! Private opt-in float envelopes. No registry route or helper discovery policy.
use super::*;
#[cfg(all(feature = "hdr-codec", feature = "exr-codec"))]
use crate::float_import;
use crate::{float_import::wire as payload, precision::Settings};
pub const RAW_INPUT_BYTES_MAX: usize = 16 << 20;
pub const INPUT_BYTES_MAX: usize = RAW_INPUT_BYTES_MAX + payload::SETTINGS_BYTES;
pub const PIXELS_MAX: u64 = 1_048_576;
pub const OUTPUT_BYTES_MAX: usize =
    RESPONSE_HEADER + 64 + payload::REPORT_BYTES + PIXELS_MAX as usize * 4;
/// Caller admits raw source and the additional bounded envelope copy first.
pub fn capture(
    codec: Codec,
    source: Arc<Vec<u8>>,
    settings: Settings,
    owner: Owner,
    pixels: u64,
) -> Result<Request> {
    if cfg!(target_arch = "wasm32") || owner.frontend != 3 {
        return Err(unsupported("Private float capture is native Studio-only"));
    }
    if !codec.is_float() {
        return Err(unsupported("Float settings require a float codec"));
    }
    if source.is_empty() || source.len() > RAW_INPUT_BYTES_MAX || pixels == 0 || pixels > PIXELS_MAX
    {
        return Err(resource("Private float request allowance"));
    }
    let settings = payload::encode_settings(settings)?;
    let mut envelope = Vec::new();
    envelope
        .try_reserve_exact(settings.len() + source.len())
        .map_err(|_| resource("Float request envelope allocation"))?;
    envelope.extend(settings);
    envelope.extend(source.as_slice());
    Request::capture_for(codec, Arc::new(envelope), owner, pixels)
}
pub fn settings_and_source(header: Header, envelope: &[u8]) -> Result<(Settings, &[u8])> {
    if !header.codec.is_float()
        || header.length as usize != envelope.len()
        || envelope.len() <= payload::SETTINGS_BYTES
        || envelope.len() > INPUT_BYTES_MAX
        || header.pixels == 0
        || u64::from(header.pixels) > PIXELS_MAX
    {
        return Err(resource("Private float envelope allowance"));
    }
    header.verify_source(envelope)?;
    Ok((
        payload::decode_settings(&envelope[..payload::SETTINGS_BYTES])?,
        &envelope[payload::SETTINGS_BYTES..],
    ))
}
#[cfg(all(feature = "hdr-codec", feature = "exr-codec"))]
pub fn execute(header: Header, envelope: &[u8], peak: &dyn Fn() -> u64) -> Result<Vec<u8>> {
    use paintcraft_core::types::resources::SharedBudget;
    let (settings, source) = settings_and_source(header, envelope)?;
    let budget = SharedBudget::new(HEAP_BYTES);
    let _input = budget.reserve(envelope.len() as u64)?;
    let prepared = match header.codec {
        Codec::HdrSdr8 => float_import::prepare_hdr(
            source,
            u64::from(header.pixels),
            settings,
            budget.clone(),
            &|| Ok(()),
        )?,
        Codec::ExrSdr8 => float_import::prepare_exr(
            source,
            u64::from(header.pixels),
            settings,
            budget.clone(),
            &|| Ok(()),
        )?,
        _ => return Err(unsupported("Not a private float codec")),
    };
    let (w, h) = prepared.assessment().binding().size();
    let report = prepared.report().encode();
    let rgba = prepared.preview_rgba();
    let output_bytes = RESPONSE_HEADER
        .checked_add(env!("CARGO_PKG_VERSION").len())
        .and_then(|n| n.checked_add(report.len()))
        .and_then(|n| n.checked_add(rgba.len()))
        .ok_or_else(|| resource("Float response extent overflow"))?;
    if output_bytes > OUTPUT_BYTES_MAX {
        return Err(resource("Float response allowance"));
    }
    let _output = budget.reserve(output_bytes as u64)?;
    let version = env!("CARGO_PKG_VERSION").as_bytes();
    let mut out = response_header(
        header,
        0,
        w,
        h,
        NativeEncoding {
            primaries: 1,
            transfer: 13,
            matrix: 0,
            full_range: true,
        },
        rgba.len(),
        0,
        report.len(),
        version.len(),
        peak(),
    )?;
    out.try_reserve_exact(version.len() + report.len() + rgba.len())
        .map_err(|_| resource("Float response allocation"))?;
    out.extend(version);
    out.extend(report);
    out.extend(rgba);
    Ok(out)
}
