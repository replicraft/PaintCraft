//! Separate Source6 ABI. No ordinary constructor until policy qualification.
use crate::{
    noise_service::{NoiseService, SlicePolicy},
    *,
};
use wasm_bindgen::prelude::*;
fn error(e: Error) -> JsValue {
    JsValue::from_str(&text(&Value::object([
        ("code", Value::str(e.code.as_str())),
        ("message", Value::str(e.message)),
    ])))
}
#[wasm_bindgen]
#[derive(Debug)]
pub struct NoiseRenderWorker {
    service: NoiseService,
}
#[wasm_bindgen]
impl NoiseRenderWorker {
    /// Explicit qualification entry only; no JS-supplied policy/quantum argument.
    #[wasm_bindgen(js_name = openQualified)]
    pub fn open_qualified(source: &[u8], control: &str) -> std::result::Result<Self, JsValue> {
        Ok(Self {
            service: NoiseService::open(source, control, SlicePolicy::selected_qualification())
                .map_err(error)?,
        })
    }
    pub fn prepare(&mut self, control: &str) -> std::result::Result<String, JsValue> {
        self.service
            .prepare(control)
            .map(|v| text(&v))
            .map_err(error)
    }
    pub fn step(&mut self) -> std::result::Result<bool, JsValue> {
        self.service.step().map_err(error)
    }
    pub fn frame(&mut self) -> std::result::Result<Vec<u8>, JsValue> {
        self.service.copy_frame_for_wasm().map_err(error)
    }
    pub fn binding(&self) -> String {
        text(&self.service.context())
    }
    pub fn retire(&mut self) {
        self.service.retire();
    }
    pub fn cancel(&mut self, generation: &str) -> std::result::Result<(), JsValue> {
        if generation.len() > 20 {
            return Err(error(bad("Source6 generation extent")));
        }
        let generation = decimal(&Value::str(generation)).map_err(error)?;
        self.service.cancel(generation).map_err(error)
    }
}
#[wasm_bindgen]
pub fn scoped_protocol_version() -> u32 {
    6
}
#[wasm_bindgen]
pub fn scoped_capabilities(control: &str) -> std::result::Result<String, JsValue> {
    NoiseService::capabilities(control, &|| Ok(()))
        .map(|v| text(&v))
        .map_err(error)
}
#[wasm_bindgen]
pub fn scoped_control_quote() -> std::result::Result<String, JsValue> {
    let q = NoiseService::control_quote().map_err(error)?;
    Ok(text(&Value::object([
        ("retainedBytes", Value::Int(q.retained_bytes as i64)),
        ("handleBytes", Value::Int(q.handle_bytes as i64)),
        ("frameBytes", Value::Int(q.frame_bytes as i64)),
        ("extraOutputBytes", Value::Int(q.extra_output_bytes as i64)),
    ])))
}
#[wasm_bindgen]
pub fn scoped_qualification_policy() -> String {
    let p = SlicePolicy::selected_qualification();
    text(&Value::object([
        ("nativeEdge", Value::Int(p.native_edge())),
        ("baseSliceLimit", Value::Int(p.base_limit() as i64)),
        ("noiseSliceLimit", Value::Int(p.noise_limit() as i64)),
    ]))
}
