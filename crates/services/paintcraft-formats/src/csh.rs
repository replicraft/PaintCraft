//! Checked Photoshop custom-shape resources, separate from image documents.
//!
//! Coordinates retain their signed 8.24 wire values, rather than a pixel or
//! floating-point approximation. The selected codec is invoked only after
//! complete structural validation. Public asset workflows are not enabled yet.
mod preflight;
#[cfg(test)]
mod tests;
mod verify;

use paintcraft_core::types::{Error, ErrorCode, Result};

pub const BYTES_MAX: usize = 4 << 20;
pub const SHAPES_MAX: usize = 256;
pub const RECORDS_MAX: usize = 65536;
pub const NAME_UNITS_MAX: usize = 1024;
pub const NAME_BYTES_MAX: usize = 256 << 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Exclude,
    Combine,
    Subtract,
    Intersect,
}

/// Ordered records retain path topology and the document-level fill records
/// that the selected backend otherwise omits from its public representation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Record {
    Subpath {
        open: bool,
        knots: u16,
        operation: Option<Operation>,
        nonzero_fill: bool,
    },
    Knot {
        open: bool,
        linked: bool,
        /// x0, y0, x1, y1, x2, y2 in signed normalized 8.24 units.
        points: [i32; 6],
    },
    FillRule(u16),
    /// top, left, bottom, right, resolution in signed 8.24 units.
    Clipboard([i32; 5]),
    InitialFill(bool),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    pub name: String,
    pub id: String,
    /// Original top, left, bottom, right bounds, including the origin.
    pub bounds: [u32; 4],
    pub records: Vec<Record>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Library {
    shapes: Vec<Shape>,
}
impl Library {
    pub fn shapes(&self) -> &[Shape] {
        &self.shapes
    }
}

pub fn decode(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Library> {
    let shapes = preflight::inspect(bytes, check)?;
    check()?;
    let decoded =
        ag_psd::csh::read_csh(bytes).map_err(|e| invalid(&format!("CSH decoding failed: {e}")))?;
    check()?;
    if decoded.shapes.len() != shapes.len() {
        return Err(invalid("CSH backend shape count disagrees with preflight"));
    }
    for (source, parsed) in shapes.iter().zip(&decoded.shapes) {
        check()?;
        if source.name != parsed.name
            || source.id != parsed.id
            || source.bounds[3] - source.bounds[1] != parsed.width
            || source.bounds[2] - source.bounds[0] != parsed.height
            || source
                .records
                .iter()
                .filter(|r| matches!(r, Record::Subpath { .. }))
                .count()
                != parsed.mask.paths.len()
        {
            return Err(invalid("CSH backend resource disagrees with preflight"));
        }
        verify::shape(source, parsed, check)?;
    }
    Ok(Library { shapes })
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
