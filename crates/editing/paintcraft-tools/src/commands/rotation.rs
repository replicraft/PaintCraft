//! Closed ordinary commands; no mutation, preview or resource authority.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, rotation::Rotation};
use paintcraft_raster::Selection;
pub fn rotate_source(
    document: &paintcraft_document::Document,
    selection: &Selection,
) -> Result<Option<Rect>> {
    super::stretch_skew_source(document, selection)
}
pub fn rotate_image(
    document: &paintcraft_document::Document,
    selection: Selection,
    target: Option<Uuid>,
    coefficients: (i32, i32),
    clear: Rgba,
) -> Result<Vec<Value>> {
    let Some(area) = rotate_source(document, &selection)? else {
        return Ok(Vec::new());
    };
    let geometry = Rotation::new(area, coefficients)?;
    if geometry.is_identity() {
        return Ok(Vec::new());
    }
    let selected = selection != Selection::Inactive;
    let mut fields = vec![
        (
            "op",
            Value::str(if selected {
                "raster.rotate_selection"
            } else {
                "canvas.rotate"
            }),
        ),
        (
            "coefficients",
            Value::Array(vec![
                Value::Int(i64::from(coefficients.0)),
                Value::Int(i64::from(coefficients.1)),
            ]),
        ),
    ];
    if selected {
        let id = target.ok_or_else(|| {
            Error::new(
                ErrorCode::ReferenceUnknownId,
                "Choose a layer to rotate selected pixels",
            )
        })?;
        fields.push(("layer_ref", layer_ref(id)));
        fields.push(("clear", Value::str(clear.to_hex())));
    }
    Ok(vec![Value::Object(
        fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
    )])
}
