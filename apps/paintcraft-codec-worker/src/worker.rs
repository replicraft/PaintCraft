//! Decode only inside the executable's fixed allocator/process boundary.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::codec_worker as wire;
use std::io::{Read, Write};
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
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
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        wire::execute(header, &source, &|| crate::HEAP.peak() as u64)
    }))
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
