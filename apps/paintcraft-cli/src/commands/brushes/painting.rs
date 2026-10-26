use super::*;
use paintcraft_commands::Batch;
use paintcraft_core::types::{ids::parse_uuid, json};
use paintcraft_tools::sampled_brush::SampledStroke;
use schemars::JsonSchema;
use serde::Deserialize;
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Stroke {
    /// Integer [x,y] pairs, capped at 65,536, in point_scale units per pixel.
    pub points: Vec<[i64; 2]>,
    pub color: String,
    #[serde(default = "one")]
    pub point_scale: u32,
    pub layer_id: Option<String>,
}
fn one() -> u32 {
    1
}
pub fn paint(
    session: &mut Session,
    preset: &AdoptedPreset,
    stroke: Stroke,
    budget: &SharedBudget,
) -> Result<paintcraft_editor::session::Receipt> {
    let [first, rest @ ..] = stroke.points.as_slice() else {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "sampled stroke needs points",
        ));
    };
    if stroke.points.len() > 65_536 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "sampled point allowance exceeded",
        ));
    }
    let initial = stroke.layer_id.is_none() && session.document().layers.is_empty();
    let layer = match stroke.layer_id {
        Some(id) => parse_uuid(&id)
            .ok_or_else(|| Error::new(ErrorCode::InputSchema, "invalid layer UUID"))?,
        None => session
            .document()
            .roots
            .last()
            .copied()
            .unwrap_or_else(|| parse_uuid("00000000-0000-0000-0000-000000000000").unwrap()),
    };
    if !initial && !session.document().layers.contains_key(&layer) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "painting layer does not exist",
        ));
    }
    let color = Rgba::parse(&stroke.color)
        .ok_or_else(|| Error::new(ErrorCode::InputSchema, "color must be #RRGGBBAA"))?;
    let mut gesture = SampledStroke::begin(
        layer,
        color,
        preset.tip().clone(),
        (first[0], first[1]),
        stroke.point_scale,
        preset.spacing_q8(),
    )?;
    for &[x, y] in rest {
        gesture.append((x, y))?;
    }
    let resolved = gesture.finish(session.document().canvas_rect(), budget, &|| Ok(()))?;
    let operations = if initial {
        resolved.operations_with_first_layer(session.document().canvas, &|| Ok(()))?
    } else {
        resolved.operations(&|| Ok(()))?
    };
    if operations.is_empty() {
        let prepared = session.preparation()?.no_change(|| Ok(()))?;
        return session.publish_prepared(prepared, || Ok(()));
    }
    let add_initial = initial;
    let ticket = session.ticket();
    let value = Value::object([
        ("schema", Value::str("paintcraft.batch/v2")),
        ("atomic", Value::Bool(true)),
        ("policy", Value::str(POLICY)),
        (
            "base",
            Value::object([
                ("state_id", Value::str(ticket.state_id.to_string())),
                (
                    "publication_epoch",
                    Value::str(ticket.publication_epoch.to_string()),
                ),
            ]),
        ),
        ("operations", Value::Array(operations)),
    ]);
    let batch = Batch::from_json(&json::canonical(&value))?;
    let preparation = session.preparation()?;
    let prepared = if add_initial {
        preparation.apply_initial_mask_stroke(&batch, || Ok(()))?
    } else {
        preparation.apply_mask_stroke(&batch, || Ok(()))?
    };
    session.publish_prepared(prepared, || Ok(()))
}
