//! Explicit RGB/indexed policy and the same prepared pixels for preview/output.
use super::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Rgb24,
    IndexedExact,
    Quantized256,
}

enum Pixels {
    Rgb(Vec<u8>),
    Indexed {
        indices: Vec<u8>,
        palette: Vec<[u8; 3]>,
    },
}
pub struct Prepared {
    width: u32,
    height: u32,
    pixels: Pixels,
    quantized: bool,
}
impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prepared")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("quantized", &self.quantized)
            .finish_non_exhaustive()
    }
}
#[derive(Debug)]
pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

fn buffer<T>(length: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values.try_reserve_exact(length).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "Cannot allocate PCX palette conversion",
        )
    })?;
    Ok(values)
}

/// Apply the declared white matte before identifying colors. Exact indexed mode
/// refuses over 256 colors; quantization occurs only after an explicit request.
pub fn prepare_rgba(
    width: u32,
    height: u32,
    rgba: &[u8],
    mode: Mode,
    check: &dyn Fn() -> Result<()>,
) -> Result<Prepared> {
    check()?;
    let (w, h) = geometry(width, height, (72, 72))?;
    if rgba.len() != w * h * 4 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "PCX palette source length does not match dimensions",
        ));
    }
    let mut rgb = buffer(w * h * 3)?;
    for row in rgba.chunks_exact(w * 4) {
        check()?;
        for px in row.as_chunks::<4>().0 {
            let alpha = u32::from(px[3]);
            rgb.extend(
                px[..3]
                    .iter()
                    .map(|c| ((u32::from(*c) * alpha + 255 * (255 - alpha) + 127) / 255) as u8),
            );
        }
    }
    if mode == Mode::Rgb24 {
        return Ok(Prepared {
            width,
            height,
            pixels: Pixels::Rgb(rgb),
            quantized: false,
        });
    }
    let mut palette = buffer::<[u8; 3]>(256)?;
    let mut lookup = HashMap::new();
    lookup.try_reserve(256).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "Cannot allocate PCX palette lookup",
        )
    })?;
    let mut needs_quantization = false;
    for (index, pixel) in rgb.as_chunks::<3>().0.iter().enumerate() {
        if index.is_multiple_of(16384) {
            check()?;
        }
        if !lookup.contains_key(pixel) {
            if palette.len() == 256 {
                if mode == Mode::IndexedExact {
                    return Err(Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "Exact indexed PCX needs more than 256 colors; choose RGB24 or explicitly request palette quantization",
                    ));
                }
                needs_quantization = true;
                break;
            }
            lookup.insert(*pixel, palette.len() as u8);
            palette.push(*pixel);
        }
    }
    let mut indices = buffer::<u8>(w * h)?;
    if needs_quantization {
        // The upstream training call has no cancellation callback. Keep its
        // input bounded independently of the full image, and check both sides.
        let pixels = w * h;
        let count = pixels.min(32768);
        let mut sample = buffer::<u8>(count * 4)?;
        for i in 0..count {
            if i.is_multiple_of(4096) {
                check()?;
            }
            let pixel = &rgb[((i as u64 * pixels as u64 / count as u64) as usize) * 3..][..3];
            sample.extend_from_slice(pixel);
            sample.push(255);
        }
        check()?;
        let quantizer = color_quant::NeuQuant::new(10, 256, &sample);
        check()?;
        palette = quantizer.color_map_rgb().as_chunks::<3>().0.to_vec();
        for (i, pixel) in rgb.as_chunks::<3>().0.iter().enumerate() {
            if i.is_multiple_of(16384) {
                check()?;
            }
            indices.push(quantizer.index_of(&[pixel[0], pixel[1], pixel[2], 255]) as u8);
        }
    } else {
        for (i, pixel) in rgb.as_chunks::<3>().0.iter().enumerate() {
            if i.is_multiple_of(16384) {
                check()?;
            }
            indices.push(lookup[pixel]);
        }
    }
    // Report actual changed colors, rather than assuming a requested operation
    // must be lossy. Duplicate/exact palettes do not acquire a false loss code.
    let mut changed = false;
    for (i, (pixel, index)) in rgb.as_chunks::<3>().0.iter().zip(&indices).enumerate() {
        if i.is_multiple_of(16384) {
            check()?;
        }
        changed |= *pixel != palette[usize::from(*index)];
    }
    check()?;
    Ok(Prepared {
        width,
        height,
        pixels: Pixels::Indexed { indices, palette },
        quantized: changed,
    })
}

impl Prepared {
    pub fn is_quantized(&self) -> bool {
        self.quantized
    }
    pub fn palette_len(&self) -> Option<usize> {
        match &self.pixels {
            Pixels::Rgb(_) => None,
            Pixels::Indexed { palette, .. } => Some(palette.len()),
        }
    }
    pub fn encode(&self, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
        match &self.pixels {
            Pixels::Rgb(rgb) => encode_rgb(self.width, self.height, rgb, (72, 72), check),
            Pixels::Indexed { indices, palette } => {
                encode_indexed(self.width, self.height, indices, palette, (72, 72), check)
            }
        }
    }
    /// A fitted nearest-sample preview of the exact prepared output colors.
    /// It shares palette/index ownership with the encoder and never quantizes again.
    pub fn preview(&self, side: u16, check: &dyn Fn() -> Result<()>) -> Result<Preview> {
        check()?;
        if !(1..=256).contains(&side) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "PCX preview side must be 1..256",
            ));
        }
        let max = self.width.max(self.height);
        let side = u32::from(side).min(max);
        let width = ((u64::from(self.width) * u64::from(side) + u64::from(max) / 2)
            / u64::from(max))
        .max(1) as u32;
        let height = ((u64::from(self.height) * u64::from(side) + u64::from(max) / 2)
            / u64::from(max))
        .max(1) as u32;
        let mut rgba = buffer::<u8>(width as usize * height as usize * 4)?;
        for y in 0..height {
            check()?;
            for x in 0..width {
                let source = ((u64::from(y) * u64::from(self.height) / u64::from(height))
                    * u64::from(self.width)
                    + u64::from(x) * u64::from(self.width) / u64::from(width))
                    as usize;
                let rgb = match &self.pixels {
                    Pixels::Rgb(rgb) => &rgb[source * 3..][..3],
                    Pixels::Indexed { indices, palette } => &palette[usize::from(indices[source])],
                };
                rgba.extend_from_slice(rgb);
                rgba.push(255);
            }
        }
        check()?;
        Ok(Preview {
            width,
            height,
            rgba,
        })
    }
}
