//! Additive closed stored-coefficient rotation grammar; no admission authority.
use super::*;
pub(super) fn parse(value: &Value) -> Result<Op> {
    let selected = value.get("op").and_then(Value::as_str) == Some("raster.rotate_selection");
    let fields: &[&str] = if selected {
        &["op", "coefficients", "layer_ref", "clear"]
    } else {
        &["op", "coefficients"]
    };
    let o = obj(value, fields, &[])?;
    let pair = o["coefficients"]
        .as_array()
        .filter(|a| a.len() == 2)
        .ok_or_else(|| bad("rotation needs two Q30 integers"))?;
    let q = i64::from(paintcraft_core::types::rotation::COEFFICIENT_ONE);
    let coefficients = (
        int_in(&pair[0], -q, q, "cosine coefficient")? as i32,
        int_in(&pair[1], -q, q, "sine coefficient")? as i32,
    );
    // Validate the dimension-independent closed coefficient policy; real geometry
    // and all materialization admission remain the actual editor's responsibility.
    paintcraft_core::types::rotation::Rotation::new(
        paintcraft_core::types::Rect::new(0, 0, 1, 1).unwrap(),
        coefficients,
    )?;
    Ok(if selected {
        Op::RotateSelection {
            layer: layer_ref(&o["layer_ref"])?,
            coefficients,
            clear: color(&o["clear"])?,
        }
    } else {
        Op::CanvasRotate { coefficients }
    })
}
