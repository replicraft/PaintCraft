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
        "Isolated browser TIFF page import failed",
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
fn owner(value: decoder::Owner) -> Result<JsValue> {
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
pub(super) fn sync_owner(service: &JsValue, captured: decoder::Owner) -> Result<()> {
    let args = js_sys::Array::new();
    args.push(&owner(captured)?);
    call(service, "setOwner", &args).map(|_| ())
}
fn plan(selection: Option<wire::Selection>) -> Result<JsValue> {
    let Some(selected) = selection else {
        return Ok(JsValue::NULL);
    };
    let value = js_sys::Object::new();
    for (key, item) in [
        ("index", selected.page.index as f64),
        ("pageCount", selected.page_count as f64),
        ("width", selected.page.width as f64),
        ("height", selected.page.height as f64),
        ("ifdOffset", selected.page.ifd_offset as f64),
    ] {
        Reflect::set(&value, &key.into(), &item.into()).map_err(error)?;
    }
    Reflect::set(
        &value,
        &"sourceSha256".into(),
        &paintcraft_document::hex(&selected.source_sha256).into(),
    )
    .map_err(error)?;
    Ok(value.into())
}
pub(super) async fn prepare(
    service: &JsValue,
    source: Source,
    captured: decoder::Owner,
    nonce: Uuid,
    cancelled: Rc<Cell<bool>>,
    cancellation: Rc<RefCell<Option<Cancellation>>>,
    selection: Option<wire::Selection>,
) -> Result<Output> {
    let check = || {
        if cancelled.get() {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "TIFF import cancelled",
            ))
        } else {
            Ok(())
        }
    };
    check()?;
    if call(service, "tiffPageProtocolVersion", &js_sys::Array::new())?.as_f64()
        != Some(f64::from(wire::VERSION))
    {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Browser TIFF service protocol changed",
        ));
    }
    let args = js_sys::Array::new();
    args.push(&source.file);
    args.push(&owner(captured)?);
    args.push(&plan(selection)?);
    args.push(&nonce.to_string().into());
    let mut admission = Admission {
        value: call(service, "beginTiffFile", &args)?,
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
    let raw = bytes(&lease.value, "requestHeader", wire::REQUEST_HEADER)?;
    let request = wire::Header::decode(&raw)?;
    let sha = bytes(&lease.value, "sourceSha256", 32)?;
    if request.owner != captured
        || request.nonce != nonce
        || request.source_bytes != source.length
        || request.selection != selection
        || request.pixels != wire::PIXELS_MAX
        || sha != request.source_sha256
    {
        return Err(Error::new(
            ErrorCode::ConflictStaleBase,
            "TIFF response differs from captured File/selection/owner",
        ));
    }
    let raw = bytes(&lease.value, "responseBytes", wire::OUTPUT_MAX)?;
    match wire::decode_response(&raw, &request, captured, env!("CARGO_PKG_VERSION"))? {
        wire::Frame::Refused(error) => {
            lease.validated = true;
            lease.release()?;
            Err(error)
        }
        wire::Frame::Success {
            page_count,
            width,
            height,
            report,
            payload,
        } => {
            check()?;
            if let Some(selected) = selection {
                let assessment = tiff_pages::Assessment::decode(report)?;
                assessment.validate_result(
                    request.source_sha256,
                    request.source_bytes,
                    selected.page.index,
                    width,
                    height,
                    payload,
                )?;
                if assessment.page_count() != page_count
                    || assessment.descriptor().ifd_offset != selected.page.ifd_offset
                {
                    return Err(Error::new(
                        ErrorCode::CodecFailed,
                        "TIFF selected report changed from inventory",
                    ));
                }
                lease.validated = true;
                let document = response::assemble(
                    &assessment,
                    payload,
                    paintcraft_raster::Admission::new(0).residency_budget(),
                    &check,
                )?;
                check()?;
                Ok(Output::Decoded(Box::new(Prepared {
                    document,
                    report: assessment,
                    selection: selected,
                    lease,
                })))
            } else {
                let pages = wire::decode_inventory(payload, page_count, source.length)?;
                lease.validated = true;
                lease.release()?;
                Ok(Output::Inventory {
                    pages,
                    source_sha256: request.source_sha256,
                })
            }
        }
    }
}
