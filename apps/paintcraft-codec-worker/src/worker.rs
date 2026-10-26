//! Decode only inside the executable's fixed allocator/process boundary.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::codec_worker as wire;
use std::io::{Read, Write};
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
pub fn run(jxr_source: Option<&std::path::Path>) -> Result<()> {
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
    // Release the global stdin lock before the watchdog acquires it. Keeping
    // this lock across decode would hide parent death until decode completed.
    drop(input);
    crate::parent_lease::start()?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if header.codec == wire::Codec::JxrRgb8 {
            let path =
                jxr_source.ok_or_else(|| invalid("JXR needs parent-owned private source path"))?;
            crate::jxr::execute(header, &source, path, &|| crate::HEAP.peak() as u64)
        } else if jxr_source.is_some() {
            Err(invalid("Private JXR path supplied for another codec"))
        } else if header.codec == wire::Codec::AvifSrgb8 {
            #[cfg(feature = "avif-codec")]
            {
                crate::avif::execute(header, &source, &|| crate::HEAP.peak() as u64)
            }
            #[cfg(not(feature = "avif-codec"))]
            {
                Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "AVIF charged native decoder is not built",
                ))
            }
        } else {
            wire::execute(header, &source, &|| crate::HEAP.peak() as u64)
        }
    }))
    .unwrap_or_else(|_| Err(invalid("isolated decoder panic")));
    let output = match result {
        Ok(output) => output,
        Err(error) => wire::error(header, &error, crate::HEAP.peak() as u64)?,
    };
    // Counters cover requested Rust and selected C-shim allocations, not RSS.
    debug_assert!(crate::HEAP.live() <= wire::HEAP_BYTES as usize);
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&output)
        .and_then(|_| stdout.flush())
        .map_err(|_| invalid("response pipe closed"))
}
