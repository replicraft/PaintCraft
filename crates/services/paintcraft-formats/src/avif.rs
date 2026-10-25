//! Bounded encoder-only AVIF adapter; frontend admission is owned by the registry.
//!
//! The selected image/ravif path is lossy even at quality 100. Its public API
//! cleans invisible RGB and uses the selected quality for alpha too. Input and
//! returned bytes have hard ceilings; codec-owned scratch is not an RSS budget.
use image::ImageEncoder;
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::io::{self, Write};

pub const ENCODE_PIXELS_MAX: u64 = 1_048_576;
pub const ENCODE_SIDE_MAX: u32 = 4096;
pub const ENCODE_BYTES_MAX: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub struct Encoded {
    pub bytes: Vec<u8>,
    pub losses: Vec<&'static str>,
}

/// Exact format losses after classifying source alpha and invisible RGB.
/// Shared by the pre-publication export policy and the selected encoder.
pub(crate) fn export_losses(has_alpha: bool, has_hidden_rgb: bool) -> Vec<&'static str> {
    let mut losses = vec!["lossy-avif"];
    if has_alpha {
        losses.push("alpha-lossy");
    }
    if has_hidden_rgb {
        losses.push("transparent-rgb-discarded");
    }
    losses
}

struct Output<'a> {
    bytes: Vec<u8>,
    failure: Option<Error>,
    check: &'a dyn Fn() -> Result<()>,
}
impl Write for Output<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = (|| {
            (self.check)()?;
            if self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|n| n > ENCODE_BYTES_MAX)
            {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "AVIF output exceeds byte budget",
                ));
            }
            self.bytes.try_reserve_exact(bytes.len()).map_err(|_| {
                Error::new(ErrorCode::ResourceLimit, "AVIF output allocation failed")
            })?;
            if self.bytes.capacity() > ENCODE_BYTES_MAX {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "AVIF output allocation exceeds byte budget",
                ));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        })();
        result.map_err(|error| {
            self.failure = Some(error);
            io::Error::other("AVIF output admission failed")
        })
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A single RGBA8 still image, fixed fast mode, one worker, identity RGB.
/// Cancellation is checked around input scanning and publication, including the
/// output write; the opaque AV1 encode itself cannot be interrupted by this API.
pub fn encode_rgba8(
    width: u32,
    height: u32,
    rgba: &[u8],
    quality: u8,
    check: &dyn Fn() -> Result<()>,
) -> Result<Encoded> {
    check()?;
    if width == 0 || height == 0 || !(1..=100).contains(&quality) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "AVIF needs nonzero dimensions and quality 1..100",
        ));
    }
    let pixels = u64::from(width) * u64::from(height);
    if width > ENCODE_SIDE_MAX || height > ENCODE_SIDE_MAX || pixels > ENCODE_PIXELS_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "AVIF encoder exceeds qualified dimensions",
        ));
    }
    if rgba.len() as u64 != pixels * 4 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "AVIF RGBA buffer does not match dimensions",
        ));
    }
    let mut alpha = false;
    let mut hidden = false;
    for chunk in rgba.chunks(4096) {
        check()?;
        for sample in chunk.as_chunks::<4>().0 {
            alpha |= sample[3] != 255;
            hidden |= sample[3] == 0 && sample[..3] != [0, 0, 0];
        }
    }
    let losses = export_losses(alpha, hidden);
    let mut output = Output {
        bytes: Vec::new(),
        failure: None,
        check,
    };
    let result = image::codecs::avif::AvifEncoder::new_with_speed_quality(&mut output, 10, quality)
        .with_colorspace(image::codecs::avif::ColorSpace::Srgb)
        .with_num_threads(Some(1))
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8);
    if let Some(error) = output.failure {
        return Err(error);
    }
    result.map_err(|error| {
        let code = match &error {
            image::ImageError::Limits(_) => ErrorCode::ResourceLimit,
            image::ImageError::Unsupported(_) => ErrorCode::CapabilityUnsupported,
            _ => ErrorCode::CodecFailed,
        };
        Error::new(code, error.to_string())
    })?;
    check()?;
    if output.bytes.get(4..8) != Some(b"ftyp") {
        return Err(Error::new(
            ErrorCode::CodecFailed,
            "AVIF encoder returned an invalid container",
        ));
    }
    Ok(Encoded {
        bytes: output.bytes,
        losses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn invalid_inputs_and_cancellation_fail_before_publication() {
        for (w, h, data, q, code) in [
            (0, 1, &[][..], 90, ErrorCode::InputSchema),
            (1, 1, &[0; 3][..], 90, ErrorCode::InputSchema),
            (1, 1, &[0; 4][..], 0, ErrorCode::InputSchema),
            (4097, 1, &[][..], 90, ErrorCode::ResourceLimit),
            (4096, 4096, &[][..], 90, ErrorCode::ResourceLimit),
        ] {
            assert_eq!(
                encode_rgba8(w, h, data, q, &|| Ok(())).unwrap_err().code,
                code
            );
        }
        for cancel_at in [1, 2, 3, 4] {
            let calls = Cell::new(0);
            let error = encode_rgba8(1, 1, &[10, 20, 30, 128], 90, &|| {
                calls.set(calls.get() + 1);
                if calls.get() == cancel_at {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancelled"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
        }
    }
    #[test]
    fn quality_100_is_not_advertised_as_lossless_and_raw_input_is_immutable() {
        let samples = [1, 2, 3, 0, 4, 5, 6, 128, 7, 8, 9, 255];
        let encoded = encode_rgba8(3, 1, &samples, 100, &|| Ok(())).unwrap();
        assert_eq!(
            encoded.losses,
            ["lossy-avif", "alpha-lossy", "transparent-rgb-discarded"]
        );
        assert_eq!(samples, [1, 2, 3, 0, 4, 5, 6, 128, 7, 8, 9, 255]);
        assert!(encoded.bytes.len() < ENCODE_BYTES_MAX);
        assert!(encoded.bytes.windows(4).any(|chunk| chunk == b"avif"));
        assert_eq!(
            encode_rgba8(1, 1, &[10, 20, 30, 255], 100, &|| Ok(()))
                .unwrap()
                .losses,
            ["lossy-avif"]
        );
    }
    #[test]
    fn output_limit_and_cancelled_write_do_not_publish_partial_bytes() {
        let mut output = Output {
            bytes: Vec::new(),
            failure: None,
            check: &|| Ok(()),
        };
        output.write_all(b"private").unwrap();
        assert!(output.write_all(&vec![0; ENCODE_BYTES_MAX]).is_err());
        assert_eq!(
            output.failure.as_ref().unwrap().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(output.bytes, b"private");
    }
}
