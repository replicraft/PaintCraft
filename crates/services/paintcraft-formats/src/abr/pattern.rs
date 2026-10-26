//! Exact pattern channels plus a separately labelled converted preview.
//! https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/
mod preview;
#[cfg(test)]
mod tests;
use super::{preflight::Cursor, *};

pub const COUNT_MAX: usize = 64;
pub const PIXELS_MAX: usize = 4 << 20;
pub const TOTAL_PIXELS_MAX: usize = 16 << 20;
pub const VALUES_MAX: usize = 16 << 20;
pub const DECODED_BYTES_MAX: usize = 64 << 20;
pub const BACKEND_BYTES_MAX: usize = 36 << 20;
pub const BACKEND_TOTAL_BYTES_MAX: usize = 144 << 20;
const NAME_UNITS_MAX: usize = 1024;
const NAME_BYTES_MAX: usize = 256 << 10;
const SLOTS_MAX: usize = 66;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    Rgb,
    Grayscale,
    Indexed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Samples {
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32Bits(Vec<u32>),
}
impl Samples {
    pub fn len(&self) -> usize {
        match self {
            Self::U8(v) => v.len(),
            Self::U16(v) => v.len(),
            Self::F32Bits(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    bounds: [u32; 4],
    /// Original compression code, zero for raw or one for PackBits.
    compression: u8,
    samples: Samples,
}
impl Channel {
    pub fn bounds(&self) -> [u32; 4] {
        self.bounds
    }
    pub fn compression(&self) -> u8 {
        self.compression
    }
    pub fn samples(&self) -> &Samples {
        &self.samples
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    Unwritten,
    EmptyWritten,
    Channel(Channel),
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConvertedPreview {
    /// Straight RGBA8 thumbnail values, not the original pattern storage.
    pub rgba: Vec<u8>,
    /// Unqualified channel/palette metadata keeps its data without a thumbnail.
    /// `rgba` is empty when this reason is present.
    pub unavailable: Option<PreviewUnavailable>,
    pub reduced_precision: bool,
    pub clamped_float_values: bool,
    pub auxiliary_channels_not_rendered: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewUnavailable {
    MissingColorChannels,
    AdditionalColorChannels,
    UserMask,
    PaletteMetadata,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    name: String,
    id: String,
    /// Vertical, horizontal, retaining the original signed point values.
    point: [i16; 2],
    bounds: [u32; 4],
    color_mode: ColorMode,
    palette: Option<Vec<[u8; 3]>>,
    palette_extra: Option<[u8; 4]>,
    slots: Vec<Slot>,
    preview: ConvertedPreview,
}
impl Pattern {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn point(&self) -> [i16; 2] {
        self.point
    }
    pub fn bounds(&self) -> [u32; 4] {
        self.bounds
    }
    pub fn color_mode(&self) -> ColorMode {
        self.color_mode
    }
    pub fn palette(&self) -> Option<&[[u8; 3]]> {
        self.palette.as_deref()
    }
    pub fn palette_extra(&self) -> Option<[u8; 4]> {
        self.palette_extra
    }
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }
    /// Explicitly converted informational preview. This does not adopt a brush,
    /// convert stored channel precision, or claim color-managed display parity.
    pub fn converted_preview(&self) -> &ConvertedPreview {
        &self.preview
    }
}
#[derive(Default)]
pub(super) struct Budget {
    count: usize,
    pixels: usize,
    values: usize,
    bytes: usize,
    names: usize,
    backend_bytes: usize,
    writer_name_terminator: bool,
}

pub(super) fn inspect(
    source: &mut Cursor<'_>,
    budget: &mut Budget,
    patterns: &mut Vec<Pattern>,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    while source.pos < source.bytes.len() {
        check()?;
        budget.count += 1;
        if budget.count > COUNT_MAX {
            return Err(resource("ABR pattern count allowance exceeded"));
        }
        let size = source.u32()? as usize;
        let mut record = Cursor {
            bytes: source.take(size)?,
            pos: 0,
        };
        if record.u32()? != 1 {
            return Err(unsupported("unsupported ABR pattern version"));
        }
        let color_mode = match record.u32()? {
            1 => ColorMode::Grayscale,
            2 => ColorMode::Indexed,
            3 => ColorMode::Rgb,
            _ => {
                return Err(unsupported(
                    "ABR pattern color model needs a qualified conversion route",
                ));
            }
        };
        let point = [record.u16()? as i16, record.u16()? as i16];
        let name = name(&mut record, budget)?;
        let id_size = usize::from(record.take(1)?[0]);
        let id = record.take(id_size)?;
        if !id.iter().all(|v| v.is_ascii() && *v != 0) {
            return Err(invalid("ABR pattern ID must be null-free ASCII"));
        }
        let id = String::from_utf8(id.to_vec()).unwrap();
        budget.names += id_size;
        if budget.names > NAME_BYTES_MAX {
            return Err(resource("ABR pattern name allowance exceeded"));
        }
        let mut palette = None;
        let mut palette_extra = None;
        if color_mode == ColorMode::Indexed {
            palette = Some(record.take(768)?.as_chunks::<3>().0.to_vec());
            let next = record
                .bytes
                .get(record.pos..record.pos + 4)
                .ok_or_else(|| invalid("truncated ABR pattern palette"))?;
            let following = record.bytes.get(record.pos + 4..record.pos + 8);
            if next != [0, 0, 0, 3] || following == Some(&[0, 0, 0, 3]) {
                palette_extra = Some(record.take(4)?.try_into().unwrap());
            }
        }
        if record.u32()? != 3 {
            return Err(unsupported("unsupported ABR pattern VMAL version"));
        }
        let length = record.u32()? as usize;
        let mut arrays = Cursor {
            bytes: record.take(length)?,
            pos: 0,
        };
        let bounds = rectangle(&mut arrays)?;
        let (width, height) = dimensions(bounds)?;
        let pixels = width * height;
        budget.pixels += pixels;
        if pixels > PIXELS_MAX || budget.pixels > TOTAL_PIXELS_MAX {
            return Err(resource("ABR pattern pixel allowance exceeded"));
        }
        let backend_bound = pixels * 8 + height * 8 + 4096;
        budget.backend_bytes += backend_bound;
        if backend_bound > BACKEND_BYTES_MAX || budget.backend_bytes > BACKEND_TOTAL_BYTES_MAX {
            return Err(resource("ABR pattern backend aggregate allowance exceeded"));
        }
        let slots = arrays.u32()? as usize;
        if slots > SLOTS_MAX - 2 {
            return Err(resource("ABR pattern channel count allowance exceeded"));
        }
        let mut channels = Vec::new();
        for slot_index in 0..slots + 2 {
            check()?;
            let written = arrays.u32()?;
            if written == 0 {
                channels.push(Slot::Unwritten);
                continue;
            }
            if written != 1 {
                return Err(invalid("ABR pattern written flag must be Boolean"));
            }
            let size = arrays.u32()? as usize;
            if size == 0 {
                channels.push(Slot::EmptyWritten);
                continue;
            }
            let mut channel = Cursor {
                bytes: arrays.take(size)?,
                pos: 0,
            };
            let depth = channel.u32()?;
            let region = rectangle(&mut channel)?;
            if region[0] < bounds[0]
                || region[1] < bounds[1]
                || region[2] > bounds[2]
                || region[3] > bounds[3]
            {
                return Err(invalid("ABR pattern channel lies outside its rectangle"));
            }
            let (w, h) = dimensions_allow_empty(region)?;
            if u32::from(channel.u16()?) != depth {
                return Err(invalid("ABR pattern channel depths disagree"));
            }
            let stride = match depth {
                8 => 1,
                16 => 2,
                32 => 4,
                _ => return Err(unsupported("unsupported ABR pattern channel precision")),
            };
            if color_mode == ColorMode::Indexed && slot_index == 0 && depth != 8 {
                return Err(unsupported("ABR palette indices must be eight-bit"));
            }
            budget.values += w * h;
            budget.bytes += w * h * stride;
            if budget.values > VALUES_MAX || budget.bytes > DECODED_BYTES_MAX {
                return Err(resource("ABR pattern decoded channel allowance exceeded"));
            }
            let compression = channel.take(1)?[0];
            let data = match compression {
                0 => channel.take(w * h * stride)?.to_vec(),
                1 => preflight::rle(&mut channel, w * stride, h, check)?,
                _ => return Err(unsupported("unsupported ABR pattern channel compression")),
            };
            channel.finished()?;
            let samples = match depth {
                8 => Samples::U8(data),
                16 => Samples::U16(
                    data.as_chunks::<2>()
                        .0
                        .iter()
                        .map(|v| u16::from_be_bytes(*v))
                        .collect(),
                ),
                32 => {
                    let mut values = Vec::new();
                    for bits in data.as_chunks::<4>().0 {
                        let bits = u32::from_be_bytes(*bits);
                        if !f32::from_bits(bits).is_finite() {
                            return Err(invalid("ABR pattern float samples must be finite"));
                        }
                        values.push(bits);
                    }
                    Samples::F32Bits(values)
                }
                _ => unreachable!(),
            };
            channels.push(Slot::Channel(Channel {
                bounds: region,
                compression,
                samples,
            }));
        }
        arrays.finished()?;
        if record.bytes.len() - record.pos > 3 {
            return Err(invalid("ABR pattern has unexplained trailing bytes"));
        }
        record.zeros(record.bytes.len() - record.pos)?;
        source.zeros((4 - size % 4) % 4)?;
        patterns.push(Pattern {
            name,
            id,
            point,
            bounds,
            color_mode,
            palette,
            palette_extra,
            slots: channels,
            preview: ConvertedPreview::default(),
        });
    }
    Ok(())
}
fn rectangle(source: &mut Cursor<'_>) -> Result<[u32; 4]> {
    Ok([source.u32()?, source.u32()?, source.u32()?, source.u32()?])
}
fn dimensions_allow_empty(bounds: [u32; 4]) -> Result<(usize, usize)> {
    let width = bounds[3]
        .checked_sub(bounds[1])
        .ok_or_else(|| invalid("inverted ABR pattern rectangle"))?;
    let height = bounds[2]
        .checked_sub(bounds[0])
        .ok_or_else(|| invalid("inverted ABR pattern rectangle"))?;
    if width > DIMENSION_MAX as u32 || height > DIMENSION_MAX as u32 {
        return Err(resource("ABR pattern dimension allowance exceeded"));
    }
    Ok((width as usize, height as usize))
}
fn dimensions(bounds: [u32; 4]) -> Result<(usize, usize)> {
    let dimensions = dimensions_allow_empty(bounds)?;
    if dimensions.0 == 0 || dimensions.1 == 0 {
        return Err(invalid("empty ABR pattern rectangle"));
    }
    Ok(dimensions)
}
fn name(source: &mut Cursor<'_>, budget: &mut Budget) -> Result<String> {
    let units = source.u32()? as usize;
    if units > NAME_UNITS_MAX + usize::from(budget.writer_name_terminator) {
        return Err(resource("ABR pattern name unit allowance exceeded"));
    }
    budget.names += units * 2;
    if budget.names > NAME_BYTES_MAX {
        return Err(resource("ABR pattern name allowance exceeded"));
    }
    let data = source.take(units * 2)?;
    let data = data.strip_suffix(&[0, 0]).unwrap_or(data);
    let mut text = String::new();
    for character in char::decode_utf16(
        data.as_chunks::<2>()
            .0
            .iter()
            .map(|v| u16::from_be_bytes(*v)),
    ) {
        let character = character.map_err(|_| invalid("invalid ABR pattern UTF-16"))?;
        if character == '\0' {
            return Err(invalid("ABR pattern name contains an interior null"));
        }
        text.push(character);
    }
    Ok(text)
}
pub(super) fn verify(pattern: &mut Pattern, check: &dyn Fn() -> Result<()>) -> Result<()> {
    pattern.preview = preview::decode(pattern, check)?;
    Ok(())
}
