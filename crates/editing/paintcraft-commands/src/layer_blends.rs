//! New closed appearance-v2 grammar; numerical filter inventory is unchanged.
use super::*;
use paintcraft_core::types::LayerBlendMode;
pub const LAYER_BLEND_BATCH_SCHEMA: &str = "paintcraft.batch/layer-appearance-v2";
pub(super) fn parse(value: &Value) -> Result<Op> {
    let o = obj(value, &["op", "layer_ref", "blend_mode"], &[])?;
    let mode = match o["blend_mode"].as_str() {
        Some("normal") => LayerBlendMode::Normal,
        Some("multiply") => LayerBlendMode::Multiply,
        Some("screen") => LayerBlendMode::Screen,
        _ => return Err(bad("blend_mode must be normal, multiply or screen")),
    };
    Ok(Op::LayerSetBlendMode {
        layer: layer_ref(&o["layer_ref"])?,
        mode,
    })
}
pub(super) fn required(value: &Value) -> Result<Vec<&'static str>> {
    let mut expected = layer_appearance::required(value)?;
    let ops = value
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("operations must be an array"))?;
    if ops
        .iter()
        .any(|o| o.get("op").and_then(Value::as_str) == Some("layer.set_blend_mode"))
    {
        expected.push("layer-blend-modes.v1");
    }
    expected.sort_unstable();
    Ok(expected)
}
pub(super) fn validate(value: &Value) -> Result<()> {
    let expected = required(value)?;
    let got = value
        .get("required_layer_features")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("appearance-v2 requires required_layer_features"))?;
    if got.len() != expected.len()
        || got
            .iter()
            .zip(expected)
            .any(|(got, want)| got.as_str() != Some(want))
    {
        return Err(bad(
            "layer requirements must equal the sorted operation inventory",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests;
