//! Calls only the host-installed service; no document supplies a Worker URL.
use js_sys::{Function, Reflect, Uint8Array};
use paintcraft_core::types::{Error, ErrorCode, Result};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
pub(super) fn error(value: JsValue) -> Error {
    let code = Reflect::get(&value, &"code".into())
        .ok()
        .and_then(|v| v.as_string());
    Error::new(
        match code.as_deref() {
            Some("ResourceLimit") => ErrorCode::ResourceLimit,
            Some("OperationCancelled") => ErrorCode::OperationCancelled,
            Some("ConflictStaleBase") => ErrorCode::ConflictStaleBase,
            Some("CapabilityUnsupported") => ErrorCode::CapabilityUnsupported,
            _ => ErrorCode::CodecFailed,
        },
        "Browser codec Worker failed",
    )
}
pub(super) fn call(value: &JsValue, name: &str, args: &js_sys::Array) -> Result<JsValue> {
    Reflect::get(value, &name.into())
        .map_err(error)?
        .dyn_into::<Function>()
        .map_err(error)?
        .apply(value, args)
        .map_err(error)
}
pub(super) fn object(fields: &[(&str, JsValue)]) -> Result<JsValue> {
    let value = js_sys::Object::new();
    for (key, field) in fields {
        Reflect::set(&value, &(*key).into(), field).map_err(error)?;
    }
    Ok(value.into())
}
#[derive(Clone, Debug)]
pub(super) struct Cancellation(pub(super) JsValue);
impl Cancellation {
    pub(super) fn cancel(&self) {
        let _ = call(&self.0, "cancel", &js_sys::Array::new());
    }
}
pub(super) fn quarantined(service: &JsValue) -> bool {
    call(service, "budget", &js_sys::Array::new())
        .and_then(|v| Reflect::get(&v, &"quarantined".into()).map_err(error))
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}
#[derive(Debug)]
pub(super) struct Lease {
    pub(super) value: JsValue,
    pub(super) valid: bool,
}
impl Drop for Lease {
    fn drop(&mut self) {
        if !self.valid {
            let _ = call(&self.value, "quarantine", &js_sys::Array::new());
        }
        let _ = call(&self.value, "release", &js_sys::Array::new());
    }
}
pub(super) struct Admission {
    pub(super) value: JsValue,
    pub(super) used: bool,
}
impl Drop for Admission {
    fn drop(&mut self) {
        if !self.used {
            let _ = call(&self.value, "release", &js_sys::Array::new());
        }
    }
}
pub(super) fn bytes(value: &JsValue, key: &str, max: usize) -> Result<Vec<u8>> {
    let array = Reflect::get(value, &key.into())
        .map_err(error)?
        .dyn_into::<Uint8Array>()
        .map_err(error)?;
    let length = array.length() as usize;
    if length == 0 || length > max {
        return Err(failed("Codec response allowance"));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Codec response allocation"))?;
    bytes.resize(length, 0);
    array.copy_to(&mut bytes);
    Ok(bytes)
}
fn failed(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
#[wasm_bindgen::prelude::wasm_bindgen(module = "/src/lib/download.js")]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(catch,js_name=downloadCopyTracked)]
    fn download_tracked(
        bytes: &[u8],
        name: &str,
        mime: &str,
    ) -> std::result::Result<js_sys::Promise, JsValue>;
}
/// Hold the caller's prepared bytes/lease until URL retirement is acknowledged.
/// Initiating a download never asserts filesystem persistence.
pub(super) async fn download(
    bytes: &[u8],
    name: &str,
    mime: &str,
    lease: Option<&mut Lease>,
) -> Result<()> {
    let retired: Result<bool> = async {
        let value = JsFuture::from(download_tracked(bytes, name, mime).map_err(error)?)
            .await
            .map_err(error)?;
        Reflect::get(&value, &"requested".into())
            .ok()
            .and_then(|v| v.as_bool())
            .ok_or_else(|| failed("Invalid codec download retirement"))
    }
    .await;
    match retired {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::new(
            ErrorCode::StorageFailed,
            "Download could not be requested",
        )),
        Err(error) => {
            if let Some(lease) = lease {
                lease.valid = false;
            }
            Err(error)
        }
    }
}
