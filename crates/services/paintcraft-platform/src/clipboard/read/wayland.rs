//! Explicit typed Wayland offers feed the existing frozen encoded transport.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
use super::{EncodedImage, Reader, SharedBudget};
use crate::clipboard::Encoding;
#[cfg(unix)]
mod pipe;

fn acquire<R>(
    check: &dyn Fn() -> Result<(), String>,
    mut request: impl FnMut(Encoding) -> Result<Option<R>, String>,
) -> Result<(Encoding, R), String> {
    for encoding in [Encoding::Png, Encoding::Tiff] {
        check()?;
        if let Some(source) = request(encoding)? {
            return Ok((encoding, source));
        }
    }
    Err("Clipboard does not offer a supported PNG or TIFF image".into())
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(super) fn session_available() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|name| !name.is_empty())
}
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(super) struct NativeReader;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
impl Reader for NativeReader {
    fn read(
        &mut self,
        budget: &SharedBudget,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<EncodedImage, String> {
        use wl_clipboard_rs::paste::{self, ClipboardType, MimeType, Seat};
        EncodedImage::capture_stream(
            crate::clipboard::MAX_ENCODED_BYTES,
            budget,
            check,
            || {
                acquire(check, |encoding| {
                    match paste::get_contents(
                        ClipboardType::Regular,
                        Seat::Unspecified,
                        MimeType::Specific(encoding.mime()),
                    ) {
                        Ok((source, mime)) => {
                            if mime != encoding.mime() {
                                return Err(
                                    "Clipboard image MIME type changed during acquisition".into()
                                );
                            }
                            Ok(Some(pipe::Nonblocking::new(
                                source,
                                std::time::Duration::from_secs(30),
                            )?))
                        }
                        Err(paste::Error::NoMimeType) => Ok(None),
                        Err(error) => {
                            Err(format!("Cannot acquire Wayland clipboard image: {error}"))
                        }
                    }
                })
            },
            |source, out| source.read(out, check),
        )
    }
}

#[cfg(test)]
mod tests;
