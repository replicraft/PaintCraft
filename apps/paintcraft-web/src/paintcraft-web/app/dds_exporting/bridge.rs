//! Calls only the host-installed service; no document supplies a Worker URL.
use super::*;
use js_sys::{Function, Reflect, Uint8Array};
use paintcraft_formats::codec_worker::Owner;
use std::sync::Arc;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
fn error(value: JsValue) -> Error {
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
        "Browser DDS worker failed",
    )
}
fn call(value: &JsValue, name: &str, args: &js_sys::Array) -> Result<JsValue> {
    Reflect::get(value, &name.into())
        .map_err(error)?
        .dyn_into::<Function>()
        .map_err(error)?
        .apply(value, args)
        .map_err(error)
}
fn object(fields: &[(&str, JsValue)]) -> Result<JsValue> {
    let value = js_sys::Object::new();
    for (key, field) in fields {
        Reflect::set(&value, &(*key).into(), field).map_err(error)?;
    }
    Ok(value.into())
}
pub(super) fn cancel(service: &JsValue) {
    let _ = call(service, "cancel", &js_sys::Array::new());
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
    value: JsValue,
    valid: bool,
}
impl Drop for Lease {
    fn drop(&mut self) {
        if !self.valid {
            let _ = call(&self.value, "quarantine", &js_sys::Array::new());
        }
        let _ = call(&self.value, "release", &js_sys::Array::new());
    }
}
struct Admission {
    value: JsValue,
    used: bool,
}
impl Drop for Admission {
    fn drop(&mut self) {
        if !self.used {
            let _ = call(&self.value, "release", &js_sys::Array::new());
        }
    }
}
fn bytes(value: &JsValue, key: &str, max: usize) -> Result<Vec<u8>> {
    let array = Reflect::get(value, &key.into())
        .map_err(error)?
        .dyn_into::<Uint8Array>()
        .map_err(error)?;
    let length = array.length() as usize;
    if length == 0 || length > max {
        return Err(failed("DDS response allowance"));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "DDS response allocation"))?;
    bytes.resize(length, 0);
    array.copy_to(&mut bytes);
    Ok(bytes)
}
pub(super) async fn prepare(
    service: JsValue,
    doc: Arc<paintcraft_document::Document>,
    plan: wire::SourcePlan,
    owner: Owner,
    nonce: uuid::Uuid,
) -> Result<Prepared> {
    let owner_value = object(&[
        ("session", owner.session.to_string().into()),
        ("state", owner.state.to_string().into()),
        ("epoch", js_sys::BigInt::from(owner.epoch).into()),
        ("frontend", 3u32.into()),
    ])?;
    let args = js_sys::Array::new();
    args.push(&owner_value);
    call(&service, "setOwner", &args)?;
    let size = plan.size();
    let metadata = object(&[
        ("width", size.0.into()),
        ("height", size.1.into()),
        ("sourceFlags", u32::from(plan.source_flags()).into()),
        (
            "settings",
            Uint8Array::from(plan.settings().as_slice()).into(),
        ),
    ])?;
    let args = js_sys::Array::new();
    args.push(&owner_value);
    args.push(&metadata);
    args.push(&nonce.to_string().into());
    // Synchronous admission MUST precede serialization and all snapshot copies.
    let mut admission = Admission {
        value: call(&service, "beginDdsExport", &args)?,
        used: false,
    };
    let source = plan.snapshot(&doc)?;
    let request = wire::Header::capture(&plan, &source, owner, nonce)?;
    let args = js_sys::Array::new();
    args.push(&Uint8Array::from(source.as_slice()));
    let promise = call(&admission.value, "run", &args)?
        .dyn_into::<js_sys::Promise>()
        .map_err(error)?;
    admission.used = true;
    drop(source);
    drop(args);
    drop(doc);
    let value = JsFuture::from(promise).await.map_err(error)?;
    let mut lease = Lease {
        value,
        valid: false,
    };
    let echo = bytes(&lease.value, "requestHeader", wire::REQUEST_HEADER)?;
    let digest = bytes(&lease.value, "sourceSha256", 32)?;
    let response = bytes(&lease.value, "responseBytes", wire::RESPONSE_BYTES_MAX)?;
    if digest != request.source_sha256 {
        return Err(failed("DDS source binding changed"));
    }
    let mut prepared = response::validate(&request, &echo, response, || lease.valid = true)?;
    prepared.lease = Some(lease);
    Ok(prepared)
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
pub(super) async fn download(mut prepared: Prepared, name: String) -> Result<()> {
    let result = async {
        let promise =
            download_tracked(prepared.bytes(), &name, "image/vnd-ms.dds").map_err(error)?;
        JsFuture::from(promise).await.map_err(error)
    }
    .await;
    let value = match result {
        Ok(value) => value,
        Err(error) => {
            if let Some(lease) = &mut prepared.lease {
                lease.valid = false;
            }
            return Err(error);
        }
    };
    let requested = Reflect::get(&value, &"requested".into())
        .ok()
        .and_then(|v| v.as_bool());
    let Some(requested) = requested else {
        if let Some(lease) = &mut prepared.lease {
            lease.valid = false;
        }
        return Err(failed("Invalid DDS download retirement"));
    };
    if requested {
        Ok(())
    } else {
        Err(Error::new(
            ErrorCode::StorageFailed,
            "DDS download could not be requested",
        ))
    }
}
