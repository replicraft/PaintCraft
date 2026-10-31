use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub fn dds_prepared_export_protocol_version() -> u16 {
    paintcraft_formats::dds::web_export::v7::PROTOCOL_VERSION
}
#[wasm_bindgen]
pub fn msp_prepared_export_protocol_version() -> u16 {
    paintcraft_formats::msp::web_export::v7::PROTOCOL_VERSION
}
#[wasm_bindgen]
pub fn dds_prepared_export_request(bytes: &[u8]) -> std::result::Result<Vec<u8>, JsValue> {
    super::dds(bytes).map_err(|e| JsValue::from_str(&e.to_string()))
}
#[wasm_bindgen]
pub fn msp_prepared_export_request(bytes: &[u8]) -> std::result::Result<Vec<u8>, JsValue> {
    super::msp(bytes).map_err(|e| JsValue::from_str(&e.to_string()))
}
