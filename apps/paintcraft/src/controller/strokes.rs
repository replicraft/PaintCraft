//! Brush command construction shared by preview and publication.

use super::*;
use paintcraft_tools::{BlendMode, Stroke};

#[cfg(test)]
#[path = "strokes/tests.rs"]
mod tests;

impl Editor {
    /// Staged gesture commands; applying them never publishes or adds history.
    pub fn preview_batch(&self) -> Option<Result<Batch>> {
        match self.prepare_preview(self.preview()).build(|| Ok(())) {
            Ok(Some(batch)) => Some(Ok(batch)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        }
    }

    pub(crate) fn prepare_preview(&self, preview: Option<Preview>) -> PreviewPreparation {
        self.capture_preview(preview.map(Arc::new))
    }

    pub(crate) fn capture_preview(&self, preview: Option<Arc<Preview>>) -> PreviewPreparation {
        PreviewPreparation {
            preview,
            ticket: self.session.ticket(),
            canvas: self.document().canvas_rect(),
            fonts: self.text_renderer.clone(),
        }
    }

    pub(super) fn stroke(&mut self, stroke: &Stroke) {
        let operations = match stroke_operations(stroke, &|| Ok(())) {
            Ok(operations) => operations,
            Err(error) => {
                self.status = format!("Stroke not prepared: {error}");
                return;
            }
        };
        self.draw(
            operations,
            if stroke.mode == BlendMode::Replace {
                "Erase"
            } else {
                "Stroke"
            },
        );
    }
}

fn stroke_operations(stroke: &Stroke, check: &impl Fn() -> Result<()>) -> Result<Vec<Value>> {
    check()?;
    let Stroke {
        layer,
        points,
        color,
        size,
        round,
        point_scale,
        mode,
    } = stroke;
    if points.is_empty() {
        return Ok(Vec::new());
    }
    // Long gestures are split across operations; each starts on the previous
    // chunk's last point so the path stays connected. A translucent colour
    // blends the joint's stamp twice, once per operation.
    let mut ops = Vec::new();
    let mut start = 0;
    while start < points.len() && ops.len() < OPS_PER_BATCH {
        check()?;
        let end = (start + POINTS_PER_OP).min(points.len());
        let mut values = Vec::new();
        values.try_reserve_exact(end - start).map_err(|_| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "cannot prepare stroke point values",
            )
        })?;
        for (index, p) in points[start..end].iter().enumerate() {
            if index % 256 == 0 {
                check()?;
            }
            values.push(point(*p));
        }
        ops.push(Value::object([
            ("op", Value::str("raster.stroke_path")),
            ("layer_ref", layer_ref(*layer)),
            ("points", Value::Array(values)),
            ("color", Value::str(color.to_hex())),
            (
                "mode",
                Value::str(match mode {
                    BlendMode::Replace => "replace",
                    BlendMode::SourceOver => "source-over",
                }),
            ),
            ("size", Value::Int(i64::from(*size))),
            ("point_scale", Value::Int(i64::from(*point_scale))),
            (
                "brush",
                Value::str(if *round { "smooth-round" } else { "square" }),
            ),
        ]));
        if end == points.len() {
            break;
        }
        start = end - 1;
    }
    Ok(ops)
}

/// Owned render input. Capturing it does not parse commands or rasterize fonts.
pub(crate) struct PreviewPreparation {
    preview: Option<Arc<Preview>>,
    ticket: paintcraft_editor::session::Ticket,
    canvas: paintcraft_core::types::Rect,
    fonts: Option<crate::text::Shared>,
}
impl PreviewPreparation {
    pub(crate) fn possible_preview(&self) -> bool {
        self.preview.is_some()
    }
    pub(crate) fn build(self, check: impl Fn() -> Result<()>) -> Result<Option<Batch>> {
        check()?;
        let operations = match self.preview.as_deref() {
            Some(Preview::Path(stroke)) => stroke_operations(stroke, &check)?,
            Some(Preview::Paths(strokes)) => {
                let mut operations = Vec::new();
                for stroke in strokes {
                    check()?;
                    operations.extend(stroke_operations(stroke, &check)?);
                }
                operations
            }
            Some(Preview::Text {
                layer,
                style: Some(style),
                at,
                text,
                color,
                background,
                ..
            }) if !text.is_empty() => {
                use paintcraft_core::types::{Error, ErrorCode};
                let fonts = self.fonts.ok_or_else(|| {
                    Error::new(
                        ErrorCode::CapabilityUnsupported,
                        "font text renderer unavailable",
                    )
                })?;
                let mask = {
                    let mut fonts = fonts.lock().map_err(|_| {
                        Error::new(
                            ErrorCode::InvariantViolation,
                            "font text renderer unavailable",
                        )
                    })?;
                    check()?;
                    fonts.rasterize(text, style, *at, self.canvas)?
                };
                check()?;
                let operations = super::text::text_operations(
                    &mask,
                    self.canvas,
                    *layer,
                    style,
                    *color,
                    *background,
                    &check,
                )?;
                if operations.is_empty() {
                    return Ok(None);
                }
                operations
            }
            _ => return Ok(None),
        };
        check()?;
        let batch = Batch::from_value(&Value::object([
            ("schema", Value::str("paintcraft.batch/v2")),
            ("atomic", Value::Bool(true)),
            ("policy", Value::str(POLICY)),
            (
                "base",
                Value::object([
                    ("state_id", Value::str(self.ticket.state_id.to_string())),
                    (
                        "publication_epoch",
                        Value::str(self.ticket.publication_epoch.to_string()),
                    ),
                ]),
            ),
            ("operations", Value::Array(operations)),
        ]))?;
        check()?;
        Ok(Some(batch))
    }
}

#[cfg(test)]
#[path = "strokes/preparation_tests.rs"]
mod preparation_tests;
