//! Bounded lossless RGBA8 writers. Netpbm writes PAM only; farbfeld widens samples.
use super::*;
use std::io::{self, Write};

pub const PIXELS_MAX: u64 = 4_194_304;
const BYTES_MAX: usize = 64 << 20;

struct Output<'a> {
    bytes: Vec<u8>,
    failure: Option<Error>,
    check: &'a dyn Fn() -> Result<()>,
}
impl<'a> Output<'a> {
    fn new(capacity: usize, check: &'a dyn Fn() -> Result<()>) -> Result<Self> {
        if capacity > BYTES_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "extended raster output exceeds byte budget",
            ));
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(capacity).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "cannot allocate extended raster output",
            )
        })?;
        if bytes.capacity() > BYTES_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "extended raster output capacity exceeds byte budget",
            ));
        }
        Ok(Self {
            bytes,
            failure: None,
            check,
        })
    }
    fn finish(self, result: std::result::Result<(), ::image::ImageError>) -> Result<Vec<u8>> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        result.map_err(image_error)?;
        (self.check)()?;
        Ok(self.bytes)
    }
}
impl Write for Output<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = (|| {
            (self.check)()?;
            let length = self
                .bytes
                .len()
                .checked_add(bytes.len())
                .filter(|n| *n <= BYTES_MAX)
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::ResourceLimit,
                        "extended raster output exceeds byte budget",
                    )
                })?;
            if length > self.bytes.capacity() {
                self.bytes
                    .try_reserve_exact(length - self.bytes.len())
                    .map_err(|_| {
                        Error::new(
                            ErrorCode::ResourceLimit,
                            "cannot grow extended raster output",
                        )
                    })?;
                if self.bytes.capacity() > BYTES_MAX {
                    return Err(Error::new(
                        ErrorCode::ResourceLimit,
                        "extended raster capacity exceeds byte budget",
                    ));
                }
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        })();
        result.map_err(|error| {
            self.failure = Some(error);
            io::Error::other("extended raster output admission failed")
        })
    }
    fn flush(&mut self) -> io::Result<()> {
        (self.check)().map_err(|error| {
            self.failure = Some(error);
            io::Error::other("extended raster output cancelled")
        })
    }
}
pub(crate) fn encode(
    format: ImageFormat,
    width: u32,
    height: u32,
    rgba: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    check()?;
    let count = admit_dimensions(width, height, PIXELS_MAX)?;
    if rgba.len() as u64 != count * 4 {
        return Err(codec(
            "extended raster RGBA buffer does not match dimensions",
        ));
    }
    match format {
        ImageFormat::Qoi => {
            let encoder =
                qoi::Encoder::new(rgba, width, height).map_err(|e| codec(e.to_string()))?;
            let maximum = encoder.required_buf_len();
            if maximum > BYTES_MAX {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "QOI worst-case output exceeds byte budget",
                ));
            }
            let mut output = buffer(maximum as u64)?;
            check()?;
            // The codec's inner encode loop has no cancellation callback. Keep
            // its output private and check again before returning any bytes.
            let length = encoder
                .encode_to_buf(&mut output)
                .map_err(|e| codec(e.to_string()))?;
            check()?;
            output.truncate(length);
            Ok(output)
        }
        ImageFormat::Pnm => {
            let mut output = Output::new(rgba.len() + 256, check)?;
            let result = ::image::codecs::pnm::PnmEncoder::new(&mut output)
                .with_subtype(::image::codecs::pnm::PnmSubtype::ArbitraryMap)
                .write_image(rgba, width, height, ::image::ExtendedColorType::Rgba8);
            output.finish(result)
        }
        ImageFormat::Farbfeld => {
            let mut widened = buffer(count * 8)?;
            for (source, destination) in rgba.chunks(4096).zip(widened.chunks_mut(8192)) {
                check()?;
                for (sample, pair) in source.iter().zip(destination.as_chunks_mut::<2>().0) {
                    // x*257 is [x,x] in either native endian, as required by image.
                    *pair = [*sample, *sample];
                }
            }
            let mut output = Output::new(widened.len() + 16, check)?;
            let result = ::image::codecs::farbfeld::FarbfeldEncoder::new(&mut output).write_image(
                &widened,
                width,
                height,
                ::image::ExtendedColorType::Rgba16,
            );
            output.finish(result)
        }
        _ => Err(unsupported("not an extended raster writer")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_output_retains_failure_without_exposing_partial_bytes() {
        assert_eq!(
            Output::new(BYTES_MAX + 1, &|| Ok(())).err().unwrap().code,
            ErrorCode::ResourceLimit
        );
        let mut output = Output::new(8, &|| Ok(())).unwrap();
        output.write_all(b"private").unwrap();
        assert!(output.write_all(&vec![0; BYTES_MAX]).is_err());
        assert_eq!(output.bytes, b"private");
        assert_eq!(
            output.finish(Ok(())).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
}
