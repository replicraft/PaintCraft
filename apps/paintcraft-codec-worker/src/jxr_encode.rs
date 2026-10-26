//! Optional private RGB8 encoder. No output path or application state is opened.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::codec_worker::jxr_encode as wire;
use std::io::{Cursor, Read, Write};
fn codec(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn encoded(request: &wire::Request) -> Result<Vec<u8>> {
    let header = request.header();
    let layout = header.layout();
    header.verify_rgb(request.rgb(), &|| Ok(()))?;
    let maximum = usize::try_from(layout.encoded_cap())
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "JXR output address extent"))?;
    // Charge the complete permitted capacity through this executable's existing
    // requested-heap allocator before codec work. No hidden seekable file exists.
    let mut output = Vec::new();
    output
        .try_reserve_exact(maximum)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "JXR output capacity allocation"))?;
    let mut sink = Cursor::new(output);
    let format = jpegxr::PixelFormat::rgb8().map_err(|error| codec(error.to_string()))?;
    let result = jpegxr::Encoder::new().encode_pixels_to_seekable(
        layout.width(),
        layout.height(),
        format,
        request.rgb(),
        layout.stride() as usize,
        &mut sink,
        layout.encoded_cap(),
        &mut || Ok(()),
    );
    // The selected API takes an immutable source; retain a post-codec binding
    // check as well. The parent independently retains its original RGB capture.
    header.verify_rgb(request.rgb(), &|| Ok(()))?;
    // Error::Io currently includes both quota and callback failures. Do not infer
    // ResourceLimit by matching untyped error text; all such failures refuse.
    result.map_err(|error| codec(format!("JXR encoder refused output: {error}")))?;
    Ok(sink.into_inner())
}
pub fn run() -> Result<()> {
    let mut input = std::io::stdin().lock();
    // Header geometry/stride/exact length/cap are checked before RGB allocation;
    // source hash/canonical request are checked before output allocation/codec.
    let mut magic = [0; 8];
    input
        .read_exact(&mut magic)
        .map_err(|_| codec("truncated JXR encode magic"))?;
    if magic != *wire::REQUEST_MAGIC {
        return Err(codec("wrong JXR encode request direction"));
    }
    let mut framed = Cursor::new(magic).chain(input);
    let request = wire::Request::read_from(&mut framed, &|| Ok(()))?;
    drop(framed);
    crate::parent_lease::start()?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| encoded(&request)))
        .unwrap_or_else(|_| Err(codec("isolated JXR encoder panic")));
    let mut stdout = std::io::stdout().lock();
    match result {
        Ok(encoded) => {
            // A separate typed header and borrowed file bytes are streamed only
            // after complete encoding and profile validation. No second whole
            // encoded-file response allocation is made inside this child.
            wire::write_success(
                request.header(),
                &encoded,
                crate::HEAP.peak() as u64,
                &mut stdout,
                &|| Ok(()),
            )?;
        }
        Err(error) => wire::write_failure(
            request.header(),
            &error,
            crate::HEAP.peak() as u64,
            &mut stdout,
            &|| Ok(()),
        )?,
    }
    debug_assert!(crate::HEAP.live() <= paintcraft_formats::codec_worker::HEAP_BYTES as usize);
    stdout
        .flush()
        .map_err(|_| codec("JXR response pipe closed"))
}
