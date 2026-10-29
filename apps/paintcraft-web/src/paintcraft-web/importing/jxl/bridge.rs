//! Host-installed JS object; no imported file can choose a module or Worker URL.
use super::*;
use js_sys::{Function, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
fn error(value: JsValue) -> Error {
    let code = Reflect::get(&value, &"code".into())
        .ok()
        .and_then(|v| v.as_string());
    Error::new(
        match code.as_deref() {
            Some("OperationCancelled") => ErrorCode::OperationCancelled,
            Some("ConflictStaleBase") => ErrorCode::ConflictStaleBase,
            Some("ResourceLimit") => ErrorCode::ResourceLimit,
            Some("CapabilityUnsupported") => ErrorCode::CapabilityUnsupported,
            _ => ErrorCode::CodecFailed,
        },
        "Isolated browser JPEG XL import failed",
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
#[derive(Debug)]
pub(super) struct Cancellation(JsValue);
impl Cancellation {
    pub(super) fn cancel(&self) {
        let _ = call(&self.0, "cancel", &js_sys::Array::new());
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
pub(super) fn quarantined(service: &JsValue) -> Result<bool> {
    let budget = call(service, "budget", &js_sys::Array::new())?;
    Reflect::get(&budget, &"quarantined".into())
        .map_err(error)?
        .as_bool()
        .ok_or_else(|| Error::new(ErrorCode::CodecFailed, "Invalid codec budget"))
}
#[derive(Debug)]
pub(super) struct Lease {
    value: JsValue,
    validated: bool,
    released: bool,
}
impl Lease {
    pub fn release(&mut self) -> Result<()> {
        if !self.released {
            if !self.validated {
                let _ = call(&self.value, "quarantine", &js_sys::Array::new());
            }
            call(&self.value, "release", &js_sys::Array::new())?;
            self.released = true;
        }
        Ok(())
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        let _ = self.release();
    }
}
fn bytes(value: &JsValue, name: &str, limit: usize) -> Result<Vec<u8>> {
    let array = Reflect::get(value, &name.into())
        .map_err(error)?
        .dyn_into::<Uint8Array>()
        .map_err(error)?;
    let length = array.length() as usize;
    if length == 0 || length > limit {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Browser response allowance",
        ));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Browser response allocation"))?;
    bytes.resize(length, 0);
    array.copy_to(&mut bytes);
    Ok(bytes)
}
fn owner(value: wire::Owner) -> Result<JsValue> {
    let object = js_sys::Object::new();
    for (key, value) in [
        ("session", JsValue::from_str(&value.session.to_string())),
        ("state", JsValue::from_str(&value.state.to_string())),
        ("epoch", js_sys::BigInt::from(value.epoch).into()),
        ("frontend", JsValue::from_f64(3.0)),
    ] {
        Reflect::set(&object, &key.into(), &value).map_err(error)?;
    }
    Ok(object.into())
}
pub(super) fn sync_owner(service: &JsValue, captured: wire::Owner) -> Result<()> {
    let args = js_sys::Array::new();
    args.push(&owner(captured)?);
    call(service, "setOwner", &args).map(|_| ())
}
pub(super) async fn prepare(
    service: &JsValue,
    source: Source,
    captured: wire::Owner,
    nonce: Uuid,
    cancelled: Rc<Cell<bool>>,
    cancellation: Rc<RefCell<Option<Cancellation>>>,
) -> Result<Prepared> {
    if cancelled.get() {
        return Err(Error::new(
            ErrorCode::OperationCancelled,
            "Import cancelled",
        ));
    }
    let current = owner(captured)?;
    let args = js_sys::Array::new();
    args.push(&source.file);
    args.push(&current);
    args.push(&JsValue::from_f64(wire::PIXELS_MAX as f64));
    args.push(&nonce.to_string().into());
    let mut admission = Admission {
        value: call(service, "beginDecodeIntegerFile", &args)?,
        used: false,
    };
    *cancellation.borrow_mut() = Some(Cancellation(admission.value.clone()));
    let promise = call(&admission.value, "run", &js_sys::Array::new())?
        .dyn_into::<js_sys::Promise>()
        .map_err(error)?;
    admission.used = true;
    let value = JsFuture::from(promise).await.map_err(error)?;
    let mut lease = Lease {
        value,
        validated: false,
        released: false,
    };
    let header = bytes(&lease.value, "requestHeader", wire::REQUEST_HEADER)?;
    let digest = bytes(&lease.value, "sourceSha256", 32)?;
    let response = bytes(&lease.value, "responseBytes", wire::OUTPUT_BYTES_MAX)?;
    let (request, frame) =
        response::validate(&source, captured, nonce, &header, &response, &digest)?;
    if request.codec != wire::Codec::JxlIntegerRgba8 || frame.jxl_report.is_none() {
        return Err(Error::new(
            ErrorCode::CodecFailed,
            "Integer JPEG XL result contract changed",
        ));
    }
    let report = frame
        .jxl_report
        .ok_or_else(|| Error::new(ErrorCode::CodecFailed, "Missing integer JPEG XL report"))?;
    lease.validated = true;
    let check = || {
        if cancelled.get() {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "Import cancelled",
            ))
        } else {
            Ok(())
        }
    };
    let document = response::assemble(
        request,
        frame,
        paintcraft_raster::Admission::new(0).residency_budget(),
        &check,
    )?;
    if cancelled.get() {
        return Err(Error::new(
            ErrorCode::OperationCancelled,
            "Import cancelled",
        ));
    }
    Ok(Prepared {
        document,
        name: source.name,
        report,
        lease,
    })
}
