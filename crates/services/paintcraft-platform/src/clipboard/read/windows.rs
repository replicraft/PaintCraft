//! Frozen encoded image transport; all DIB layout/pixel decoding stays in formats.
use super::*;
use crate::clipboard::Encoding;

trait Snapshot {
    fn formats(&mut self) -> Result<Vec<u32>, String>;
    fn generation(&self) -> Result<u32, String>;
    fn length(&mut self, format: u32) -> Result<usize, String>;
    fn copy(&mut self, format: u32, bytes: &mut [u8]) -> Result<usize, String>;
}
fn capture(
    board: &mut impl Snapshot,
    preferred: &[(u32, Encoding)],
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<EncodedImage, String> {
    check()?;
    let formats = board.formats()?;
    if formats.len() > 256 {
        return Err("Clipboard advertised type count exceeds the limit".into());
    }
    let &(format, encoding) = preferred
        .iter()
        .find(|(id, _)| formats.contains(id))
        .ok_or("Clipboard does not contain an advertised supported image")?;
    let length = board.length(format)?;
    // GetClipboardData may realize a delayed format. Pin its generation after
    // the initial size query, not before legitimate provider rendering.
    let generation = board.generation()?;
    let image = EncodedImage::capture(encoding, length, budget, check, || {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| "Cannot allocate clipboard snapshot".to_owned())?;
        bytes.resize(length, 0);
        check()?;
        if board.copy(format, &mut bytes)? != length
            || board.length(format)? != length
            || board.generation()? != generation
        {
            return Err("Clipboard image changed during read".into());
        }
        Ok(bytes)
    })?;
    check()?;
    if board.generation()? != generation {
        return Err("Clipboard image changed during read".into());
    }
    Ok(image)
}

#[cfg(target_os = "windows")]
pub(super) use native::NativeReader;
#[cfg(target_os = "windows")]
mod native {
    use super::*;
    use clipboard_win::{Clipboard, raw};
    pub(crate) struct NativeReader;
    struct Board;
    impl Snapshot for Board {
        fn formats(&mut self) -> Result<Vec<u32>, String> {
            Ok(raw::EnumFormats::new().take(257).collect())
        }
        fn generation(&self) -> Result<u32, String> {
            raw::seq_num()
                .map(|value| value.get())
                .ok_or_else(|| "Cannot identify clipboard generation".into())
        }
        fn length(&mut self, format: u32) -> Result<usize, String> {
            raw::size(format)
                .map(|value| value.get())
                .ok_or_else(|| "Clipboard image disappeared during read".into())
        }
        fn copy(&mut self, format: u32, bytes: &mut [u8]) -> Result<usize, String> {
            raw::get(format, bytes).map_err(|error| format!("Cannot copy clipboard image: {error}"))
        }
    }
    impl Reader for NativeReader {
        fn read(
            &mut self,
            budget: &SharedBudget,
            check: &dyn Fn() -> Result<(), String>,
        ) -> Result<EncodedImage, String> {
            check()?;
            // The guard keeps the clipboard open through size checks and copying,
            // and closes it on every error/panic path. No writes are performed.
            let _clipboard =
                Clipboard::new().map_err(|error| format!("Cannot open clipboard: {error}"))?;
            let png = raw::register_format("PNG")
                .ok_or("Cannot register PNG clipboard format")?
                .get();
            let mime_png = raw::register_format("image/png")
                .ok_or("Cannot register PNG clipboard format")?
                .get();
            capture(
                &mut Board,
                &[
                    (png, Encoding::Png),
                    (mime_png, Encoding::Png),
                    (clipboard_win::formats::CF_DIBV5, Encoding::DibV5),
                    (clipboard_win::formats::CF_DIB, Encoding::Dib),
                    (clipboard_win::formats::CF_TIFF, Encoding::Tiff),
                ],
                budget,
                check,
            )
        }
    }
}

#[cfg(test)]
mod tests;
