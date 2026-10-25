//! Raw advertised PNG/TIFF only; no image decoder is invoked by this adapter.
use super::*;
use crate::clipboard::{Encoding, MAX_ENCODED_BYTES};
use objc2::rc::autoreleasepool;
use objc2_app_kit::NSPasteboard;
use objc2_foundation::{NSCopying, NSString};

pub(super) struct NativeReader;
impl Reader for NativeReader {
    fn read(
        &mut self,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<EncodedImage, String> {
        autoreleasepool(|_| capture_board(&NSPasteboard::generalPasteboard(), budget, check))
    }
}
fn capture_board(
    board: &NSPasteboard,
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<EncodedImage, String> {
    check()?;
    let generation = board.changeCount();
    let types = board
        .types()
        .ok_or_else(|| "Clipboard contains no image types".to_owned())?;
    if types.count() > 256 {
        return Err("Clipboard advertised type count exceeds the limit".into());
    }
    for encoding in [Encoding::Png, Encoding::Tiff] {
        let mime = NSString::from_str(match encoding {
            Encoding::Png => "public.png",
            Encoding::Tiff => "public.tiff",
        });
        // Ask only for an advertised type, avoiding native image conversion.
        if !types.containsObject(&mime) {
            continue;
        }
        let source = board
            .dataForType(&mime)
            .ok_or_else(|| "Clipboard image disappeared during read".to_owned())?;
        let length = source.len();
        if length == 0 || length as u64 > MAX_ENCODED_BYTES {
            return Err("Clipboard encoded image exceeds the limit".into());
        }
        let image = EncodedImage::capture(encoding, length, budget, check, || {
            // NSCopying freezes mutable NSData subclasses. Copying the bounded
            // native snapshot through Foundation's safe API needs no Rust FFI.
            let frozen = source.copy();
            if frozen.len() != length {
                return Err("Clipboard image changed during read".into());
            }
            Ok(frozen.to_vec())
        })?;
        check()?;
        if board.changeCount() != generation {
            return Err("Clipboard image changed during read".into());
        }
        return Ok(image);
    }
    Err("Clipboard does not contain an advertised PNG or TIFF image".into())
}

#[cfg(test)]
mod tests;
