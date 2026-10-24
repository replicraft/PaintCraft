//! Leaf types shared by every PaintCraft crate. No other PaintCraft dependencies or I/O implementations.

pub mod error;
pub mod geometry;
pub mod json;
pub mod pixel;

pub use error::{Error, ErrorCode, Result};
pub use geometry::Rect;
pub use pixel::Rgba;

pub mod ids;
pub mod policy;

pub mod coordinates;

pub mod shapes;
pub mod text;

pub mod storage;

pub mod resources;
