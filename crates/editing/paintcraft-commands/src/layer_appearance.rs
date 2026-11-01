//! Closed layer-appearance batch grammar, independent from filter inventory.
use super::*;
pub const LAYER_APPEARANCE_BATCH_SCHEMA: &str = "paintcraft.batch/layer-appearance-v1";
pub(super) fn required(value: &Value) -> Result<Vec<&'static str>> {
    let operations = value
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("operations must be an array"))?;
    if operations.len() > MAX_OPERATIONS {
        return Err(bad("operations must contain 1..=1024 items"));
    }
    Ok(
        if operations.iter().any(|o| {
            o.get("op")
                .and_then(Value::as_str)
                .is_some_and(|n| n.starts_with("layer.mask."))
        }) {
            vec!["layer-masks.v1"]
        } else {
            vec![]
        },
    )
}
pub(super) fn validate(value: &Value) -> Result<()> {
    let expected = required(value)?;
    let declared = value
        .get("required_layer_features")
        .and_then(Value::as_array)
        .ok_or_else(|| bad("layer appearance batch needs required_layer_features"))?;
    if declared.len() != expected.len()
        || declared
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual.as_str() != Some(expected))
    {
        return Err(bad(
            "layer requirements must equal the sorted operation inventory",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests;
