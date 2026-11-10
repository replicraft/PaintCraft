//! Narrow genuine controller data adapter; no numeric JS byte/work authority.
use super::{denied, snapshot::Snapshot8};
use js_sys::{Array, Function, Object, Promise, Reflect};
use paintcraft_core::types::{Error, ErrorCode, Result, policy::hex};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
#[wasm_bindgen::prelude::wasm_bindgen(module = "/src/lib/layer-blend-preview-client.js")]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(catch,js_name=beginLayerBlendPreview8WithHostControl)]
    pub fn begin_host(bytes: f64) -> std::result::Result<JsValue, JsValue>;
}
#[wasm_bindgen::prelude::wasm_bindgen(module = "/src/lib/layer-blend-main-owner.js")]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(catch,js_name=mainBlendOwner8Facade)]
    pub fn facade(handle: &JsValue) -> std::result::Result<JsValue, JsValue>;
    #[wasm_bindgen::prelude::wasm_bindgen(catch,js_name=mainBlendRetirementTick8)]
    pub fn tick() -> std::result::Result<Promise, JsValue>;
    #[wasm_bindgen::prelude::wasm_bindgen(catch,js_name=deferMainBlendCredit8)]
    pub fn defer_credit(handle: &JsValue) -> std::result::Result<Promise, JsValue>;
}
pub(super) fn error(value: JsValue) -> Error {
    let get = |name: &str| Reflect::get(&value, &name.into()).ok();
    let raw_code = get("code");
    let code = [
        ("input.schema", ErrorCode::InputSchema),
        ("InputSchema", ErrorCode::InputSchema),
        ("policy.unknown", ErrorCode::PolicyUnknown),
        ("PolicyUnknown", ErrorCode::PolicyUnknown),
        ("reference.unknown_alias", ErrorCode::ReferenceUnknownAlias),
        ("ReferenceUnknownAlias", ErrorCode::ReferenceUnknownAlias),
        ("reference.unknown_id", ErrorCode::ReferenceUnknownId),
        ("ReferenceUnknownId", ErrorCode::ReferenceUnknownId),
        ("target.invalid", ErrorCode::TargetInvalid),
        ("TargetInvalid", ErrorCode::TargetInvalid),
        ("capability.unsupported", ErrorCode::CapabilityUnsupported),
        ("CapabilityUnsupported", ErrorCode::CapabilityUnsupported),
        ("conflict.stale_base", ErrorCode::ConflictStaleBase),
        ("ConflictStaleBase", ErrorCode::ConflictStaleBase),
        ("resource.limit", ErrorCode::ResourceLimit),
        ("ResourceLimit", ErrorCode::ResourceLimit),
        ("operation.cancelled", ErrorCode::OperationCancelled),
        ("OperationCancelled", ErrorCode::OperationCancelled),
        ("storage.failed", ErrorCode::StorageFailed),
        ("StorageFailed", ErrorCode::StorageFailed),
        ("invariant.violation", ErrorCode::InvariantViolation),
        ("InvariantViolation", ErrorCode::InvariantViolation),
    ]
    .into_iter()
    .find_map(|(name, code)| {
        raw_code
            .as_ref()
            .is_some_and(|v| string_equal(v, name))
            .then_some(code)
    })
    .unwrap_or(ErrorCode::CodecFailed);
    let message = get("message")
        .and_then(|v| v.as_string())
        .or_else(|| value.as_string())
        .unwrap_or_else(|| "Source8 genuine controller failure".into());
    let mut error = Error::new(code, message);
    if let Some(n) = get("operationIndex")
        .and_then(|v| v.as_f64())
        .filter(|n| n.is_finite() && n.fract() == 0.0 && *n >= 0.0 && *n <= u32::MAX as f64)
    {
        error = error.at(n as usize);
    }
    error
}
pub(super) fn fail(error: &Error) -> JsValue {
    let value = js_sys::Error::new(&error.message);
    let _ = Reflect::set(&value, &"code".into(), &error.code.as_str().into());
    let index = error
        .operation_index
        .map(|n| JsValue::from_f64(n as f64))
        .unwrap_or(JsValue::NULL);
    let _ = Reflect::set(&value, &"operationIndex".into(), &index);
    value.into()
}
pub(super) fn field(value: &JsValue, name: &str) -> Result<JsValue> {
    Reflect::get(value, &name.into()).map_err(error)
}
/// Frozen protocol requests must expose data fields; reject an accessor before
/// invoking it. The returned descriptor is a native JS metadata temporary.
pub(super) fn data(value: &JsValue, name: &str) -> Result<JsValue> {
    if !value.is_object() || value.is_null() {
        return Err(denied());
    }
    let object = value.unchecked_ref::<Object>();
    let descriptor = Object::get_own_property_descriptor(object, &name.into());
    if descriptor.is_undefined() || !Reflect::has(&descriptor, &"value".into()).map_err(error)? {
        return Err(denied());
    }
    field(&descriptor, "value")
}
pub(super) fn call(value: &JsValue, name: &str, args: &[JsValue]) -> Result<JsValue> {
    let f = field(value, name)?.dyn_into::<Function>().map_err(error)?;
    let a = Array::new();
    for value in args {
        a.push(value);
    }
    f.apply(value, &a).map_err(error)
}
pub(super) async fn wait(value: JsValue) -> Result<JsValue> {
    JsFuture::from(Promise::resolve(&value))
        .await
        .map_err(error)
}
pub(super) fn set(value: &Object, name: &str, item: impl Into<JsValue>) -> Result<()> {
    Reflect::set(value, &name.into(), &item.into()).map_err(error)?;
    Ok(())
}
pub(super) fn string_equal(value: &JsValue, expected: &str) -> bool {
    value.is_string()
        && value.unchecked_ref::<js_sys::JsString>().length() == expected.len() as u32
        && *value == JsValue::from_str(expected)
}
fn bounded_string(value: JsValue, max: u32) -> Result<String> {
    if !value.is_string() || value.unchecked_ref::<js_sys::JsString>().length() > max {
        return Err(denied());
    }
    value.as_string().ok_or_else(denied)
}
pub(super) fn decimal(value: &JsValue, name: &str) -> Result<u64> {
    let s = bounded_string(data(value, name)?, 20)?;
    if s.is_empty()
        || s.len() > 20
        || s.len() > 1 && s.starts_with('0')
        || !s.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(denied());
    }
    s.parse().map_err(|_| denied())
}
pub(super) fn numbers(values: &[i64]) -> Array {
    values
        .iter()
        .map(|n| JsValue::from_f64(*n as f64))
        .collect()
}
pub(super) fn rect(value: &JsValue) -> Result<paintcraft_core::types::Rect> {
    if !Array::is_array(value) {
        return Err(denied());
    }
    let a = value.unchecked_ref::<Array>();
    if a.length() != 4 {
        return Err(denied());
    }
    let mut v = [0i64; 4];
    for (at, dest) in v.iter_mut().enumerate() {
        let n = data(value, &at.to_string())?.as_f64().ok_or_else(denied)?;
        if !n.is_finite() || n.fract() != 0.0 || n.abs() > 1048576.0 {
            return Err(denied());
        }
        *dest = n as i64;
    }
    paintcraft_core::types::Rect::new(v[0], v[1], v[2] - v[0], v[3] - v[1]).ok_or_else(denied)
}
pub(super) fn ready(value: &JsValue, profile: [u8; 32]) -> Result<()> {
    use paintcraft_web_render_worker::blend_service::{LayerBlendService, runtime_fact};
    if !string_equal(&data(value, "profileSha256")?, &hex(&profile)) {
        return Err(denied());
    }
    let ready = data(value, "ready")?;
    if !string_equal(&data(&ready, "kind")?, "ready") {
        return Err(denied());
    }
    let facts = data(&ready, "runtimeFacts")?;
    for (i, key) in [
        "protocol",
        "registryContract",
        "sourceFormat",
        "appearanceResource",
        "blendResource",
        "arithmeticRevision",
        "workRevision",
        "queryRevision",
        "sliceRevision",
        "nativeEdge",
        "displayRevision",
    ]
    .iter()
    .enumerate()
    {
        if data(&facts, key)?.as_f64() != Some(runtime_fact(i as u32)? as f64) {
            return Err(denied());
        }
    }
    let limits = data(&facts, "limits")?;
    for (i, key) in [
        "jobBytes",
        "encodedBytes",
        "baseWork",
        "noiseWork",
        "payloadBytes",
        "linearBytes",
        "outputCells",
        "patchEdge",
    ]
    .iter()
    .enumerate()
    {
        let wanted = runtime_fact(11 + i as u32)?;
        let value = data(&limits, key)?;
        if i < 6 {
            if !string_equal(&value, &wanted.to_string()) {
                return Err(denied());
            }
        } else if value.as_f64() != Some(wanted as f64) {
            return Err(denied());
        }
    }
    let controls = data(&facts, "controls")?;
    for (key, n) in ["serviceBytes", "stageBytes", "viewBytes"]
        .into_iter()
        .zip(LayerBlendService::control_quote()?)
    {
        if decimal(&controls, key)? != n {
            return Err(denied());
        }
    }
    Ok(())
}
pub(super) fn open(snapshot: &Snapshot8) -> Result<JsValue> {
    if snapshot.roles.roles[8] != snapshot.workspace.workspace_bytes {
        return Err(denied());
    }
    let value = Object::new();
    let identity = &snapshot.identity;
    for (key, text) in [
        ("kind", "open".to_string()),
        ("owner", identity.owner.to_string()),
        ("document", identity.document.document_id.to_string()),
        ("sourceState", identity.document.state_id.to_string()),
        ("draft", identity.draft.to_string()),
        ("sourceSha256", hex(&snapshot.demand.source_sha256)),
        ("profileSha256", hex(&identity.profile)),
    ] {
        set(&value, key, text)?;
    }
    set(&value, "protocol", 8)?;
    set(&value, "id", 1)?;
    let ticket = Object::new();
    set(&ticket, "state", identity.ticket.state_id.to_string())?;
    set(
        &ticket,
        "epoch",
        identity.ticket.publication_epoch.to_string(),
    )?;
    set(&value, "publicTicket", ticket)?;
    set(
        &value,
        "canvas",
        numbers(&[
            i64::from(snapshot.demand.canvas[0]),
            i64::from(snapshot.demand.canvas[1]),
        ]),
    )?;
    set(
        &value,
        "byteLength",
        snapshot.demand.encoded_bytes.to_string(),
    )?;
    set(&value, "storedNonNormal", true)?;
    set(&value, "storedNoise", snapshot.demand.stored_noise)?;
    set(
        &value,
        "mode",
        if snapshot.demand.stored_noise {
            "noise"
        } else {
            "base"
        },
    )?;
    let roles = Object::new();
    for (key, n) in ["E", "P", "M", "A", "I", "C", "Q", "K", "W", "F", "O", "H"]
        .into_iter()
        .zip(snapshot.roles.roles)
    {
        set(&roles, key, n.to_string())?;
    }
    set(&value, "roles", roles)?;
    set(
        &value,
        "pairedBytes",
        snapshot.roles.paired_noise_bytes.to_string(),
    )?;
    set(
        &value,
        "baseWork",
        paintcraft_web_render_worker::blend_service::runtime_fact(13)?.to_string(),
    )?;
    set(
        &value,
        "noiseWork",
        if snapshot.demand.stored_noise {
            paintcraft_web_render_worker::blend_service::runtime_fact(14)?
        } else {
            0
        }
        .to_string(),
    )?;
    Ok(value.into())
}
