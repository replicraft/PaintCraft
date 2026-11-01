//! Compare the complete fixed Source7 context before copying any returned pixels.
use super::{capture::Capture7, frames::Frames7};
use crate::app::filter_editing::render::View;
use paintcraft_commands::FilterOp;
use paintcraft_core::types::{Error, ErrorCode, Result, json::Value, policy::hex};
use paintcraft_formats::export::layer_appearance::Admission7;

pub(in crate::app::filter_editing) fn owner(capture: &Capture7) -> Value {
    let ticket = capture.wire_ticket();
    Value::object([
        ("session", Value::str(capture.owner.to_string())),
        ("state", Value::str(ticket.state_id.to_string())),
        ("epoch", Value::str(ticket.publication_epoch.to_string())),
        ("frontend", Value::Int(3)),
    ])
}
pub(in crate::app::filter_editing) fn view(view: View) -> Value {
    Value::object([
        (
            "region",
            Value::Array(
                [
                    view.visible.x0,
                    view.visible.y0,
                    view.visible.width(),
                    view.visible.height(),
                ]
                .into_iter()
                .map(Value::Int)
                .collect(),
            ),
        ),
        ("level", Value::Int(i64::from(view.level))),
    ])
}
/// Only bounded fixed control copies occur here: no digest, source scan, private
/// staging, requirement projection, or fresh execution authority after OPEN.
pub(in crate::app::filter_editing) fn expected(
    capture: &Capture7,
    frames: &Frames7,
    source_sha256: &[u8; 32],
    operation_sha256: &[u8; 32],
    generation: u64,
    request_view: View,
    render: bool,
) -> Value {
    Value::object([
        (
            "kind",
            Value::str(if render { "frame" } else { "prepared" }),
        ),
        ("protocol", Value::Int(7)),
        ("registryContract", Value::Int(3)),
        ("owner", owner(capture)),
        ("draft", Value::str(capture.draft.to_string())),
        ("sourceSha256", Value::str(hex(source_sha256))),
        ("generation", Value::str(generation.to_string())),
        (
            "target",
            capture
                .operation
                .as_ref()
                .and_then(FilterOp::stable_uuid_refs)
                .map(|(layer, _)| Value::str(layer.to_string()))
                .unwrap_or(Value::Null),
        ),
        ("view", view(request_view)),
        ("width", Value::Int(i64::from(request_view.size[0]))),
        ("height", Value::Int(i64::from(request_view.size[1]))),
        (
            "region",
            Value::Array(
                [
                    request_view.region.x0,
                    request_view.region.y0,
                    request_view.region.width(),
                    request_view.region.height(),
                ]
                .into_iter()
                .map(Value::Int)
                .collect(),
            ),
        ),
        ("level", Value::Int(i64::from(request_view.level))),
        ("operationSha256", Value::str(hex(operation_sha256))),
        ("admissionSha256", Value::str(hex(&frames.admission_sha256))),
        ("sourceRequirements", frames.source.value()),
        ("candidateRequirements", frames.candidate.value()),
        ("unionRequirements", frames.union.value()),
    ])
}
pub(in crate::app::filter_editing) fn validate(
    actual: &Value,
    expected: &Value,
    admission: Admission7,
) -> Result<()> {
    let (Value::Object(actual), Value::Object(expected)) = (actual, expected) else {
        return Err(changed());
    };
    if actual.len() != expected.len() + 1
        || expected
            .iter()
            .any(|(key, value)| actual.get(key) != Some(value))
    {
        return Err(changed());
    }
    let quote = actual.get("workQuote").ok_or_else(changed)?;
    let Value::Object(fields) = quote else {
        return Err(changed());
    };
    if fields.len() != 6 {
        return Err(changed());
    }
    let get = |key| fields.get(key).ok_or_else(changed).and_then(decimal);
    let base = get("baseWork")?;
    let noise = get("noiseWork")?;
    let total = get("totalWork")?;
    let actual_mask = get("maskWorkActual")?;
    let upper = get("maskWorkUpper")?;
    let margin = get("maskWorkMargin")?;
    if base > admission.base_work
        || noise > admission.noise_work
        || base.checked_add(noise) != Some(total)
        || actual_mask.checked_add(margin) != Some(upper)
        || upper > base
    {
        return Err(changed());
    }
    Ok(())
}
fn decimal(value: &Value) -> Result<u64> {
    let text = value.as_str().ok_or_else(changed)?;
    if text.is_empty()
        || text.len() > 20
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(changed());
    }
    text.parse().map_err(|_| changed())
}
fn changed() -> Error {
    Error::new(ErrorCode::CodecFailed, "Source7 complete binding changed")
}
