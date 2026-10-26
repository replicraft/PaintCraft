use super::*;
use ag_psd::{
    psd::{PatternBounds, PatternInfo},
    reader::PsdReader,
};

pub(super) fn decode(
    pattern: &Pattern,
    check: &dyn Fn() -> Result<()>,
) -> Result<ConvertedPreview> {
    check()?;
    let colors = if pattern.color_mode == ColorMode::Rgb {
        3
    } else {
        1
    };
    let color_slots = pattern.slots.len().saturating_sub(2);
    // Adobe specifies a 256x3 pattern palette, but not this optional tail.
    // The separate ACT format assigns count/transparency semantics to a tail
    // of the same size. Preserve it; do not assume it is ignorable padding.
    let unavailable = if pattern.color_mode == ColorMode::Indexed && pattern.palette_extra.is_some()
    {
        Some(PreviewUnavailable::PaletteMetadata)
    } else if color_slots < colors
        || pattern.slots[..colors]
            .iter()
            .any(|s| !matches!(s, Slot::Channel(_)))
    {
        Some(PreviewUnavailable::MissingColorChannels)
    } else if pattern.slots[colors..color_slots]
        .iter()
        .any(|s| matches!(s, Slot::Channel(_)))
    {
        Some(PreviewUnavailable::AdditionalColorChannels)
    } else if matches!(pattern.slots[color_slots], Slot::Channel(_)) {
        Some(PreviewUnavailable::UserMask)
    } else {
        None
    };
    if let Some(reason) = unavailable {
        return Ok(ConvertedPreview {
            unavailable: Some(reason),
            auxiliary_channels_not_rendered: reason == PreviewUnavailable::AdditionalColorChannels,
            ..Default::default()
        });
    }
    let (width, height) = dimensions(pattern.bounds)?;
    // A PackBits packet emits at most two bytes per source byte. Four 8-bit
    // planes plus row tables, names and the writer's fixed slot list fit this
    // conservative bound, admitted before invoking any upstream writer/reader.
    let bound = width * height * 8 + height * 8 + 4096;
    if bound > BACKEND_BYTES_MAX {
        return Err(resource("ABR pattern backend view allowance exceeded"));
    }
    let mut preview = ConvertedPreview {
        rgba: vec![0; width * height * 4],
        ..Default::default()
    };
    for pixel in preview.rgba.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    for (ordinal, slot_index) in (0..colors)
        .chain(std::iter::once(pattern.slots.len() - 1))
        .enumerate()
    {
        let Slot::Channel(channel) = &pattern.slots[slot_index] else {
            continue;
        };
        check()?;
        let (w, h) = dimensions_allow_empty(channel.bounds)?;
        let ox = (channel.bounds[1] - pattern.bounds[1]) as usize;
        let oy = (channel.bounds[0] - pattern.bounds[0]) as usize;
        for y in 0..h {
            check()?;
            for x in 0..w {
                let value = sample(&channel.samples, y * w + x, &mut preview);
                let dst = ((y + oy) * width + x + ox) * 4;
                if ordinal == colors {
                    preview.rgba[dst + 3] = value;
                } else {
                    match pattern.color_mode {
                        ColorMode::Rgb => preview.rgba[dst + ordinal] = value,
                        ColorMode::Grayscale => preview.rgba[dst..dst + 3].fill(value),
                        ColorMode::Indexed => {
                            let color = pattern
                                .palette
                                .as_ref()
                                .and_then(|p| p.get(usize::from(value)))
                                .ok_or_else(|| invalid("ABR pattern preview lost its palette"))?;
                            preview.rgba[dst..dst + 3].copy_from_slice(color);
                        }
                    }
                }
            }
        }
    }
    let info = PatternInfo {
        name: pattern.name.clone(),
        id: pattern.id.clone(),
        x: f64::from(pattern.point[0]),
        y: f64::from(pattern.point[1]),
        bounds: PatternBounds {
            x: f64::from(pattern.bounds[1]),
            y: f64::from(pattern.bounds[0]),
            w: width as f64,
            h: height as f64,
        },
        data: preview.rgba.clone(),
    };
    let mut writer = ag_psd::writer::create_writer(4096);
    ag_psd::writer::write_pattern(&mut writer, &info);
    check()?;
    let bytes = ag_psd::writer::get_writer_buffer_no_copy(&writer);
    if bytes.len() > bound {
        return Err(resource(
            "ABR pattern backend view exceeded its admitted bound",
        ));
    }
    // Complete admission also applies to the library-generated channel packets.
    let mut admitted = Vec::new();
    inspect(
        &mut Cursor { bytes, pos: 0 },
        &mut Budget {
            writer_name_terminator: true,
            ..Default::default()
        },
        &mut admitted,
        check,
    )?;
    drop(admitted);
    let mut reader = PsdReader::new(bytes, None, None);
    let parsed = ag_psd::reader::read_pattern(&mut reader)
        .map_err(|e| invalid(&format!("ABR pattern decoding failed: {e}")))?;
    check()?;
    if reader.offset != bytes.len()
        || parsed.name != pattern.name
        || parsed.id != pattern.id
        || parsed.x != info.x
        || parsed.y != info.y
        || parsed.bounds.x != info.bounds.x
        || parsed.bounds.y != info.bounds.y
        || parsed.bounds.w != info.bounds.w
        || parsed.bounds.h != info.bounds.h
        || parsed.data.len() != preview.rgba.len()
        || parsed
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .zip(preview.rgba.as_chunks::<4>().0)
            .any(|(a, b)| a[..3] != b[..3])
    {
        return Err(invalid(
            "ABR pattern backend disagrees with converted preview",
        ));
    }
    // The pinned pattern reader ignores alpha. Exact original alpha channels
    // and the independently constructed straight preview remain separate.
    Ok(preview)
}

fn sample(samples: &Samples, index: usize, preview: &mut ConvertedPreview) -> u8 {
    match samples {
        Samples::U8(v) => v[index],
        Samples::U16(v) => {
            preview.reduced_precision = true;
            ((u32::from(v[index]) + 128) / 257) as u8
        }
        Samples::F32Bits(v) => {
            preview.reduced_precision = true;
            let value = f64::from(f32::from_bits(v[index]));
            preview.clamped_float_values |= !(0.0..=1.0).contains(&value);
            (value.clamp(0.0, 1.0) * 255.0).round() as u8
        }
    }
}
