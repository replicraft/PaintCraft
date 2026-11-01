//! User activation is consumed before asynchronous Source6 work; no Main codec.
use super::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue};
#[derive(Debug)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) struct Deferred {
    #[cfg(target_arch = "wasm32")]
    value: JsValue,
    consumed: bool,
}
impl Deferred {
    pub(crate) fn begin() -> Result<Self> {
        #[cfg(target_arch = "wasm32")]
        {
            Ok(Self {
                value: browser::begin().map_err(|_| unavailable())?,
                consumed: false,
            })
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Err(unavailable())
        }
    }
    pub(crate) fn complete(mut self, ctx: &eframe::egui::Context, bytes: &[u8]) -> Result<Job> {
        if bytes.len() > 16 << 20 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(Error::new(
                ErrorCode::CodecFailed,
                "Background clipboard PNG extent/signature",
            ));
        }
        #[cfg(target_arch = "wasm32")]
        {
            let get = |name: &str| {
                js_sys::Reflect::get(&self.value, &name.into()).map_err(|_| unavailable())
            };
            let resolve = get("resolve")?
                .dyn_into::<js_sys::Function>()
                .map_err(|_| unavailable())?;
            resolve
                .call1(&self.value, &js_sys::Uint8Array::from(bytes))
                .map_err(|_| unavailable())?;
            let promise = get("promise")?
                .dyn_into::<js_sys::Promise>()
                .map_err(|_| unavailable())?;
            let (job, completion) = channel();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = wasm_bindgen_futures::JsFuture::from(promise)
                    .await
                    .map(|_| ())
                    .map_err(|_| write_failed());
                completion.complete(result);
                ctx.request_repaint();
            });
            self.consumed = true;
            Ok(job)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (ctx, bytes);
            self.consumed = true;
            Err(unavailable())
        }
    }
}
impl Drop for Deferred {
    fn drop(&mut self) {
        #[cfg(target_arch = "wasm32")]
        if !self.consumed
            && let Ok(value) = js_sys::Reflect::get(&self.value, &"cancel".into())
            && let Ok(cancel) = value.dyn_into::<js_sys::Function>()
        {
            let _ = cancel.call0(&self.value);
        }
    }
}
#[cfg(target_arch = "wasm32")]
mod browser {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(module = "/src/lib/clipboard-background.js")]
    extern "C" {
        #[wasm_bindgen(catch,js_name=beginDeferredPngClipboard)]
        pub fn begin() -> std::result::Result<JsValue, JsValue>;
    }
}
