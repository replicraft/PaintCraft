//! Genuine staged Source8 ABI. No encoded source Vec/slice or arbitrary callback.
use crate::blend_service::{self, LayerBlendService};
use paintcraft_core::types::{Error, json::Value};
use wasm_bindgen::prelude::*;
fn error(e: Error) -> JsValue {
    JsValue::from_str(&crate::text(&Value::object([
        ("code", Value::str(e.code.as_str())),
        ("message", Value::str(e.message)),
        (
            "operationIndex",
            e.operation_index
                .map_or(Value::Null, |i| Value::Int(i as i64)),
        ),
    ])))
}
#[wasm_bindgen]
#[repr(transparent)]
#[derive(Debug)]
pub struct LayerBlendRenderWorker {
    service: LayerBlendService,
}
#[wasm_bindgen]
impl LayerBlendRenderWorker {
    #[wasm_bindgen(constructor)]
    pub fn new() -> std::result::Result<Self, JsValue> {
        Ok(Self {
            service: LayerBlendService::new().map_err(error)?,
        })
    }
    #[wasm_bindgen(js_name=controlPtr)]
    pub fn control_ptr(&mut self) -> usize {
        self.service.control_ptr()
    }
    #[wasm_bindgen(js_name=controlLen)]
    pub fn control_len(&self) -> usize {
        self.service.control_len()
    }
    #[wasm_bindgen(js_name=hasOpenManifest)]
    pub fn has_open_manifest(&self) -> bool {
        self.service.has_open_manifest()
    }
    pub fn stage(&mut self) -> std::result::Result<u32, JsValue> {
        self.service.stage().map_err(error)
    }
    #[wasm_bindgen(js_name=sourcePtr)]
    pub fn source_ptr(&mut self) -> std::result::Result<usize, JsValue> {
        self.service.source_ptr().map_err(error)
    }
    #[wasm_bindgen(js_name=sourceLen)]
    pub fn source_len(&self) -> std::result::Result<usize, JsValue> {
        self.service.source_len().map_err(error)
    }
    pub fn seal(&mut self) -> std::result::Result<u32, JsValue> {
        self.service.seal().map_err(error)
    }
    #[wasm_bindgen(js_name=beginView)]
    pub fn begin_view(&mut self) -> std::result::Result<u32, JsValue> {
        self.service.begin_view().map_err(error)
    }
    pub fn step(&mut self) -> std::result::Result<u32, JsValue> {
        self.service.step().map_err(error)
    }
    #[wasm_bindgen(js_name=rgbaPtr)]
    pub fn rgba_ptr(&self) -> std::result::Result<usize, JsValue> {
        self.service.rgba_ptr().map_err(error)
    }
    #[wasm_bindgen(js_name=rgbaLen)]
    pub fn rgba_len(&self) -> std::result::Result<usize, JsValue> {
        self.service.rgba_len().map_err(error)
    }
    #[wasm_bindgen(js_name=patchFact)]
    pub fn patch_fact(&self, index: u32) -> std::result::Result<u64, JsValue> {
        self.service.patch_fact(index).map_err(error)
    }
    pub fn ack(&mut self) -> std::result::Result<u32, JsValue> {
        self.service.ack().map_err(error)
    }
    pub fn cancel(&mut self) -> std::result::Result<u32, JsValue> {
        self.service.cancel().map_err(error)
    }
    #[wasm_bindgen(js_name=workUsed)]
    pub fn work_used(&self, noise: bool) -> u64 {
        self.service.work_used(noise)
    }
    /// Consuming return means actual Rust data/factories and singleton are retired.
    pub fn close(self) -> std::result::Result<u32, JsValue> {
        self.service.close().map_err(error)
    }
}
#[wasm_bindgen]
pub fn layer_blend_runtime_fact(index: u32) -> std::result::Result<u64, JsValue> {
    blend_service::runtime_fact(index).map_err(error)
}
#[wasm_bindgen]
pub fn layer_blend_control_fact(index: u32) -> std::result::Result<u64, JsValue> {
    blend_service::control_fact(index).map_err(error)
}
#[wasm_bindgen]
pub fn layer_blend_profile_word(index: u32) -> std::result::Result<u32, JsValue> {
    blend_service::profile_word(index).map_err(error)
}
#[wasm_bindgen]
pub fn layer_blend_live_instances() -> u32 {
    LayerBlendService::live_instances()
}
