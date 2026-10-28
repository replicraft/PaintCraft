//! A private WeakMap holds no strong canvas keys or Rust owner/runner references.
use super::{Entry, Error, Lease, Registry};
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

thread_local! {
    static CANVASES: js_sys::WeakMap<HtmlCanvasElement, JsValue> = js_sys::WeakMap::new_typed();
}
#[derive(Clone, Copy)]
pub(crate) struct Map;
pub(crate) type CanvasLease = Lease<Map>;
impl Registry for Map {
    type Canvas = HtmlCanvasElement;
    type Owner = JsValue;
    fn get(&self, canvas: &Self::Canvas) -> Result<Option<Entry<JsValue>>, Error> {
        let Some(value) = CANVASES.with(|map| map.get_checked(canvas)) else {
            return Ok(None);
        };
        let owner = js_sys::Reflect::get(&value, &"owner".into()).map_err(|_| Error::Storage)?;
        let leases = js_sys::Reflect::get(&value, &"leases".into())
            .map_err(|_| Error::Storage)?
            .as_f64()
            .filter(|v| v.is_finite() && v.fract() == 0.0 && *v >= 1.0 && *v <= u32::MAX as f64)
            .ok_or(Error::Storage)? as u32;
        if !owner.is_object() {
            return Err(Error::Storage);
        }
        Ok(Some(Entry { owner, leases }))
    }
    fn put(&self, canvas: &Self::Canvas, entry: Entry<JsValue>) -> Result<(), Error> {
        let value: JsValue = js_sys::Object::new().into();
        for (key, field) in [("owner", entry.owner), ("leases", entry.leases.into())] {
            if !js_sys::Reflect::set(&value, &key.into(), &field).map_err(|_| Error::Storage)? {
                return Err(Error::Storage);
            }
        }
        CANVASES.with(|map| map.set(canvas, &value));
        Ok(())
    }
    fn remove(&self, canvas: &Self::Canvas) {
        CANVASES.with(|map| map.delete(canvas));
    }
}
pub(crate) fn owner() -> JsValue {
    js_sys::Object::new().into()
}
