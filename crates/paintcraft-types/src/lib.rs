//! Leaf types shared by every PaintCraft crate. No dependencies, no I/O.

pub mod error;
pub mod geometry;
pub mod json;
pub mod pixel;

pub use error::{Error, ErrorCode, Result};
pub use geometry::Rect;
pub use pixel::Rgba;
