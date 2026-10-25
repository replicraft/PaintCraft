use super::*;
mod metadata;

pub(super) fn inspect(
    bytes: &[u8],
    pixel_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Inspection> {
    if bytes.len() > ENCODED_MAX {
        return Err(resource("PSD source exceeds encoded byte budget"));
    }
    check()?;
    let mut c = Cursor::new(bytes);
    if c.take(4)? != b"8BPS" {
        return Err(malformed("invalid PSD signature"));
    }
    let version = match c.u16()? {
        1 => Version::Psd,
        2 => Version::Psb,
        _ => return Err(unsupported("PSD version is not qualified")),
    };
    let large = version == Version::Psb;
    c.zeros(6)?;
    let composite_channels = c.u16()?;
    if ![3, 4].contains(&composite_channels) {
        return Err(unsupported(
            "PSD requires RGB and optional merged transparency only",
        ));
    }
    let height = c.u32()?;
    let width = c.u32()?;
    let side = if large { 300000 } else { 30000 };
    if width == 0
        || height == 0
        || width > side
        || height > side
        || u64::from(width) * u64::from(height) > pixel_limit
    {
        return Err(resource("PSD canvas exceeds import pixel budget"));
    }
    if c.u16()? != 8 || c.u16()? != 3 {
        return Err(unsupported("PSD import requires eight-bit RGB samples"));
    }
    if !c.section(false)?.is_empty() {
        return Err(unsupported(
            "PSD RGB color-mode extensions are not qualified",
        ));
    }
    let resources_start = c.pos;
    let resource_data = c.section(false)?;
    let resources = resources_start..c.pos;
    let mut losses = Vec::new();
    metadata::resources(resource_data, &mut losses, check)?;
    let mut metadata_bytes = resource_data.len();
    let mut layer_mask = Cursor::new(c.section(large)?);
    let layer_data = if layer_mask.remaining() == 0 {
        &[][..]
    } else {
        layer_mask.section(large)?
    };
    let mut layers = Vec::new();
    let mut global_alpha = false;
    if !layer_data.is_empty() {
        let mut input = Cursor::new(layer_data);
        let count = i16::from_be_bytes(input.take(2)?.try_into().unwrap());
        global_alpha = count < 0;
        let count = i32::from(count).unsigned_abs() as usize;
        if count > LAYERS_MAX {
            return Err(resource("PSD layer count exceeds import budget"));
        }
        layers
            .try_reserve_exact(count)
            .map_err(|_| resource("cannot allocate PSD layer inventory"))?;
        let mut pixels = 0u64;
        let mut lengths = Vec::with_capacity(count);
        for _ in 0..count {
            check()?;
            let top = input.i32()?;
            let left = input.i32()?;
            let bottom = input.i32()?;
            let right = input.i32()?;
            let w = i64::from(right) - i64::from(left);
            let h = i64::from(bottom) - i64::from(top);
            if w < 0 || h < 0 || w > i64::from(side) || h > i64::from(side) {
                return Err(malformed(
                    "PSD layer rectangle is inverted or outside the format limits",
                ));
            }
            let w = w as u32;
            let h = h as u32;
            pixels += u64::from(w.max(1)) * u64::from(h.max(1));
            if pixels > pixel_limit {
                return Err(resource("PSD aggregate layer pixels exceed import budget"));
            }
            if w == 0 || h == 0 {
                loss(&mut losses, "psd-empty-layer-extent-normalized");
            }
            let count = input.u16()?;
            if ![3, 4].contains(&count) {
                return Err(unsupported(
                    "PSD layer spot channels and masks are not qualified",
                ));
            }
            let mut channels = Vec::with_capacity(usize::from(count));
            for _ in 0..count {
                let id = i16::from_be_bytes(input.take(2)?.try_into().unwrap());
                if ![-1, 0, 1, 2].contains(&id) || channels.iter().any(|(known, _)| *known == id) {
                    return Err(unsupported(
                        "PSD layer has duplicate or unsupported channels",
                    ));
                }
                let length = input.length(large)?;
                if !(2..=ENCODED_MAX).contains(&length) {
                    return Err(malformed("invalid PSD channel payload length"));
                }
                channels.push((id, length));
            }
            if !(0..3).all(|id| channels.iter().any(|(known, _)| *known == id)) {
                return Err(malformed("PSD layer is missing a color channel"));
            }
            if input.take(4)? != b"8BIM" {
                return Err(malformed("invalid PSD layer blend signature"));
            }
            if input.take(4)? != b"norm" {
                return Err(unsupported("PSD layer blend mode is not representable"));
            }
            let opacity = input.u8()?;
            if input.u8()? != 0 {
                return Err(unsupported("PSD clipping layers are not qualified"));
            }
            let flags = input.u8()?;
            if flags & 0xd0 != 0 {
                return Err(unsupported(
                    "PSD non-raster or reserved layer flags are not qualified",
                ));
            }
            if flags & 0x25 != 0 {
                loss(&mut losses, "psd-metadata-ignored");
            }
            input.zeros(1)?;
            let extra = input.section(false)?;
            metadata_bytes = metadata_bytes
                .checked_add(extra.len())
                .filter(|size| *size <= METADATA_MAX)
                .ok_or_else(|| resource("PSD metadata exceeds import allowance"))?;
            let (name, source_id) = metadata::layer(extra, &mut losses, check)?;
            layers.push(LayerInfo {
                name,
                origin: (left, top),
                width: w,
                height: h,
                opacity,
                visible: flags & 2 == 0,
                source_id,
                channels: Vec::new(),
            });
            lengths.push(channels);
        }
        // Record offsets refer to the original immutable file, not a repaired
        // or resized payload. Channel order remains explicit.
        let base = layer_data.as_ptr() as usize - bytes.as_ptr() as usize;
        for (layer, channels) in layers.iter_mut().zip(lengths) {
            for (id, length) in channels {
                check()?;
                let start = base + input.pos;
                let payload = input.take(length)?;
                channels::admit(payload, layer.width, u64::from(layer.height), large, check)?;
                layer.channels.push((id, start..start + length));
            }
        }
        if input.remaining() > 1 {
            return Err(malformed("PSD layer channel data has an undeclared suffix"));
        }
        let pad = input.remaining();
        input.zeros(pad)?;
    }
    if layer_mask.remaining() > 0 {
        if !layer_mask.section(false)?.is_empty() {
            return Err(unsupported("PSD global layer masks are not qualified"));
        }
        // Whole-document tagged blocks can override the layer set (Lr16/Lr32),
        // so no unknown document extensions reach the recursive library reader.
        if layer_mask.remaining() > 3 {
            return Err(unsupported(
                "PSD document tagged extensions are not qualified",
            ));
        }
        let pad = layer_mask.remaining();
        layer_mask.zeros(pad)?;
    }
    if composite_channels == 4 && !global_alpha {
        return Err(unsupported(
            "PSD extra alpha/spot channels are not qualified",
        ));
    }
    let composite = c.pos;
    channels::admit(
        c.take(c.remaining())?,
        width,
        u64::from(height) * u64::from(composite_channels),
        large,
        check,
    )?;
    Ok(Inspection {
        version,
        width,
        height,
        layers,
        losses,
        resources,
        composite,
    })
}
