//! Closed final transport wrapper and fixed-context BEGIN controls.
use super::{echo, prepare::Prepared7};
use crate::app::filter_editing::render::View;
use paintcraft_commands::FilterOp;
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    json::{self, Value},
    policy::hex,
};
use paintcraft_formats::export::layer_appearance::Requirements7;
fn bad() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Source7 bounded control extent")
}
fn integer(n: u64) -> Result<Value> {
    if n > 9_007_199_254_740_991 {
        return Err(bad());
    }
    Ok(Value::Int(n as i64))
}
fn pairs(requirements: &Requirements7) -> Value {
    Value::Array(
        requirements
            .requirements()
            .pairs()
            .map(|(profile, codec)| {
                Value::object([
                    ("profile_id", Value::str(profile)),
                    ("codec_id", Value::str(codec)),
                ])
            })
            .collect(),
    )
}
fn bounded(value: Value) -> Result<Value> {
    if json::canonical(&value).len() > 8192 {
        return Err(bad());
    }
    Ok(value)
}
pub(in crate::app::filter_editing) fn final_wrapper(prepared: &Prepared7) -> Result<Value> {
    let capture = &prepared.capture;
    let frames = &prepared.frames;
    let admission = frames.sealed.admission;
    if admission.base_work.checked_add(frames.sealed.main_spent)
        != Some(capture.admission.base_work)
        || admission.workspace_bytes != capture.admission.workspace_bytes
        || admission.noise_work != capture.admission.noise_work
        || admission.work_authority != capture.admission.work_authority
    {
        return Err(Error::new(
            ErrorCode::ConflictStaleBase,
            "Source7 prefix handoff changed",
        ));
    }
    let mut source = frames.source.value();
    let Value::Object(fields) = &mut source else {
        unreachable!()
    };
    fields.insert("pairs".into(), pairs(&frames.source_requirements));
    let masks = prepared.filter_masks;
    let plan = Value::object([
        ("protocol", Value::Int(7)),
        ("registryContract", Value::Int(3)),
        ("owner", echo::owner(capture)),
        ("draft", Value::str(capture.draft.to_string())),
        (
            "document",
            Value::str(capture.document.document_id.to_string()),
        ),
        ("sourceSha256", Value::str(hex(&prepared.source_sha256))),
        (
            "size",
            Value::Array(vec![
                Value::Int(i64::from(capture.document.canvas.0)),
                Value::Int(i64::from(capture.document.canvas.1)),
            ]),
        ),
        ("layers", integer(capture.document.layers.len() as u64)?),
        ("residentBytes", integer(prepared.resident)?),
        ("metadataBytes", integer(prepared.metadata_bytes)?),
        ("metadataCharge", integer(prepared.metadata_charge)?),
        ("filterDigest", Value::str(hex(&prepared.filter_digest))),
        ("maskCount", integer(masks.masks)?),
        ("maskBands", integer(masks.bands)?),
        ("maskRuns", integer(masks.runs)?),
        ("maskDecodeBytes", integer(masks.decode_bytes)?),
        ("sourceRequirements", source),
        (
            "appearance",
            frames.source_requirements.appearance().value(),
        ),
        ("admission", admission.value()?),
        ("slice", admission.slice_value()),
    ]);
    let hello = Value::object([
        ("protocol", Value::Int(7)),
        ("registryContract", Value::Int(3)),
        ("pairs", pairs(&frames.union_requirements)),
        ("appearance", frames.union_requirements.appearance().value()),
        ("admission", admission.value()?),
        ("slice", admission.slice_value()),
    ]);
    bounded(Value::object([("plan", plan), ("hello", hello)]))
}
pub(in crate::app::filter_editing) fn begin(
    prepared: &Prepared7,
    generation: u64,
    view: View,
    render: bool,
) -> Result<Value> {
    let target = if let Some(operation) = &prepared.capture.operation {
        let (layer, _) = operation.stable_uuid_refs().ok_or_else(bad)?;
        Value::str(layer.to_string())
    } else {
        Value::Null
    };
    bounded(Value::object([
        ("protocol", Value::Int(7)),
        ("registryContract", Value::Int(3)),
        ("owner", echo::owner(&prepared.capture)),
        ("draft", Value::str(prepared.capture.draft.to_string())),
        ("sourceSha256", Value::str(hex(&prepared.source_sha256))),
        ("generation", Value::str(generation.to_string())),
        ("operation", prepared.operation.clone()),
        ("target", target),
        ("view", echo::view(view)),
        ("render", Value::Bool(render)),
        ("candidateRequirements", prepared.frames.candidate.value()),
        ("unionRequirements", prepared.frames.union.value()),
        (
            "admissionSha256",
            Value::str(hex(&prepared.frames.admission_sha256)),
        ),
    ]))
}
pub(in crate::app::filter_editing) fn same_operation(
    prepared: &Prepared7,
    operation: Option<&FilterOp>,
) -> Result<()> {
    if prepared.capture.operation.as_ref() != operation {
        return Err(Error::new(
            ErrorCode::ConflictStaleBase,
            "Source7 typed intent needs acknowledged new OPEN",
        ));
    }
    Ok(())
}
