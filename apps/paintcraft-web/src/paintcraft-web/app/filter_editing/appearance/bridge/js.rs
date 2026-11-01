//! Bounded control conversion only; no source hashing, numerical math or Worker module.
use super::*;
use js_sys::{Array, Function, Object, Reflect};
use wasm_bindgen::JsCast;
pub(super) fn error(value: JsValue) -> Error {
    let code = Reflect::get(&value, &"code".into())
        .ok()
        .and_then(|v| v.as_string());
    Error::new(
        match code.as_deref() {
            Some("OperationCancelled" | "operation.cancelled") => ErrorCode::OperationCancelled,
            Some("ResourceLimit" | "resource.limit") => ErrorCode::ResourceLimit,
            Some("ConflictStaleBase" | "conflict.stale_base") => ErrorCode::ConflictStaleBase,
            Some("CapabilityUnsupported" | "capability.unsupported") => {
                ErrorCode::CapabilityUnsupported
            }
            Some("InputSchema" | "input.schema") => ErrorCode::InputSchema,
            _ => ErrorCode::CodecFailed,
        },
        "Source7 Worker request failed",
    )
}
pub(super) fn bad(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
pub(super) fn field(value: &JsValue, key: &str) -> Result<JsValue> {
    Reflect::get(value, &key.into()).map_err(error)
}
pub(super) fn call(value: &JsValue, key: &str, args: &[JsValue]) -> Result<JsValue> {
    let arguments = Array::new();
    for arg in args {
        arguments.push(arg);
    }
    field(value, key)?
        .dyn_into::<Function>()
        .map_err(error)?
        .apply(value, &arguments)
        .map_err(error)
}
pub(super) async fn promise(value: JsValue) -> Result<JsValue> {
    wasm_bindgen_futures::JsFuture::from(value.dyn_into::<js_sys::Promise>().map_err(error)?)
        .await
        .map_err(error)
}
pub(super) fn value(value: &Value) -> Result<JsValue> {
    let bytes = paintcraft_core::types::json::canonical(value);
    if bytes.len() > 8192 {
        return Err(bad("Source7 control UTF-8 bound"));
    }
    js_sys::JSON::parse(std::str::from_utf8(&bytes).map_err(|_| bad("Source7 control UTF-8"))?)
        .map_err(error)
}
pub(super) fn parse(value: &JsValue) -> Result<Value> {
    let text = js_sys::JSON::stringify(value)
        .map_err(error)?
        .as_string()
        .ok_or_else(|| bad("Source7 control object"))?;
    if text.len() > 8192 {
        return Err(bad("Source7 returned control bound"));
    }
    paintcraft_core::types::json::parse(text.as_bytes())
        .map_err(|_| bad("Source7 returned control JSON"))
}
pub(super) fn frame_metadata(result: &JsValue, render: bool) -> Result<Value> {
    const KEYS: [&str; 19] = [
        "kind",
        "protocol",
        "registryContract",
        "generation",
        "owner",
        "draft",
        "sourceSha256",
        "target",
        "view",
        "width",
        "height",
        "region",
        "level",
        "operationSha256",
        "admissionSha256",
        "sourceRequirements",
        "candidateRequirements",
        "unionRequirements",
        "workQuote",
    ];
    let object = result
        .dyn_ref::<Object>()
        .ok_or_else(|| bad("Source7 result object"))?;
    let keys = Object::keys(object);
    if keys.length() != (KEYS.len() + 1 + usize::from(render)) as u32 {
        return Err(bad("Source7 result closed fields"));
    }
    for key in keys.iter() {
        let key = key.as_string().ok_or_else(|| bad("Source7 result field"))?;
        if key != "release" && !(render && key == "pixels") && !KEYS.contains(&key.as_str()) {
            return Err(bad("Source7 unknown result field"));
        }
    }
    let metadata = Object::new();
    for key in KEYS {
        Reflect::set(&metadata, &key.into(), &field(result, key)?).map_err(error)?;
    }
    parse(&metadata.into())
}
