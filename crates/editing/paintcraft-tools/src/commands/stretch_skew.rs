//! Ordinary strict JSON commands; no authority or direct document mutation.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Rect, stretch_skew::StretchSkew};
use paintcraft_raster::Selection;
pub fn stretch_skew_source(
    document: &paintcraft_document::Document,
    selection: &Selection,
) -> Result<Option<Rect>> {
    Ok(match selection {
        Selection::Inactive => Some(document.canvas_rect()),
        Selection::Empty => None,
        Selection::Rect(r) => r.intersect(&document.canvas_rect()),
        Selection::Mask(mask) => mask.bounds_checked(
            document.canvas_rect(),
            paintcraft_raster::selection::MaskLimits::default().work,
            &|| Ok(()),
        )?,
    })
}
pub fn stretch_skew_image(
    document: &paintcraft_document::Document,
    selection: Selection,
    target: Option<Uuid>,
    size: (u32, u32),
    shear: (i32, i32),
    clear: Rgba,
) -> Result<Vec<Value>> {
    let Some(area) = stretch_skew_source(document, &selection)? else {
        return Ok(Vec::new());
    };
    let g = StretchSkew::new(area, size, shear)?;
    if g.is_identity() {
        return Ok(Vec::new());
    }
    let mut fields = vec![
        (
            "op",
            Value::str(if selection == Selection::Inactive {
                "canvas.stretch_skew"
            } else {
                "raster.stretch_skew_selection"
            }),
        ),
        ("width", Value::Int(i64::from(size.0))),
        ("height", Value::Int(i64::from(size.1))),
        (
            "shear",
            Value::Array(vec![
                Value::Int(i64::from(shear.0)),
                Value::Int(i64::from(shear.1)),
            ]),
        ),
    ];
    if selection != Selection::Inactive {
        let id = target.ok_or_else(|| {
            Error::new(
                ErrorCode::ReferenceUnknownId,
                "Choose a layer to transform selected pixels",
            )
        })?;
        fields.push(("layer_ref", layer_ref(id)));
        fields.push(("clear", Value::str(clear.to_hex())));
    }
    Ok(vec![Value::Object(
        fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
    )])
}
