//! Browser PNG clipboard writes. Only the browser's acknowledgement reports success.
use paintcraft_core::types::{Error, ErrorCode, Result, clipboard::Image};
use std::{cell::RefCell, rc::Rc};

const MAX_PIXELS: u64 = crate::editor::MAX_PIXELS;
#[cfg(any(target_arch = "wasm32", test))]
const ENCODED_BYTES: usize = 32 << 20;

#[derive(Debug)]
pub(crate) struct Job {
    outcome: Rc<RefCell<Option<Result<()>>>>,
}
impl Job {
    #[cfg(test)]
    pub(crate) fn completed(result: Result<()>) -> Self {
        let (job, completion) = channel();
        completion.complete(result);
        job
    }
    /// Pending and previously consumed jobs return None. Hosts keep at most one job.
    pub(crate) fn try_result(&mut self) -> Option<Result<()>> {
        self.outcome.borrow_mut().take()
    }
}
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug)]
struct Completion(Rc<RefCell<Option<Result<()>>>>);
#[cfg(any(target_arch = "wasm32", test))]
impl Completion {
    fn complete(self, result: Result<()>) {
        *self.0.borrow_mut() = Some(result);
    }
}
#[cfg(any(target_arch = "wasm32", test))]
fn channel() -> (Job, Completion) {
    let outcome = Rc::new(RefCell::new(None));
    (
        Job {
            outcome: outcome.clone(),
        },
        Completion(outcome),
    )
}
fn validate(image: &Image) -> Result<()> {
    let pixels = u64::from(image.width()) * u64::from(image.height());
    if pixels == 0 || pixels > MAX_PIXELS || image.rgba().len() as u64 != pixels * 4 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Clipboard image exceeds the browser limit",
        ));
    }
    Ok(())
}
#[cfg(any(target_arch = "wasm32", test))]
fn encode_png(image: &Image) -> Result<Vec<u8>> {
    validate(image)?;
    let png = paintcraft_formats::image::encode(
        paintcraft_formats::image::ImageFormat::Png,
        image.width(),
        image.height(),
        image.rgba(),
        90,
    )?;
    if png.len() > ENCODED_BYTES {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Clipboard image exceeds the browser limit",
        ));
    }
    Ok(png)
}
fn unavailable() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Image clipboard is unavailable in this browser",
    )
}

/// Call directly in the user action. Browser permission is checked by write(),
/// before PNG encoding is scheduled. This does not request or alter permissions.
#[cfg(target_arch = "wasm32")]
pub(crate) fn submit(ctx: &eframe::egui::Context, image: Image) -> Result<Job> {
    use js_sys::{Array, Function, Object, Promise, Reflect};
    use wasm_bindgen::{JsCast, JsValue};
    validate(&image)?;
    let window = web_sys::window().ok_or_else(unavailable)?;
    if !window.is_secure_context() {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Clipboard access requires a secure context",
        ));
    }
    let clipboard = Reflect::get(window.navigator().as_ref(), &JsValue::from_str("clipboard"))
        .map_err(|_| unavailable())?;
    let write = Reflect::get(&clipboard, &JsValue::from_str("write"))
        .ok()
        .and_then(|write| write.dyn_into::<Function>().ok())
        .ok_or_else(unavailable)?;
    // A pending Blob promise lets write() consume the current activation before
    // potentially expensive PNG work. Nothing is encoded for an unsupported API.
    let mut resolvers = None;
    let payload = Promise::new(&mut |resolve, reject| resolvers = Some((resolve, reject)));
    let (resolve, reject) = resolvers.ok_or_else(unavailable)?;
    let record = Object::new();
    if !Reflect::set(&record, &JsValue::from_str("image/png"), &payload)
        .map_err(|_| unavailable())?
    {
        return Err(unavailable());
    }
    let item = web_sys::ClipboardItem::new_with_record_from_str_to_blob_promise(&record)
        .map_err(|_| unavailable())?;
    let items = Array::new();
    items.push(item.as_ref());
    // Function::call1 catches synchronous failures as well as feature absence;
    // the generated Clipboard::write binding has no catch annotation.
    let written = write
        .call1(&clipboard, items.as_ref())
        .map_err(|_| write_failed())?
        .dyn_into::<Promise>()
        .map_err(|_| write_failed())?;
    let acknowledged = wasm_bindgen_futures::JsFuture::from(written);
    let (job, completion) = channel();
    let ctx = ctx.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let encoded = png_blob(&image);
        let encoding_failure = match encoded {
            Ok(blob) => {
                if resolve.call1(&JsValue::UNDEFINED, blob.as_ref()).is_err() {
                    let error = write_failed();
                    let _ = reject.call1(&JsValue::UNDEFINED, &JsValue::from_str(&error.message));
                    Some(error)
                } else {
                    None
                }
            }
            Err(error) => {
                let _ = reject.call1(&JsValue::UNDEFINED, &JsValue::from_str(&error.message));
                Some(error)
            }
        };
        let result = acknowledged.await.map(|_| ()).map_err(|_| write_failed());
        completion.complete(encoding_failure.map_or(result, Err));
        ctx.request_repaint();
    });
    Ok(job)
}
#[cfg(target_arch = "wasm32")]
fn write_failed() -> Error {
    Error::new(
        ErrorCode::StorageFailed,
        "Clipboard write was denied or failed",
    )
}
#[cfg(target_arch = "wasm32")]
fn png_blob(image: &Image) -> Result<web_sys::Blob> {
    let png = encode_png(image)?;
    let bytes = js_sys::Uint8Array::from(png.as_slice());
    let parts = js_sys::Array::new();
    parts.push(bytes.as_ref());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("image/png");
    web_sys::Blob::new_with_u8_array_sequence_and_options(parts.as_ref(), &options).map_err(|_| {
        Error::new(
            ErrorCode::CodecFailed,
            "Clipboard image could not be encoded",
        )
    })
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn submit(_ctx: &eframe::egui::Context, image: Image) -> Result<Job> {
    validate(&image)?;
    Err(unavailable())
}
#[cfg(test)]
mod tests;
