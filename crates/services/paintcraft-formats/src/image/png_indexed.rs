//! Expand validated original palette indices, retaining straight transparent RGB.
//! The backend EXPAND transform fills undeclared entries with opaque black;
//! importing that output would silently repair an invalid source.
use super::{Decoded, ImageFormat, codec, png_metadata::Descriptor};
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::io::Cursor;

fn buffer(len: usize) -> Result<Vec<u8>> {
    super::admit_buffer_bytes(len as u64, isize::MAX as u64)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(len).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "PNG allocation exceeds available memory",
        )
    })?;
    bytes.resize(len, 0);
    Ok(bytes)
}

pub(super) fn decode(
    bytes: &[u8],
    source: &Descriptor,
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Decoded> {
    check()?;
    let allowance = pixels.min(super::DECODE_PIXELS_MAX).saturating_mul(8);
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: usize::try_from(allowance).unwrap_or(usize::MAX),
        },
    );
    decoder.set_transformations(png::Transformations::IDENTITY);
    decoder.set_ignore_text_chunk(false);
    let mut reader = decoder.read_info().map_err(|e| codec(e.to_string()))?;
    check()?;
    let header = reader.info();
    if header.width != source.width
        || header.height != source.height
        || header.color_type != png::ColorType::Indexed
        || header.bit_depth as u8 != source.bit_depth
        || header.palette.as_deref() != Some(source.palette.rgb.as_flattened())
        || header.trns.as_deref().unwrap_or_default() != source.palette.alpha
    {
        return Err(codec("PNG palette descriptor differs from decoder source"));
    }
    let animated = header.animation_control.is_some();
    let len = reader.output_buffer_size().ok_or_else(|| {
        Error::new(
            ErrorCode::ResourceLimit,
            "PNG packed samples exceed address space",
        )
    })?;
    if len as u64 > allowance {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "PNG packed sample allowance",
        ));
    }
    let mut packed = buffer(len)?;
    let frame = reader
        .next_frame(&mut packed)
        .map_err(|e| codec(e.to_string()))?;
    check()?;
    let depth = usize::from(source.bit_depth);
    let stride = (source.width as usize * depth).div_ceil(8);
    if !matches!(depth, 1 | 2 | 4 | 8)
        || frame.color_type != png::ColorType::Indexed
        || frame.bit_depth as u8 != source.bit_depth
        || frame.width != source.width
        || frame.height != source.height
        || frame.line_size != stride
        || frame.buffer_size() != stride * source.height as usize
    {
        return Err(codec("PNG indexed frame geometry is inconsistent"));
    }
    let count = u64::from(source.width) * u64::from(source.height);
    if count > pixels.min(super::DECODE_PIXELS_MAX) {
        return Err(Error::new(ErrorCode::ResourceLimit, "PNG pixel allowance"));
    }
    let mut rgba =
        buffer(usize::try_from(count * 4).map_err(|_| {
            Error::new(ErrorCode::ResourceLimit, "PNG output exceeds address space")
        })?)?;
    let mask = (1usize << depth) - 1;
    for y in 0..source.height as usize {
        check()?;
        for x in 0..source.width as usize {
            if x % 16384 == 0 {
                check()?;
            }
            let bit = x * depth;
            let index = (usize::from(packed[y * stride + bit / 8]) >> (8 - depth - bit % 8)) & mask;
            let rgb = source
                .palette
                .rgb
                .get(index)
                .ok_or_else(|| codec("PNG sample references an undeclared palette entry"))?;
            let out = (y * source.width as usize + x) * 4;
            rgba[out..out + 3].copy_from_slice(rgb);
            rgba[out + 3] = source.palette.alpha.get(index).copied().unwrap_or(255);
        }
    }
    check()?;
    let mut losses: Vec<String> = source.losses().into_iter().map(str::to_owned).collect();
    if animated {
        losses.push("animation-first-frame-only".into());
    }
    Ok(Decoded {
        format: ImageFormat::Png,
        width: source.width,
        height: source.height,
        rgba,
        losses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const ROOT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/raster-metadata-fixtures/"
    );
    #[test]
    fn independent_packed_indices_preserve_all_palette_samples() {
        let oracle = paintcraft_core::types::json::parse(
            &std::fs::read(format!("{ROOT}oracle.json")).unwrap(),
        )
        .unwrap();
        for fixture in oracle.get("fixtures").unwrap().as_array().unwrap() {
            let file = fixture.get("file").unwrap().as_str().unwrap();
            if !file.starts_with("indexed") || !file.ends_with(".png") {
                continue;
            }
            let bytes = std::fs::read(format!("{ROOT}{file}")).unwrap();
            let descriptor = super::super::png_metadata::inspect(&bytes, 100, &|| Ok(())).unwrap();
            let decoded = decode(&bytes, &descriptor, 100, &|| Ok(())).unwrap();
            let expected: Vec<u8> = fixture
                .get("expected_rgba8")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as u8)
                .collect();
            assert_eq!(decoded.rgba, expected, "{file}");
        }
    }
    #[test]
    fn undeclared_palette_entry_is_refused() {
        let bytes = std::fs::read(format!("{ROOT}invalid-index.png")).unwrap();
        let descriptor = super::super::png_metadata::inspect(&bytes, 100, &|| Ok(())).unwrap();
        assert!(decode(&bytes, &descriptor, 100, &|| Ok(())).is_err());
        assert!(super::super::decode(&bytes).is_err());
    }
    #[test]
    fn cancellation_during_expansion_never_returns_partial_pixels() {
        let bytes = std::fs::read(format!("{ROOT}indexed8-trns.png")).unwrap();
        let descriptor = super::super::png_metadata::inspect(&bytes, 100, &|| Ok(())).unwrap();
        let checks = std::cell::Cell::new(0);
        let error = decode(&bytes, &descriptor, 100, &|| {
            checks.set(checks.get() + 1);
            if checks.get() == 5 {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel PNG expansion",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(checks.get(), 5);
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
}
