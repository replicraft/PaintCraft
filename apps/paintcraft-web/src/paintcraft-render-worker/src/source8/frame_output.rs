//! Account precedes the old bare output allocator; no detached Vec/credit route.
use super::{
    frame_image::{self, RETAINED_CONTROL, Source8RgbaImage},
    frame_inputs::Source8FrameRealms,
    *,
};
use std::mem::size_of;
#[repr(C)]
pub(super) struct PendingOutput8 {
    width: u32,
    height: u32,
    account: BudgetAccount,
    factory: BudgetScope,
}
#[repr(C)]
struct PendingPixels8 {
    pixels: Vec<u8>,
    output: PendingOutput8,
}
impl PendingOutput8 {
    /// Caller must admit F and validate full source/frame/caller overlap first.
    /// No pixel allocation takes place here. The original O factory is retained.
    pub(super) fn admit(
        width: u32,
        height: u32,
        realms: &Source8FrameRealms<'_>,
        auth: &entry::EntryAuth8<'_>,
    ) -> Result<Self> {
        auth.spend(64)?;
        if !frame_image::layout_supported()
            || size_of::<PendingOutput8>() > 112
            || size_of::<PendingPixels8>() > 112
            || size_of::<Result<PendingOutput8>>() > 112
            || size_of::<Result<PendingPixels8>>() > 112
        {
            return Err(invalid());
        }
        let bytes = extent(width, height)?;
        let retained = bytes.checked_add(RETAINED_CONTROL).ok_or_else(invalid)?;
        if retained > realms.caps.output {
            return Err(invalid());
        }
        auth.spend(112)?;
        {
            let s = realms.output.capacity_snapshot();
            if !s.reserved || s.ceiling != realms.caps.output {
                return Err(invalid());
            }
        }
        // Account/factory transitions, admit-result moves, complete-result moves,
        // and one eventual output control/deallocation path, all BEFORE escape.
        auth.spend(12 + 384 + 896 + 64)?;
        let factory = realms.output.clone();
        let mut account = factory.account();
        account.resize(retained)?;
        Ok(Self {
            width,
            height,
            account,
            factory,
        })
    }
    pub(super) fn finish(
        self,
        pixels: Vec<u8>,
        auth: &entry::EntryAuth8<'_>,
    ) -> Result<Source8RgbaImage> {
        // Guard first, without a fallible operation between the argument move and
        // this local. Every later failure destroys pixels before O credit/factory.
        let pending = PendingPixels8 {
            pixels,
            output: self,
        };
        auth.spend(64)?;
        let bytes = extent(pending.output.width, pending.output.height)?;
        if pending.pixels.len() as u64 != bytes
            || pending.pixels.capacity() as u64 != bytes
            || pending.output.account.bytes()
                != bytes.checked_add(RETAINED_CONTROL).ok_or_else(invalid)?
        {
            return Err(invalid());
        }
        let PendingPixels8 { pixels, output } = pending;
        Ok(Source8RgbaImage {
            pixels,
            width: output.width,
            height: output.height,
            _account: output.account,
            _factory: output.factory,
        })
    }
}
fn extent(width: u32, height: u32) -> Result<u64> {
    if width == 0 || height == 0 {
        return Err(invalid());
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .filter(|n| *n <= paintcraft_render::RENDER_PIXELS_MAX)
        .ok_or_else(invalid)?;
    pixels.checked_mul(4).ok_or_else(invalid)
}
fn invalid() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Source8 frame output declaration mismatch",
    )
}
