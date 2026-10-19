//! Brush command construction shared by preview and publication.

use super::*;
use paintcraft_tools::{BlendMode, Stroke};

#[cfg(test)]
#[path = "strokes/tests.rs"]
mod tests;

impl Editor {
    /// Staged gesture commands; applying them never publishes or adds history.
    pub fn preview_batch(&self) -> Option<Result<Batch>> {
        let Preview::Path(stroke) = self.preview()? else {
            return None;
        };
        Some(Batch::from_value(&Value::object([
            ("schema", Value::str("paintcraft.batch/v2")),
            ("atomic", Value::Bool(true)),
            ("policy", Value::str(POLICY)),
            (
                "base",
                Value::object([
                    (
                        "state_id",
                        Value::str(self.session.ticket().state_id.to_string()),
                    ),
                    (
                        "publication_epoch",
                        Value::str(self.session.ticket().publication_epoch.to_string()),
                    ),
                ]),
            ),
            ("operations", Value::Array(stroke_operations(&stroke))),
        ])))
    }

    pub(super) fn stroke(&mut self, stroke: &Stroke) {
        self.draw(
            stroke_operations(stroke),
            if stroke.mode == BlendMode::Replace {
                "Erase"
            } else {
                "Stroke"
            },
        );
    }
}

fn stroke_operations(stroke: &Stroke) -> Vec<Value> {
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
        return Vec::new();
    }
    // Long gestures are split across operations; each starts on the previous
    // chunk's last point so the path stays connected. A translucent colour
    // blends the joint's stamp twice, once per operation.
    let mut ops = Vec::new();
    let mut start = 0;
    while start < points.len() && ops.len() < OPS_PER_BATCH {
        let end = (start + POINTS_PER_OP).min(points.len());
        ops.push(Value::object([
            ("op", Value::str("raster.stroke_path")),
            ("layer_ref", layer_ref(*layer)),
            (
                "points",
                Value::Array(points[start..end].iter().map(|p| point(*p)).collect()),
            ),
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
    ops
}
