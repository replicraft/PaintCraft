//! Decode only inside the executable's fixed allocator/process boundary.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::heif::{admission, wire};
use std::io::{Read, Write};
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn decoder_error(error: heic_decoder::HeifError) -> Error {
    match error {
        heic_decoder::HeifError::Unsupported(reason) => {
            Error::new(ErrorCode::CapabilityUnsupported, reason)
        }
        heic_decoder::HeifError::LimitExceeded(reason) => {
            Error::new(ErrorCode::ResourceLimit, reason)
        }
        _ => invalid("isolated HEVC frame decoder refused input"),
    }
}
fn plane(samples: &[i32]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.try_reserve_exact(samples.len())
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "native plane allocation"))?;
    for sample in samples {
        out.push(u8::try_from(*sample).map_err(|_| invalid("native sample outside gray8"))?);
    }
    Ok(out)
}
fn decode(header: wire::Header, source: &[u8]) -> Result<Vec<u8>> {
    header.verify_source(source)?;
    let admission = admission::admit_gray8(source, u64::from(header.pixels), &|| Ok(()))?;
    let image = heic_decoder::decode_with(
        admission.source(),
        &heic_decoder::DecodeOptions {
            apply_transforms: false,
        },
    )
    .map_err(decoder_error)?;
    let expected = admission.size();
    let pixels = u64::from(expected.0) * u64::from(expected.1);
    if (image.width as u64, image.height as u64) != (u64::from(expected.0), u64::from(expected.1))
        || image.bit_depth_luma != 8
        || image.chroma_format_idc != 0
        || !image.cb.is_empty()
        || !image.cr.is_empty()
        || image.y.len() as u64 != pixels
        || image.alpha.is_some() != admission.alpha()
    {
        return Err(invalid(
            "decoded native geometry/profile differs from admission",
        ));
    }
    let gray = plane(&image.y)?;
    let alpha = image
        .alpha
        .as_ref()
        .map(|alpha| {
            if alpha.bit_depth != 8 || alpha.premultiplied || alpha.samples.len() as u64 != pixels {
                return Err(invalid("decoded native alpha differs from admission"));
            }
            plane(&alpha.samples)
        })
        .transpose()?;
    let encoding = admission.encoding();
    wire::success(
        header,
        wire::Success {
            width: expected.0,
            height: expected.1,
            gray: &gray,
            alpha: alpha.as_deref(),
            encoding: wire::NativeEncoding {
                primaries: encoding.primaries,
                transfer: encoding.transfer,
                matrix: encoding.matrix,
                full_range: encoding.full_range,
            },
        },
        crate::HEAP.peak() as u64,
    )
}
pub fn run() -> Result<()> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build_global()
        .map_err(|_| invalid("isolated decoder thread pool startup"))?;
    let mut input = std::io::stdin().lock();
    let mut bytes = [0; wire::REQUEST_HEADER];
    input
        .read_exact(&mut bytes)
        .map_err(|_| invalid("truncated request header"))?;
    let header = wire::Header::decode(&bytes)?;
    let mut source = Vec::new();
    source
        .try_reserve_exact(header.length as usize)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "source allocation"))?;
    source.resize(header.length as usize, 0);
    input
        .read_exact(&mut source)
        .map_err(|_| invalid("truncated source"))?;
    let mut tail = [0];
    if input
        .read(&mut tail)
        .map_err(|_| invalid("source framing"))?
        != 0
    {
        return Err(invalid("extra source bytes"));
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| decode(header, &source)))
        .unwrap_or_else(|_| Err(invalid("isolated decoder panic")));
    let output = match result {
        Ok(output) => output,
        Err(error) => wire::error(header, &error, crate::HEAP.peak() as u64)?,
    };
    // Live/peak counters cover requested Rust heap bytes, never physical RSS.
    debug_assert!(crate::HEAP.live() <= wire::HEAP_BYTES as usize);
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&output)
        .and_then(|_| stdout.flush())
        .map_err(|_| invalid("response pipe closed"))
}
