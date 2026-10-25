//! Serialization boundaries: native projects and explicitly scoped raster codecs.

pub mod dds;
pub mod avif;
pub mod export;
pub mod image;
pub mod jxl;
pub mod pcnd;
pub mod png;
#[cfg(not(target_arch = "wasm32"))]
pub mod recovery;
pub mod registry;
#[cfg(not(target_arch = "wasm32"))]
pub mod save_recovery;
#[cfg(not(target_arch = "wasm32"))]
pub use paintcraft_platform::save;
pub mod zip;
