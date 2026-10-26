//! Bounded borrowed metadata; calibration records describe already-adjusted
//! stored samples. They are not instructions to apply exposure a second time.
use super::*;
#[derive(Clone, Debug, PartialEq)]
pub struct Record<'a> {
    pub key: Option<&'a str>,
    pub value: &'a str,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Header<'a> {
    pub width: u32,
    pub height: u32,
    pub data_offset: usize,
    pub records: Vec<Record<'a>>,
    /// Product of EXPOSURE records already applied to the stored samples.
    pub applied_exposure: f64,
    /// Per-channel product of COLORCORR records already applied to samples.
    pub applied_color_correction: [f64; 3],
    pub pixel_aspect: f64,
    pub primaries: Option<[f64; 8]>,
    pub primaries_conflict: bool,
}
impl Header<'_> {
    pub fn color_metadata(&self) -> crate::precision::ColorMetadata {
        use crate::precision::{ColorEvidence, ColorMetadata, LinearSpace};
        if self.primaries_conflict {
            return ColorMetadata::Unspecified;
        }
        let Some(actual) = self.primaries else {
            return ColorMetadata::RadianceDefault;
        };
        for (expected, space) in [
            (
                [0.64, 0.33, 0.29, 0.60, 0.15, 0.06, 1.0 / 3.0, 1.0 / 3.0],
                LinearSpace::RadianceEqualEnergy,
            ),
            (
                [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290],
                LinearSpace::Rec709D65,
            ),
        ] {
            // Exact recognized numeric declarations only. Rounded/nearby
            // primaries remain unresolved rather than silently snapping gamut.
            if actual == expected {
                return ColorMetadata::Declared {
                    space,
                    evidence: ColorEvidence::Chromaticities,
                };
            }
        }
        // Explicit unfamiliar primaries need the same deliberate user source
        // assignment path as other unresolved metadata. Raw coordinates remain.
        ColorMetadata::Unspecified
    }
    pub fn has_noncolor_metadata(&self) -> bool {
        self.records
            .iter()
            .any(|record| !matches!(record.key, Some("FORMAT" | "PRIMARIES")))
    }
}
struct Lines<'a> {
    bytes: &'a [u8],
    offset: usize,
    count: usize,
}
impl<'a> Lines<'a> {
    fn line(&mut self, check: &dyn Fn() -> Result<()>) -> Result<&'a str> {
        check()?;
        self.count += 1;
        if self.count > HEADER_LINES_MAX {
            return Err(resource("Radiance header line count exceeds allowance"));
        }
        let tail = self
            .bytes
            .get(self.offset..)
            .ok_or_else(|| invalid("Truncated Radiance header"))?;
        let size = tail
            .iter()
            .take(LINE_BYTES_MAX + 1)
            .position(|b| *b == b'\n')
            .ok_or_else(|| resource("Radiance header line exceeds allowance or is unterminated"))?;
        let end = self
            .offset
            .checked_add(size + 1)
            .ok_or_else(|| resource("Radiance header position overflow"))?;
        if end > HEADER_BYTES_MAX {
            return Err(resource("Radiance header exceeds allowance"));
        }
        let bytes = &tail[..size];
        self.offset = end;
        let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
        std::str::from_utf8(bytes)
            .map_err(|_| unsupported("Radiance non-UTF8 metadata is unqualified"))
    }
}
fn numbers<const N: usize>(value: &str) -> Result<[f64; N]> {
    let mut fields = value.split_ascii_whitespace();
    let mut result = [0.0f64; N];
    for slot in &mut result {
        let text = fields
            .next()
            .ok_or_else(|| invalid("Missing Radiance numeric metadata field"))?;
        if text.len() > 64 {
            return Err(resource(
                "Radiance numeric metadata token exceeds allowance",
            ));
        }
        *slot = text
            .parse()
            .map_err(|_| invalid("Invalid Radiance numeric metadata"))?;
        if !slot.is_finite() {
            return Err(unsupported(
                "Nonfinite Radiance numeric metadata is unqualified",
            ));
        }
    }
    if fields.next().is_some() {
        return Err(invalid("Extra Radiance numeric metadata fields"));
    }
    Ok(result)
}
fn multiply(left: f64, right: f64) -> Result<f64> {
    let product = left * right;
    if left <= 0.0 || right <= 0.0 || product <= 0.0 || !product.is_finite() {
        return Err(unsupported(
            "Radiance calibration product must be finite and positive",
        ));
    }
    Ok(product)
}
pub(super) fn parse<'a>(
    bytes: &'a [u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Header<'a>> {
    let mut lines = Lines {
        bytes,
        offset: 0,
        count: 0,
    };
    if lines.line(check)? != "#?RADIANCE" {
        return Err(unsupported(
            "Initial Radiance profile requires the RADIANCE signature",
        ));
    }
    let mut header = Header {
        width: 0,
        height: 0,
        data_offset: 0,
        records: Vec::new(),
        applied_exposure: 1.0,
        applied_color_correction: [1.0; 3],
        pixel_aspect: 1.0,
        primaries: None,
        primaries_conflict: false,
    };
    let mut format = false;
    let mut gamma = false;
    loop {
        let line = lines.line(check)?;
        if line.is_empty() {
            break;
        }
        let (key, value) = if line.starts_with('#') || line.starts_with(char::is_whitespace) {
            (None, line)
        } else {
            match line.split_once('=') {
                Some((key, value)) => (Some(key), value.trim()),
                None => (None, line),
            }
        };
        match key {
            Some("FORMAT") => {
                if format {
                    return Err(invalid("Duplicate Radiance FORMAT"));
                }
                if value != "32-bit_rle_rgbe" {
                    return Err(unsupported(
                        "Initial Radiance profile requires RGBE, not XYZE/spectral data",
                    ));
                }
                format = true;
            }
            Some("EXPOSURE") => {
                header.applied_exposure =
                    multiply(header.applied_exposure, numbers::<1>(value)?[0])?
            }
            Some("COLORCORR") => {
                for (product, value) in header
                    .applied_color_correction
                    .iter_mut()
                    .zip(numbers::<3>(value)?)
                {
                    *product = multiply(*product, value)?;
                }
            }
            Some("PIXASPECT") => {
                header.pixel_aspect = multiply(header.pixel_aspect, numbers::<1>(value)?[0])?
            }
            Some("PRIMARIES") => {
                let primaries = numbers(value)?;
                if header
                    .primaries
                    .is_some_and(|previous| previous != primaries)
                {
                    header.primaries_conflict = true;
                }
                header.primaries = Some(primaries);
            }
            Some("GAMMA") => {
                if gamma || numbers::<1>(value)?[0] != 1.0 {
                    return Err(unsupported(
                        "Gamma-tagged Radiance samples need a qualified transfer policy",
                    ));
                }
                gamma = true;
            }
            _ => {} // Bounded borrowed provenance, never an executable command.
        }
        header
            .records
            .try_reserve(1)
            .map_err(|_| resource("Radiance metadata inventory allocation"))?;
        header.records.push(Record { key, value });
    }
    if !format {
        return Err(invalid("Missing Radiance FORMAT"));
    }
    let resolution = lines.line(check)?;
    let mut fields = resolution.split_ascii_whitespace();
    let major = fields
        .next()
        .ok_or_else(|| invalid("Missing Radiance resolution"))?;
    let height = fields
        .next()
        .ok_or_else(|| invalid("Missing Radiance height"))?;
    let minor = fields
        .next()
        .ok_or_else(|| invalid("Missing Radiance resolution axis"))?;
    let width = fields
        .next()
        .ok_or_else(|| invalid("Missing Radiance width"))?;
    if fields.next().is_some() {
        return Err(invalid("Extra Radiance resolution fields"));
    }
    if (major, minor) != ("-Y", "+X") {
        return Err(unsupported(
            "Initial Radiance profile requires -Y +X orientation",
        ));
    }
    if !width.bytes().all(|b| b.is_ascii_digit()) || !height.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid("Invalid Radiance pixel dimensions"));
    }
    header.width = width
        .parse()
        .map_err(|_| resource("Radiance width exceeds integer allowance"))?;
    header.height = height
        .parse()
        .map_err(|_| resource("Radiance height exceeds integer allowance"))?;
    if header.width == 0
        || header.height == 0
        || header.width > 65536
        || header.height > 65536
        || u64::from(header.width) * u64::from(header.height) > pixels.min(PIXELS_MAX)
    {
        return Err(resource("Radiance declared dimensions exceed allowance"));
    }
    header.data_offset = lines.offset;
    check()?;
    Ok(header)
}
