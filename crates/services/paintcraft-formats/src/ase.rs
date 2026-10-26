//! Adobe Swatch Exchange resources, never image documents or RGB conversions.
//! The selected ag-psd codec runs only after a complete bounded structural check.
mod codec;
mod preflight;
#[cfg(test)]
mod tests;
use paintcraft_core::types::{Error, ErrorCode, Result};

pub const BYTES_MAX: usize = 1 << 20;
pub const BLOCKS_MAX: usize = 8192;
pub const SWATCHES_MAX: usize = 4096;
pub const GROUPS_MAX: usize = 1024;
pub const NAME_UNITS_MAX: usize = 1024;
pub const NAME_BYTES_MAX: usize = 256 << 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Global,
    Spot,
    Normal,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Color {
    Rgb([f32; 3]),
    Cmyk([f32; 4]),
    Gray(f32),
    Lab([f32; 3]),
}
impl Color {
    pub fn model(self) -> &'static str {
        match self {
            Self::Rgb(_) => "RGB",
            Self::Cmyk(_) => "CMYK",
            Self::Gray(_) => "Gray",
            Self::Lab(_) => "LAB",
        }
    }
    pub fn channels(&self) -> &[f32] {
        match self {
            Self::Rgb(v) | Self::Lab(v) => v,
            Self::Cmyk(v) => v,
            Self::Gray(v) => std::slice::from_ref(v),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Swatch {
    pub name: String,
    pub color: Color,
    pub kind: Kind,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Swatch(Swatch),
    Group { name: String, swatches: Vec<Swatch> },
}
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    entries: Vec<Entry>,
}
impl Palette {
    /// Reject invalid resources before the codec allocates its write buffer.
    /// Finite source f32 values are retained exactly, without color conversion.
    pub fn new(entries: Vec<Entry>, check: &dyn Fn() -> Result<()>) -> Result<Self> {
        let palette = Self { entries };
        palette.validate(check)?;
        Ok(palette)
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn swatch_count(&self) -> usize {
        self.entries
            .iter()
            .map(|entry| match entry {
                Entry::Swatch(_) => 1,
                Entry::Group { swatches, .. } => swatches.len(),
            })
            .sum()
    }
    fn validate(&self, check: &dyn Fn() -> Result<()>) -> Result<()> {
        check()?;
        if self.entries.len() > BLOCKS_MAX {
            return Err(resource("ASE resource entry allowance exceeded"));
        }
        let (mut blocks, mut groups, mut swatches, mut names, mut bytes) = (0, 0, 0, 0, 12);
        let mut name = |text: &str| -> Result<usize> {
            if text.len() > NAME_UNITS_MAX * 4 || text.contains('\0') {
                return Err(invalid("ASE name is oversized or contains a null"));
            }
            let n = text.encode_utf16().count() + 1;
            if n > NAME_UNITS_MAX + 1 {
                return Err(resource("ASE name unit allowance exceeded"));
            }
            names += n * 2;
            if names > NAME_BYTES_MAX {
                return Err(resource("ASE aggregate name allowance exceeded"));
            }
            Ok(2 + n * 2)
        };
        for entry in &self.entries {
            check()?;
            let colors = match entry {
                Entry::Swatch(swatch) => std::slice::from_ref(swatch),
                Entry::Group {
                    name: group,
                    swatches: colors,
                } => {
                    groups += 1;
                    blocks += 2;
                    bytes += 12 + name(group)?;
                    colors.as_slice()
                }
            };
            if colors.len() > SWATCHES_MAX {
                return Err(resource("ASE group swatch allowance exceeded"));
            }
            for swatch in colors {
                check()?;
                blocks += 1;
                swatches += 1;
                bytes += 6 + name(&swatch.name)? + 4 + swatch.color.channels().len() * 4 + 2;
                if swatch.color.channels().iter().any(|v| !v.is_finite()) {
                    return Err(invalid("ASE channels must be finite"));
                }
            }
            if blocks > BLOCKS_MAX
                || groups > GROUPS_MAX
                || swatches > SWATCHES_MAX
                || bytes > BYTES_MAX
            {
                return Err(resource("ASE palette capacity allowance exceeded"));
            }
        }
        Ok(())
    }
}

pub fn decode(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Palette> {
    preflight::inspect(bytes, check)?;
    check()?;
    let decoded =
        ag_psd::ase::read_ase(bytes).map_err(|e| invalid(&format!("ASE decoding failed: {e}")))?;
    check()?;
    Palette::new(codec::from_backend(decoded), check)
}
pub fn encode(palette: &Palette, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    palette.validate(check)?;
    let bytes = ag_psd::ase::write_ase(&codec::to_backend(palette));
    check()?;
    preflight::inspect(&bytes, check)?;
    Ok(bytes)
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
