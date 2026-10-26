//! Optional private RGB8 encoder. No output path or application state is opened.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::codec_worker::jxr_encode as wire;
use std::io::{Cursor, Read, Write};
fn codec(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn encoder_error(error: jpegxr::Error) -> Error {
    let code = match error {
        jpegxr::Error::OutputLimit => ErrorCode::ResourceLimit,
        _ => ErrorCode::CodecFailed,
    };
    Error::new(code, format!("JXR encoder refused output: {error}"))
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
    // Only the adapter's typed output extent fault is a quota refusal. Sink IO,
    // cancellation callbacks and recovered panics remain ordinary codec failures.
    result.map_err(encoder_error)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_fixed_encoder_exact_cap_succeeds_and_one_byte_less_is_typed() {
        use paintcraft_formats::codec_worker::Owner;
        use std::sync::Arc;
        let rgb = Arc::new((0..36).map(|index| (index * 71) as u8).collect());
        let make = |cap| {
            wire::Request::capture(
                wire::Layout::new(4, 3, cap).unwrap(),
                Arc::clone(&rgb),
                Owner {
                    session: uuid::Uuid::new_v4(),
                    state: uuid::Uuid::new_v4(),
                    epoch: 1,
                    frontend: 3,
                },
                &|| Ok(()),
            )
            .unwrap()
        };
        let reference = encoded(&make(1 << 20)).unwrap();
        assert_eq!(encoded(&make(reference.len() as u64)).unwrap(), reference);
        for cap in [1, reference.len() as u64 - 1] {
            assert_eq!(
                encoded(&make(cap)).unwrap_err().code,
                ErrorCode::ResourceLimit
            );
        }
    }
    #[test]
    fn actual_encoder_retains_independently_qualified_long_header_files() {
        use paintcraft_formats::codec_worker::Owner;
        use std::sync::Arc;
        for (w, h, expected) in [
            (4096, 1, include_bytes!("../../../crates/services/paintcraft-formats/tests/jxr-fixtures/white-rgb8-4096x1.jxr").as_slice()),
            (1, 4096, include_bytes!("../../../crates/services/paintcraft-formats/tests/jxr-fixtures/white-rgb8-1x4096.jxr").as_slice()),
        ] {
            let request = wire::Request::capture(
                wire::Layout::new(w, h, 65536).unwrap(),
                Arc::new(vec![255; w as usize * h as usize * 3]),
                Owner { session: uuid::Uuid::new_v4(), state: uuid::Uuid::new_v4(), epoch: 1, frontend: 3 },
                &|| Ok(()),
            ).unwrap();
            assert_eq!(encoded(&request).unwrap(), expected);
        }
    }
    #[test]
    fn public_adapter_callback_fault_stays_io() {
        let mut sink = Cursor::new(Vec::new());
        let result = jpegxr::Encoder::new().encode_pixels_to_seekable(
            1,
            1,
            jpegxr::PixelFormat::rgb8().unwrap(),
            &[17, 83, 249],
            3,
            &mut sink,
            1024,
            &mut || Err(std::io::Error::other("JPEG XR output extent limit")),
        );
        assert!(matches!(result, Err(jpegxr::Error::Io(_))));
        assert!(sink.into_inner().is_empty());
    }
    #[test]
    fn only_typed_output_limit_is_a_quota_error() {
        assert_eq!(
            encoder_error(jpegxr::Error::OutputLimit).code,
            ErrorCode::ResourceLimit
        );
        for kind in [
            std::io::ErrorKind::Other,
            std::io::ErrorKind::Interrupted,
            std::io::ErrorKind::OutOfMemory,
        ] {
            let error = jpegxr::Error::Io(std::io::Error::new(kind, "JPEG XR output extent limit"));
            assert_eq!(encoder_error(error).code, ErrorCode::CodecFailed);
        }
    }
}
