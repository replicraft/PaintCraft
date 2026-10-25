//! Brush command construction shared by preview and publication.

use super::*;
use paintcraft_tools::{BlendMode, Stroke, commands::stroke_operations};

#[cfg(test)]
#[path = "strokes/tests.rs"]
mod tests;

impl Editor {
    pub(super) fn contour(&mut self, contour: &paintcraft_tools::Contour) {
        if self.editing.is_some() {
            self.start_tool_plan(Preview::Contour(contour.clone()), "Shape");
        } else {
            self.draw(vec![paintcraft_tools::commands::contour(contour)], "Shape");
        }
    }
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
            paste: self.capture_image_paste(),
        }
    }

    pub(super) fn stroke(&mut self, stroke: &Stroke) {
        if self.editing.is_some() {
            self.start_tool_plan(
                Preview::Path(stroke.clone()),
                if stroke.mode == BlendMode::Replace {
                    "Erase"
                } else {
                    "Stroke"
                },
            );
            return;
        }
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

/// Owned render input. Capturing it does not parse commands or rasterize fonts.
pub(crate) struct PreviewPreparation {
    preview: Option<Arc<Preview>>,
    ticket: paintcraft_editor::session::Ticket,
    canvas: paintcraft_core::types::Rect,
    fonts: Option<crate::text::Shared>,
    paste: Option<super::image_paste::Captured>,
}
impl PreviewPreparation {
    pub(crate) fn build_paste(
        &mut self,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Option<Document>> {
        self.paste
            .take()
            .map(|paste| paste.build(check))
            .transpose()
    }
    pub(crate) fn possible_preview(&self) -> bool {
        if self.paste.is_some() {
            return true;
        }
        match self.preview.as_deref() {
            Some(Preview::Marquee { .. } | Preview::SelectionPath { .. }) => false,
            Some(Preview::Text { text, .. }) if text.is_empty() => false,
            Some(_) => true,
            None => false,
        }
    }
    pub(crate) fn build(self, check: impl Fn() -> Result<()>) -> Result<Option<Batch>> {
        check()?;
        let operations = match self.preview.as_deref() {
            Some(Preview::MoveSelection {
                layer,
                offset,
                clear,
                transparent_key,
            }) => vec![paintcraft_tools::commands::move_selection(
                *layer,
                *offset,
                *clear,
                *transparent_key,
            )?],
            Some(Preview::Contour(contour)) => vec![paintcraft_tools::commands::contour(contour)],
            Some(Preview::Path(stroke)) => stroke_operations(stroke, &check)?,
            Some(Preview::Paths(strokes)) => {
                let mut operations = Vec::new();
                for stroke in strokes {
                    check()?;
                    operations.extend(stroke_operations(stroke, &check)?);
                }
                operations
            }
            Some(Preview::Shape {
                layer,
                tool,
                from,
                to,
                stroke,
                fill,
            }) => {
                vec![super::raster_commands::shape(
                    *layer, *tool, *from, *to, *stroke, *fill,
                )]
            }
            Some(Preview::Text {
                layer,
                style: None,
                at,
                text,
                scale,
                color,
                ..
            }) => {
                if text.is_empty() {
                    return Ok(None);
                }
                vec![super::raster_commands::bitmap_text(
                    *layer, *at, text, *scale, *color,
                )]
            }
            Some(Preview::Text {
                layer,
                style: Some(style),
                at,
                text,
                color,
                background,
                ..
            }) => {
                if text.is_empty() && self.fonts.is_none() {
                    return Ok(None);
                }
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

#[cfg(test)]
#[path = "strokes/mask_tests.rs"]
mod mask_tests;
