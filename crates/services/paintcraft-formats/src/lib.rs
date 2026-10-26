//! Serialization boundaries: native projects and explicitly scoped raster codecs.

pub mod abr;
pub mod ase;
pub mod avif;
pub mod csh;
pub mod dcx;
pub mod dds;
pub mod export;
pub mod heif;
pub mod icons;
pub mod image;
pub mod jxl;
pub mod jxr;
pub mod ora;
pub mod pcnd;
pub mod pcx;
pub mod png;
pub mod project_import;
pub mod psd;
#[cfg(not(target_arch = "wasm32"))]
pub mod recovery;
pub mod registry;
#[cfg(not(target_arch = "wasm32"))]
pub mod save_recovery;
pub mod svg;
#[cfg(not(target_arch = "wasm32"))]
pub use paintcraft_platform::save;
