use super::{
    execution::{Execution, Input, Runner},
    *,
};
use ::jxl::api::{
    AlphaMode, Event, ExtraChannel, JxlBitDepth, JxlColorEncoding, JxlColorProfile, JxlDecoder,
    JxlDecoderOptions, JxlOutputBuffer, JxlPixelFormat, Orientation,
};
use zerocopy::IntoBytes;

fn decoder(pixels: u64) -> JxlDecoder {
    let mut options = JxlDecoderOptions::default();
    // Upstream counts max(width,16) * height * channels and rejects >=.
    // Allow row padding here; the exact host pixel limit is checked from
    // BasicInfo before materializing output. Main5 still constrains complexity.
    options.sample_limit = Some(
        (pixels.min(PIXELS_MAX) as usize)
            .saturating_mul(64)
            .min(PIXELS_MAX as usize * 4)
            .saturating_add(1),
    );
    options.alpha_mode = AlphaMode::Unpremultiplied;
    options.render_spot_colors = false;
    JxlDecoder::new(options)
}
fn header(decoder: &JxlDecoder, metadata: bool, pixels: u64) -> Result<Inspection> {
    let basic = decoder
        .basic_info()
        .ok_or_else(|| codec("JPEG XL decoder omitted basic information"))?;
    admit_size(basic.size.0, basic.size.1, pixels)?;
    Ok(Inspection {
        width: basic.size.0 as u32,
        height: basic.size.1 as u32,
        bits_per_sample: basic.bit_depth.bits_per_sample(),
        floating_point: matches!(basic.bit_depth, JxlBitDepth::Float { .. }),
        animation: basic.animation.is_some(),
        extra_channels: basic.extra_channels.len(),
        associated_alpha: basic
            .extra_channels
            .iter()
            .any(|c| c.ec_type == ExtraChannel::Alpha && c.alpha_associated),
        custom_icc: matches!(
            decoder.embedded_color_profile(),
            Some(JxlColorProfile::Icc(_))
        ),
        metadata_boxes: metadata,
    })
}
fn admit_profile(decoder: &JxlDecoder, info: &Inspection) -> Result<()> {
    if info.floating_point || !matches!(info.bits_per_sample, 8 | 16) {
        return Err(unsupported(
            "JPEG XL float or non-8/16-bit samples require explicit conversion",
        ));
    }
    if info.animation {
        return Err(unsupported(
            "Animated JPEG XL requires an animation import workflow",
        ));
    }
    if info.associated_alpha {
        return Err(unsupported(
            "Associated-alpha JPEG XL requires explicit conversion",
        ));
    }
    let basic = decoder.basic_info().unwrap();
    if info.extra_channels > 1
        || basic
            .extra_channels
            .iter()
            .any(|c| c.ec_type != ExtraChannel::Alpha)
    {
        return Err(unsupported(
            "JPEG XL non-alpha extra channels are not qualified",
        ));
    }
    let profile = decoder.embedded_color_profile();
    let srgb = matches!(profile,Some(JxlColorProfile::Simple(encoding)) if *encoding==JxlColorEncoding::srgb(false)||*encoding==JxlColorEncoding::srgb(true));
    if !srgb || basic.tone_mapping.intensity_target > 255.0 {
        return Err(unsupported(
            "JPEG XL custom ICC or HDR color requires explicit conversion",
        ));
    }
    Ok(())
}
/// Header inspection does not materialize pixels. Upstream ICC scratch is not a
/// caller-controlled allocation; use worker isolation for hostile inputs.
pub fn inspect_checked(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Inspection> {
    let metadata = container::inspect(bytes, check)?;
    let execution = Execution::new(check);
    let mut input = Input {
        bytes,
        execution: &execution,
    };
    let mut decoder = decoder(pixels);
    decoder.close_input();
    check()?;
    match decoder
        .process(&mut input, None, Some(&mut Runner(&execution)))
        .map_err(|e| execution.map(e))?
    {
        Event::BasicInfo => header(&decoder, metadata, pixels),
        Event::NeedMoreInput { .. } | Event::Complete => Err(codec("Truncated JPEG XL header")),
        _ => Err(codec("JPEG XL pixel event preceded basic information")),
    }
}

pub fn decode_with_pixel_limit(bytes: &[u8], pixels: u64) -> Result<Decoded> {
    decode_checked(bytes, pixels, &|| Ok(()))
}
pub fn decode_checked(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    decode_profile(bytes, pixels, check, Profile::General)
}
/// Narrow native helper profile. Invoke only inside the fixed-heap, killable
/// codec worker: upstream header/ICC scratch may allocate before BasicInfo.
/// Unsupported precision and container metadata are refused before allocating
/// PaintCraft's output planes, not before all upstream scratch allocations.
pub fn decode_worker_rgba8_checked(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    decode_profile(bytes, pixels, check, Profile::Worker(Some(8)))
}
/// Exact 16-bit counterpart of the narrow worker profile. This retains all
/// integer samples; it does not admit them into the 8-bit document model.
/// Invoke only inside the fixed-heap, killable codec worker, as with RGBA8.
pub fn decode_worker_rgba16_checked(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    decode_profile(bytes, pixels, check, Profile::Worker(Some(16)))
}
/// One-pass 8-or-16-bit strict isolated profile. The returned sample variant
/// establishes the retained precision; callers must explicitly convert RGBA16.
pub fn decode_worker_integer_checked(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    decode_profile(bytes, pixels, check, Profile::Worker(None))
}
#[derive(Clone, Copy)]
enum Profile {
    General,
    Worker(Option<u32>),
}
fn decode_profile(
    bytes: &[u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
    profile: Profile,
) -> Result<Decoded> {
    let metadata = container::inspect(bytes, check)?;
    let execution = Execution::new(check);
    let mut input = Input {
        bytes,
        execution: &execution,
    };
    let mut decoder = decoder(pixels);
    decoder.close_input();
    let mut info = None;
    let mut output = None;
    let mut frame_complete = false;
    loop {
        check()?;
        let event = if let (Some(info), Some(output)) = (&info, &mut output) {
            let info: &Inspection = info;
            let samples = match output {
                Samples::Rgba8(v) => v.as_mut_slice(),
                Samples::Rgba16(v) => v.as_mut_slice().as_mut_bytes(),
            };
            let bytes_per_sample = if info.bits_per_sample == 16 { 2 } else { 1 };
            let mut buffers = [JxlOutputBuffer::new(
                samples,
                info.height as usize,
                info.width as usize * 4 * bytes_per_sample,
            )];
            decoder.process(
                &mut input,
                Some(&mut buffers),
                Some(&mut Runner(&execution)),
            )
        } else {
            decoder.process(&mut input, None, Some(&mut Runner(&execution)))
        };
        match event.map_err(|e| execution.map(e))? {
            Event::BasicInfo => {
                if info.is_some() {
                    return Err(codec("Duplicate JPEG XL basic information"));
                }
                let inspected = header(&decoder, metadata, pixels)?;
                admit_profile(&decoder, &inspected)?;
                if let Profile::Worker(depth) = profile
                    && (depth.is_some_and(|depth| inspected.bits_per_sample != depth)
                        || metadata
                        || decoder.basic_info().is_some_and(|basic| {
                            basic.orientation != Orientation::Identity
                                || basic.preview_size.is_some()
                                || !basic.uses_original_profile
                        })
                        || !matches!(decoder.embedded_color_profile(), Some(JxlColorProfile::Simple(encoding)) if *encoding == JxlColorEncoding::srgb(false)))
                {
                    return Err(unsupported(match depth {
                        Some(8) => {
                            "Isolated JPEG XL profile requires 8-bit sRGB without discarded container metadata"
                        }
                        Some(16) => {
                            "Isolated JPEG XL profile requires 16-bit sRGB without discarded container metadata"
                        }
                        _ => {
                            "Isolated JPEG XL profile requires 8/16-bit sRGB without discarded container metadata"
                        }
                    }));
                }
                if matches!(profile, Profile::Worker(_)) {
                    worker_profile::admit_channels(
                        inspected.bits_per_sample,
                        &decoder.basic_info().unwrap().extra_channels,
                    )?;
                }
                let count =
                    admit_size(inspected.width as usize, inspected.height as usize, pixels)?;
                let format = if inspected.bits_per_sample == 16 {
                    output = Some(Samples::Rgba16(allocated(count)?));
                    JxlPixelFormat::rgba16(inspected.extra_channels)
                } else {
                    output = Some(Samples::Rgba8(allocated(count)?));
                    JxlPixelFormat::rgba8(inspected.extra_channels)
                };
                decoder
                    .set_pixel_format(format)
                    .map_err(|e| execution.map(e))?;
                info = Some(inspected);
            }
            Event::FrameHeader => {
                if frame_complete {
                    return Err(unsupported("Multiple JPEG XL frames are not qualified"));
                }
            }
            Event::FrameComplete { has_more_frames } => {
                if has_more_frames {
                    return Err(unsupported(
                        "Multiple JPEG XL frames require an animation import workflow",
                    ));
                }
                frame_complete = true;
            }
            Event::Complete => {
                check()?;
                if !frame_complete {
                    return Err(codec("JPEG XL contained no complete frame"));
                }
                let info = info.ok_or_else(|| codec("JPEG XL omitted dimensions"))?;
                return Ok(Decoded {
                    width: info.width,
                    height: info.height,
                    samples: output.ok_or_else(|| codec("JPEG XL omitted pixels"))?,
                    losses: if metadata {
                        vec!["jxl-container-metadata-ignored".into()]
                    } else {
                        vec![]
                    },
                });
            }
            Event::NeedMoreInput { .. } => return Err(codec("Truncated JPEG XL codestream")),
        }
    }
}
