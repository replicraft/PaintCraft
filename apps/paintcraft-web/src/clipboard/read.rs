//! Explicit PNG reads; no ambient clipboard polling or browser-side pixel conversion.
use super::*;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_formats::registry::Frontend;
#[cfg(any(target_arch = "wasm32", test))]
use paintcraft_formats::{
    image,
    registry::{self, Platform},
};

#[derive(Debug)]
pub(crate) struct Decoded {
    pub image: Image,
    pub losses: Vec<String>,
}
#[cfg(any(target_arch = "wasm32", test))]
fn decode(bytes: &[u8], frontend: Frontend, budget: &SharedBudget) -> Result<Decoded> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(Error::new(
            ErrorCode::CodecFailed,
            "Clipboard PNG signature mismatch",
        ));
    }
    registry::admit_open(frontend, "clipboard.png", bytes, Platform::Web)?;
    // Reserve output before entering the codec; only actual retained capacity
    // transfers into the returned image. Codec scratch is outside this grant.
    let grant = budget.reserve_capacity(MAX_PIXELS * 4)?;
    let decoded = image::decode_with_pixel_limit(bytes, MAX_PIXELS)?;
    if decoded
        .losses
        .iter()
        .any(|loss| paintcraft_ui::i18n::import_loss_description(loss).is_none())
    {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "Unrecognized clipboard import limitation",
        ));
    }
    let image =
        Image::from_rgba_reserved(decoded.width, decoded.height, decoded.rgba, &grant, &|| {
            Ok(())
        })?;
    Ok(Decoded {
        image,
        losses: decoded.losses,
    })
}
#[cfg(target_arch = "wasm32")]
fn api() -> Result<(wasm_bindgen::JsValue, js_sys::Function)> {
    use js_sys::Reflect;
    use wasm_bindgen::{JsCast, JsValue};
    let window = web_sys::window().ok_or_else(unavailable)?;
    if !window.is_secure_context() {
        return Err(unavailable());
    }
    let clipboard = Reflect::get(window.navigator().as_ref(), &JsValue::from_str("clipboard"))
        .map_err(|_| unavailable())?;
    let read = Reflect::get(&clipboard, &JsValue::from_str("read"))
        .ok()
        .and_then(|v| v.dyn_into().ok())
        .ok_or_else(unavailable)?;
    Ok((clipboard, read))
}
pub(crate) fn read_supported() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        api().is_ok()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}
#[cfg(target_arch = "wasm32")]
fn failed() -> Error {
    Error::new(
        ErrorCode::StorageFailed,
        "Clipboard image read was denied or failed",
    )
}
/// Invoke read synchronously from the action, before the first await. No
/// permissions query, automatic retry or fallback to text/HTML is performed.
#[cfg(target_arch = "wasm32")]
pub(crate) fn read(
    ctx: &eframe::egui::Context,
    frontend: Frontend,
    budget: SharedBudget,
) -> Result<Job<Decoded>> {
    use wasm_bindgen::JsCast;
    let (clipboard, method) = api()?;
    let promise = method
        .call0(&clipboard)
        .map_err(|_| failed())?
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| failed())?;
    let (job, completion) = channel();
    let ctx = ctx.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let result = read_items(promise, frontend, budget, &completion).await;
        completion.complete(result);
        ctx.request_repaint();
    });
    Ok(job)
}
#[cfg(target_arch = "wasm32")]
async fn read_items(
    promise: js_sys::Promise,
    frontend: Frontend,
    budget: SharedBudget,
    completion: &Completion<Decoded>,
) -> Result<Decoded> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    let items = JsFuture::from(promise).await.map_err(|_| failed())?;
    if !completion.live() {
        return Err(failed());
    }
    if !js_sys::Array::is_array(&items) {
        return Err(failed());
    }
    let items = js_sys::Array::from(&items);
    if items.length() > 256 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Too many clipboard items",
        ));
    }
    let mut chosen = None;
    for value in items.iter() {
        let item = value
            .dyn_into::<web_sys::ClipboardItem>()
            .map_err(|_| failed())?;
        let types = item.types();
        if types.length() > 256 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Too many clipboard types",
            ));
        }
        if types
            .iter()
            .any(|v| v.as_string().as_deref() == Some("image/png"))
        {
            chosen = Some(item);
            break;
        }
    }
    let item = chosen.ok_or_else(|| {
        Error::new(
            ErrorCode::CapabilityUnsupported,
            "Clipboard has no PNG image",
        )
    })?;
    let blob = JsFuture::from(item.get_type("image/png"))
        .await
        .map_err(|_| failed())?
        .dyn_into::<web_sys::Blob>()
        .map_err(|_| failed())?;
    if !completion.live() {
        return Err(failed());
    }
    let length = blob.size();
    if !length.is_finite() || length < 1.0 || length > ENCODED_BYTES as f64 || length.fract() != 0.0
    {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "Clipboard encoded image exceeds the browser limit",
        ));
    }
    // Bound the ArrayBuffer plus Rust copy before materializing either. This is
    // payload accounting, not a guarantee about browser GC or native allocations.
    thread_local! { static ENCODED_BUDGET: SharedBudget = SharedBudget::new((ENCODED_BYTES * 2) as u64); }
    let encoded_budget = ENCODED_BUDGET.with(Clone::clone);
    let _encoded = encoded_budget.reserve((length as u64) * 2)?;
    let buffer = JsFuture::from(blob.array_buffer())
        .await
        .map_err(|_| failed())?;
    if !completion.live() {
        return Err(failed());
    }
    let bytes = js_sys::Uint8Array::new(&buffer);
    if f64::from(bytes.length()) != length {
        return Err(failed());
    }
    let mut copy = Vec::new();
    copy.try_reserve_exact(bytes.length() as usize)
        .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Clipboard allocation failed"))?;
    let _extra =
        encoded_budget.reserve(copy.capacity().saturating_sub(bytes.length() as usize) as u64)?;
    copy.resize(bytes.length() as usize, 0);
    bytes.copy_to(&mut copy);
    decode(&copy, frontend, &budget)
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn read(
    _ctx: &eframe::egui::Context,
    _frontend: Frontend,
    _budget: SharedBudget,
) -> Result<Job<Decoded>> {
    Err(unavailable())
}

#[cfg(test)]
mod tests;
