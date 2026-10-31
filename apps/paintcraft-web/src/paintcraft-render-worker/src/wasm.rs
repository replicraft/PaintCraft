use crate::*;
use wasm_bindgen::prelude::*;
fn error(e: paintcraft_core::types::Error) -> JsValue {
    JsValue::from_str(&text(&Value::object([
        ("code", Value::str(e.code.as_str())),
        ("message", Value::str(e.message)),
    ])))
}
#[wasm_bindgen]
#[derive(Debug)]
pub struct RenderWorker {
    service: Service,
}
#[wasm_bindgen]
impl RenderWorker {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &[u8], control: &str) -> std::result::Result<RenderWorker, JsValue> {
        Ok(Self {
            service: Service::open(source, control).map_err(error)?,
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
        self.service.take_frame().map(|(_, p)| p).map_err(error)
    }
    pub fn retire(&mut self) {
        self.service.retire();
    }
    pub fn binding(&self) -> String {
        text(&self.service.context())
    }
}
#[wasm_bindgen]
pub fn protocol_version() -> u32 {
    4
}
#[wasm_bindgen]
pub fn application_version() -> String {
    env!("CARGO_PKG_VERSION").into()
}
#[wasm_bindgen]
pub fn linear_memory_ceiling() -> u32 {
    LINEAR_MEMORY_BYTES as u32
}
