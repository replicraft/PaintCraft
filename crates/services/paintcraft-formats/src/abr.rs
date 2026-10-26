//! Photoshop brush resources, separate from image documents.
//!
//! Complete structural admission precedes the selected ag-psd reader. Original
//! section bytes and full-precision sample masks remain available even where
//! its convenience brush model omits descriptor fields or narrows alpha.
mod descriptor;
mod normalize;
mod preflight;
#[cfg(test)]
mod tests;

use paintcraft_core::types::{Error, ErrorCode, Result};

pub const BYTES_MAX: usize = 16 << 20;
/// Private raw-16 backend views may expand compressed source masks. This is a
/// byte admission allowance, not a measured process/allocator memory bound.
pub const NORMALIZED_BYTES_MAX: usize = 48 << 20;
pub const SECTIONS_MAX: usize = 64;
pub const SAMPLES_MAX: usize = 256;
pub const SAMPLE_PIXELS_MAX: usize = 4 << 20;
pub const TOTAL_PIXELS_MAX: usize = 16 << 20;
pub const DIMENSION_MAX: usize = 16384;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Alpha {
    U8(Vec<u8>),
    U16(Vec<u16>),
}
impl Alpha {
    pub fn len(&self) -> usize {
        match self {
            Self::U8(v) => v.len(),
            Self::U16(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    pub id: String,
    /// Original top, left, bottom, right coordinates, including negative origins.
    pub bounds: [i32; 4],
    pub alpha: Alpha,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionKind {
    Samples,
    Descriptors,
    Hierarchy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub kind: SectionKind,
    /// Exact payload, including fields not surfaced by the selected backend.
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Library {
    version: u16,
    minor_version: u16,
    samples: Vec<Sample>,
    brushes: Vec<ag_psd::abr::Brush>,
    sections: Vec<Section>,
}
impl Library {
    pub fn version(&self) -> (u16, u16) {
        (self.version, self.minor_version)
    }
    pub fn samples(&self) -> &[Sample] {
        &self.samples
    }
    /// The backend's complete parsed brush model. Raw descriptors remain in
    /// `sections` and must accompany any future adoption policy.
    pub fn brushes(&self) -> &[ag_psd::abr::Brush] {
        &self.brushes
    }
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }
}

pub fn decode(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Library> {
    let prepared = preflight::inspect(bytes, check)?;
    let backend_view = normalize::view(bytes, &prepared, check)?;
    check()?;
    let decoded = ag_psd::abr::read_abr(&backend_view, &Default::default())
        .map_err(|e| invalid(&format!("ABR decoding failed: {e}")))?;
    drop(backend_view);
    check()?;
    if decoded.samples.len() != prepared.samples.len() || !decoded.patterns.is_empty() {
        return Err(invalid(
            "ABR backend sample inventory disagrees with preflight",
        ));
    }
    for (source, parsed) in prepared.samples.iter().zip(&decoded.samples) {
        check()?;
        let [top, left, bottom, right] = source.bounds;
        if source.id != parsed.id
            || parsed.bounds.x != left
            || parsed.bounds.y != top
            || parsed.bounds.w != right - left
            || parsed.bounds.h != bottom - top
            || parsed.alpha.len() != source.alpha.len()
        {
            return Err(invalid(
                "ABR backend sample metadata disagrees with preflight",
            ));
        }
        let matches = match &source.alpha {
            Alpha::U8(v) => *v == parsed.alpha,
            Alpha::U16(v) => v
                .iter()
                .zip(&parsed.alpha)
                .all(|(a, b)| (a >> 8) as u8 == *b),
        };
        if !matches {
            return Err(invalid("ABR backend sample pixels disagree with preflight"));
        }
    }
    Ok(Library {
        version: prepared.version,
        minor_version: prepared.minor_version,
        samples: prepared.samples,
        brushes: decoded.brushes,
        sections: prepared.sections,
    })
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
