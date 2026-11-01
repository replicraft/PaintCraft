//! Separate Source7 ABI; no old Worker type, getter or dispatch is widened.
use crate::{layer_service::LayerAppearanceService, *};
use wasm_bindgen::prelude::*;
fn error(e: Error) -> JsValue {
    JsValue::from_str(&text(&Value::object([
        ("code", Value::str(e.code.as_str())),
        ("message", Value::str(e.message)),
    ])))
}
#[wasm_bindgen]
#[derive(Debug)]
pub struct LayerAppearanceRenderWorker {
    service: LayerAppearanceService,
}
#[wasm_bindgen]
impl LayerAppearanceRenderWorker {
    /// Strict protocol7 control selects only compiled fixed Base|Noise modes.
    pub fn open(source: &[u8], control: &str) -> std::result::Result<Self, JsValue> {
        Ok(Self {
            service: LayerAppearanceService::open(source, control).map_err(error)?,
        })
    }
    pub fn prepare(&mut self, control: &str) -> std::result::Result<String, JsValue> {
        self.service.prepare(control).map(text).map_err(error)
    }
    pub fn step(&mut self) -> std::result::Result<bool, JsValue> {
        self.service.step().map_err(error)
    }
    pub fn frame(&mut self) -> std::result::Result<Vec<u8>, JsValue> {
        self.service.copy_frame_for_wasm().map_err(error)
    }
    pub fn binding(&self) -> String {
        text(self.service.context())
    }
    #[wasm_bindgen(js_name=prepareCopy)]
    pub fn prepare_copy(
        &mut self,
        begin: &str,
        output: &str,
    ) -> std::result::Result<String, JsValue> {
        self.service
            .prepare_copy(begin, output, &|| Ok(()))
            .map(text)
            .map_err(error)
    }
    #[wasm_bindgen(js_name=encodedFrame)]
    pub fn encoded_frame(&mut self, mask: &[u8]) -> std::result::Result<Vec<u8>, JsValue> {
        self.service
            .encode_copy_for_wasm(mask, &|| Ok(()))
            .map_err(error)
    }
    pub fn retire(&mut self) {
        self.service.retire();
    }
    pub fn cancel(&mut self, generation: &str) -> std::result::Result<(), JsValue> {
        if generation.len() > 20 {
            return Err(error(bad("Source7 generation length")));
        }
        self.service
            .cancel(decimal(&Value::str(generation)).map_err(error)?)
            .map_err(error)
    }
}
#[wasm_bindgen]
pub fn layer_appearance_protocol_version() -> u32 {
    7
}
#[wasm_bindgen]
pub fn layer_appearance_ready(noise: bool) -> std::result::Result<String, JsValue> {
    LayerAppearanceService::ready(noise)
        .map(|v| text(&v))
        .map_err(error)
}
#[wasm_bindgen]
pub fn layer_appearance_capabilities(control: &str) -> std::result::Result<String, JsValue> {
    LayerAppearanceService::capabilities(control, &|| Ok(()))
        .map(|v| text(&v))
        .map_err(error)
}
