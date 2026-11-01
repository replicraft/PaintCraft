//! Narrow bounded PNG/JPEG/BMP adapter for admitted Source6 foreign copies.
//! Existing image encoding facades and codec policies remain unchanged.
use super::*;
use paintcraft_core::types::resources::{BudgetAccount, BudgetScope};
use std::io::{self, Write};
pub const OUTPUT_MAX: u64 = 16 << 20;
pub const SCRATCH_MAX: u64 = 32 << 20;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quote {
    pub output_bytes: u64,
    pub scratch_bytes: u64,
    pub base_work: u64,
}
/// Logical codec payload includes a conservative16MiB opaque codec allowance
/// and JPEG's exact RGB conversion bound. It is not an allocator/RSS assertion.
pub fn quote(format: ImageFormat, width: u32, height: u32) -> Result<Quote> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(limit)?;
    if width == 0 || height == 0 || pixels > 4_194_304 || width > 8192 || height > 8192 {
        return Err(limit());
    }
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Bmp
    ) {
        return Err(unsupported(
            "This codec has no bounded Source6 copy adapter",
        ));
    }
    if format == ImageFormat::Jpeg && (width > 65535 || height > 65535) {
        return Err(unsupported("JPEG supports at most65535 pixels per side"));
    }
    Ok(Quote {
        output_bytes: OUTPUT_MAX,
        scratch_bytes: (16 << 20)
            + if format == ImageFormat::Jpeg {
                pixels * 3
            } else {
                0
            },
        // A distinct finite codec action model, not renderer/Noise units or a
        // claim that compression instructions/cycles are bounded by this count.
        base_work: pixels.checked_mul(64).ok_or_else(limit)?,
    })
}
/// Storage precedes account so its final drop always precedes release callbacks.
#[derive(Debug)]
pub struct EncodedCopy {
    bytes: Vec<u8>,
    account: BudgetAccount,
}
impl EncodedCopy {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// The recipient must retain the account through its last Rust-owned bytes.
    /// The Wasm caller may retain it through synchronous bindgen conversion.
    pub fn into_parts(self) -> (Vec<u8>, BudgetAccount) {
        (self.bytes, self.account)
    }
}
struct Sink {
    encoded: EncodedCopy,
    failed: bool,
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(end) = self.encoded.bytes.len().checked_add(bytes.len()) else {
            self.failed = true;
            return Err(io::Error::other("bounded output"));
        };
        if end > self.encoded.bytes.capacity() {
            self.failed = true;
            return Err(io::Error::other("bounded output"));
        }
        // Complete capacity was allocated and reconciled BEFORE invoking codec.
        self.encoded.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Bounded copy payload exceeds its declared allowance",
    )
}
/// Caller pre-admits these accounts before creating any frame/transfer buffers.
/// A codec that exceeds16MiB is refused during its write, never postchecked after
/// unconstrained Vec growth. Codec cancellation is before/after opaque loops.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub quality: u8,
    pub matte: JpegMatte,
    pub output_limit: u64,
}
pub fn encode(
    options: Options,
    rgba: &[u8],
    scratch_scope: &BudgetScope,
    output_scope: &BudgetScope,
    check: &dyn Fn() -> Result<()>,
) -> Result<EncodedCopy> {
    let q = quote(options.format, options.width, options.height)?;
    let mut scratch = scratch_scope.account();
    scratch.resize(q.scratch_bytes)?;
    let mut output = output_scope.account();
    output.resize(q.output_bytes)?;
    encode_with_accounts(options, rgba, scratch, output, check)
}
/// The Source6 action path reserves both accounts BEFORE mask transfer/render.
pub fn encode_with_accounts(
    options: Options,
    rgba: &[u8],
    scratch: BudgetAccount,
    account: BudgetAccount,
    check: &dyn Fn() -> Result<()>,
) -> Result<EncodedCopy> {
    let Options {
        format,
        width,
        height,
        quality,
        matte,
        output_limit,
    } = options;
    let q = quote(format, width, height)?;
    if quality == 0
        || quality > 100
        || rgba.len() as u64 != u64::from(width) * u64::from(height) * 4
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Bounded copy geometry/quality",
        ));
    }
    if output_limit == 0 || output_limit > OUTPUT_MAX {
        return Err(limit());
    }
    if scratch.bytes() < q.scratch_bytes || account.bytes() != q.output_bytes {
        return Err(limit());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(output_limit as usize)
        .map_err(|_| limit())?;
    if bytes.capacity() as u64 > output_limit {
        return Err(limit());
    }
    let mut sink = Sink {
        encoded: EncodedCopy { bytes, account },
        failed: false,
    };
    check()?;
    let encoded = match format {
        ImageFormat::Png => ::image::codecs::png::PngEncoder::new(&mut sink).write_image(
            rgba,
            width,
            height,
            ::image::ExtendedColorType::Rgba8,
        ),
        ImageFormat::Bmp => ::image::codecs::bmp::BmpEncoder::new(&mut sink).encode(
            rgba,
            width,
            height,
            ::image::ExtendedColorType::Rgba8,
        ),
        ImageFormat::Jpeg => {
            let rgb = super::jpeg_encode::prepare(
                width,
                height,
                rgba,
                matte,
                (u64::from(width) * u64::from(height) * 3) as usize,
                check,
            )?;
            if rgb.capacity() as u64 > u64::from(width) * u64::from(height) * 3 {
                return Err(limit());
            }
            ::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut sink, quality).encode(
                &rgb,
                width,
                height,
                ::image::ExtendedColorType::Rgb8,
            )
        }
        _ => return Err(unsupported("Bounded copy codec")),
    };
    if sink.failed {
        return Err(limit());
    }
    encoded.map_err(image_error)?;
    check()?;
    // All opaque codec state and any RGB Vec have dropped before scratch release.
    drop(scratch);
    Ok(sink.encoded)
}
#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
    #[test]
    fn bounded_actual_rust_codecs_retain_output_account_and_release_after_bytes() {
        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Bmp] {
            let budget = SharedBudget::new(64 << 20);
            let scope = budget.scope(AllocationClass::SharedBudget);
            let out = encode(
                Options {
                    format,
                    width: 2,
                    height: 1,
                    quality: 90,
                    matte: JpegMatte::new([32, 160, 224]),
                    output_limit: OUTPUT_MAX,
                },
                &[255, 0, 0, 255, 0, 200, 0, 255],
                &scope,
                &scope,
                &|| Ok(()),
            )
            .unwrap();
            assert!(!out.bytes().is_empty());
            let decoded = super::super::decode(out.bytes()).unwrap();
            assert_eq!((decoded.width, decoded.height), (2, 1));
            if format != ImageFormat::Jpeg {
                assert_eq!(decoded.rgba, [255, 0, 0, 255, 0, 200, 0, 255]);
            }
            assert_eq!(budget.usage().reserved, OUTPUT_MAX);
            drop(out);
            assert_eq!(budget.usage().reserved, 0);
        }
    }
    #[test]
    fn invalid_geometry_unpriced_codec_and_low_accounts_refuse_before_callbacks() {
        let budget = SharedBudget::new(1);
        let scope = budget.scope(AllocationClass::SharedBudget);
        let count = std::cell::Cell::new(0);
        let check = || {
            count.set(count.get() + 1);
            Ok(())
        };
        for (format, w, h) in [
            (ImageFormat::Png, 0, 1),
            (ImageFormat::WebP, 1, 1),
            (ImageFormat::Png, 1, 1),
        ] {
            assert!(
                encode(
                    Options {
                        format,
                        width: w,
                        height: h,
                        quality: 90,
                        matte: JpegMatte::WHITE,
                        output_limit: OUTPUT_MAX
                    },
                    &[0; 4],
                    &scope,
                    &scope,
                    &check
                )
                .is_err()
            );
        }
        assert_eq!(count.get(), 0);
        assert_eq!(budget.usage().reserved, 0);
    }
    #[test]
    fn sink_limit_is_enforced_before_any_output_growth() {
        let budget = SharedBudget::new(8);
        let scope = budget.scope(AllocationClass::SharedBudget);
        let mut account = scope.account();
        account.resize(2).unwrap();
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(2).unwrap();
        let mut sink = Sink {
            encoded: EncodedCopy { bytes, account },
            failed: false,
        };
        sink.write_all(&[1, 2]).unwrap();
        assert!(sink.write_all(&[3]).is_err());
        assert_eq!(sink.encoded.bytes(), [1, 2]);
        assert!(sink.failed);
        drop(sink);
        assert_eq!(budget.usage().reserved, 0);
    }
}
