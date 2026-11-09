//! Independent final RGBA ownership. Never a source/Document/Arc capability.
use super::*;
use std::{fmt, mem::size_of};
pub(super) const RETAINED_CONTROL: u64 = 112;
#[repr(C)]
pub struct Source8RgbaImage {
    pub(super) pixels: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) _account: BudgetAccount,
    pub(super) _factory: BudgetScope,
}
impl Source8RgbaImage {
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn rgba(&self) -> &[u8] {
        &self.pixels
    }
    pub fn byte_len(&self) -> usize {
        self.pixels.len()
    }
}
impl fmt::Debug for Source8RgbaImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Source8RgbaImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bytes", &self.pixels.len())
            .finish_non_exhaustive()
    }
}
pub(super) fn layout_supported() -> bool {
    let expected = if cfg!(target_pointer_width = "64") {
        104
    } else if cfg!(target_arch = "wasm32") {
        64
    } else {
        return false;
    };
    size_of::<Source8RgbaImage>() == expected
        && size_of::<Source8RgbaImage>() <= RETAINED_CONTROL as usize
        && size_of::<Result<Source8RgbaImage>>() <= RETAINED_CONTROL as usize
}
