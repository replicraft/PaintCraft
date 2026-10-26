use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::codec_worker as wire;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn application_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
#[wasm_bindgen]
pub fn protocol_version() -> u16 {
    wire::PROTOCOL_VERSION
}
/// Linear memory is bounded by the final Wasm module's declared maximum, which
/// the build inspector and actual Worker tests verify. This is not native heap
/// accounting, physical RSS, or a total browser-process memory cap.
#[wasm_bindgen]
pub fn linear_memory_ceiling() -> u32 {
    256 << 20
}
fn request(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() < wire::REQUEST_HEADER
        || bytes.len() > wire::REQUEST_HEADER + wire::INPUT_BYTES_MAX
    {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Wasm codec request size allowance",
        ));
    }
    let header = wire::Header::decode(&bytes[..wire::REQUEST_HEADER])?;
    // Rayon uses this Worker's current thread. No nested Worker or additional
    // decoder thread is started. The whole Worker is destroyed after one job.
    let result = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .use_current_thread()
        .build_global()
        .map_err(|_| Error::new(ErrorCode::CodecFailed, "Wasm codec runner startup"))
        .and_then(|()| wire::execute(header, &bytes[wire::REQUEST_HEADER..], &|| 0));
    match result {
        Ok(output) => Ok(output),
        // Native requested-heap metrics do not exist here. Zero explicitly means
        // no native heap snapshot; it never represents the Wasm linear-memory cap.
        Err(error) => wire::error(header, &error, 0),
    }
}
/// Only invoke in the dedicated isolated Worker. Panics/OOM may trap this Wasm
/// instance; the controller must terminate it and discard every partial result.
#[wasm_bindgen]
pub fn decode_request(bytes: &[u8]) -> std::result::Result<Vec<u8>, JsValue> {
    request(bytes).map_err(|error| JsValue::from_str(&error.to_string()))
}
