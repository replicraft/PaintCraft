//! Resolve fonts before publication. Transactions store grayscale coverage only.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Rect, text::TextOptions};
impl Editor {
    pub(crate) fn with_text_renderer(mut self, renderer: crate::text::Shared) -> Self {
        self.text_renderer_connect(renderer);
        self
    }
    pub(crate) fn text_renderer_connect(&mut self, renderer: crate::text::Shared) {
        self.text_renderer = Some(renderer);
        if self.tools.text_style.is_none() {
            self.tools.text_style = Some(TextOptions::default());
        }
    }
    pub(crate) fn font_text_mask(
        &self,
        text: &str,
        style: &TextOptions,
        at: (i64, i64),
    ) -> Result<Arc<crate::text::TextMask>> {
        self.text_renderer
            .as_ref()
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "font text renderer unavailable",
                )
            })?
            .lock()
            .map_err(|_| {
                Error::new(
                    ErrorCode::InvariantViolation,
                    "font text renderer unavailable",
                )
            })?
            .rasterize(text, style, at, self.document().canvas_rect())
    }
    pub(crate) fn cached_font_text_mask(
        &self,
        text: &str,
        style: &TextOptions,
        at: (i64, i64),
    ) -> Option<Arc<crate::text::TextMask>> {
        self.text_renderer.as_ref()?.try_lock().ok()?.cached(
            text,
            style,
            at,
            self.document().canvas_rect(),
        )
    }
    pub(super) fn font_text_operations(
        &self,
        layer: Uuid,
        at: (i64, i64),
        text: &str,
        style: &TextOptions,
        color: Rgba,
        background: Rgba,
    ) -> Result<Vec<Value>> {
        let mask = self.font_text_mask(text, style, at)?;
        text_operations(
            &mask,
            self.document().canvas_rect(),
            layer,
            style,
            color,
            background,
            &|| Ok(()),
        )
    }
    pub(super) fn draw_font_text(
        &mut self,
        layer: Uuid,
        at: (i64, i64),
        text: &str,
        style: &TextOptions,
        color: Rgba,
        background: Rgba,
    ) {
        if self.editing.is_some() {
            self.start_tool_plan(
                Preview::Text {
                    layer,
                    style: Some(style.clone()),
                    background,
                    at,
                    text: text.into(),
                    scale: 1,
                    color,
                },
                "Text",
            );
            return;
        }
        match self.font_text_operations(layer, at, text, style, color, background) {
            Ok(ops) if !ops.is_empty() => self.draw(ops, "Text"),
            Ok(_) => {}
            Err(error) => self.status = format!("Cannot place text: {error}"),
        }
    }
}
pub(super) fn text_operations(
    mask: &crate::text::TextMask,
    canvas: Rect,
    layer: Uuid,
    style: &TextOptions,
    color: Rgba,
    background: Rgba,
    check: &impl Fn() -> Result<()>,
) -> Result<Vec<Value>> {
    let mut ops = Vec::new();
    if style.background_fill
        && let Some(rect) = mask.bounds.and_then(|rect| rect.intersect(&canvas))
    {
        ops.push(Value::object([
            ("op", Value::str("raster.fill_rect")),
            ("layer_ref", layer_ref(layer)),
            ("rect", rect_value(rect)),
            ("color", Value::str(background.to_hex())),
            ("mode", Value::str("source-over")),
        ]));
    }
    check()?;
    if !mask.spans.is_empty() {
        ops.push(Value::object([
            ("op", Value::str("raster.mask_spans")),
            ("layer_ref", layer_ref(layer)),
            ("color", Value::str(color.to_hex())),
            (
                "spans",
                Value::Array(
                    mask.spans
                        .iter()
                        .enumerate()
                        .map(|(index, s)| {
                            if index % 256 == 0 {
                                check()?;
                            }
                            Ok(Value::Array(vec![
                                Value::Int(s.x0),
                                Value::Int(s.y),
                                Value::Int(s.x1),
                                Value::Int(i64::from(s.coverage)),
                            ]))
                        })
                        .collect::<Result<Vec<_>>>()?,
                ),
            ),
        ]));
    }
    Ok(ops)
}

fn rect_value(r: Rect) -> Value {
    Value::object([
        ("x", Value::Int(r.x0)),
        ("y", Value::Int(r.y0)),
        ("width", Value::Int(r.x1 - r.x0)),
        ("height", Value::Int(r.y1 - r.y0)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::{Action, Tool};
    #[test]
    fn font_text_preview_commit_undo_and_saved_pixels_match_with_selection_and_alpha() {
        let renderer = Arc::new(std::sync::Mutex::new(crate::text::TextRenderer::bundled()));
        let mut editor = Editor::new_document(128, 80)
            .unwrap()
            .with_text_renderer(renderer);
        editor.request(1, Action::AddLayer);
        editor.run_pending();
        editor.request(
            2,
            Action::Select {
                corners: Some(((12, 9), (75, 60))),
            },
        );
        editor.run_pending();
        let original = editor.document_snapshot();
        let layer = editor.target().unwrap();
        editor.color = Rgba([30, 90, 180, 128]);
        editor.select_tool(Tool::Text);
        editor.set_text_style(TextOptions {
            points: 24,
            bold: true,
            ..Default::default()
        });
        editor.pointer_down((8, 8));
        editor.type_text("Hello Ω");
        let batch = editor.preview_batch().unwrap().unwrap();
        assert!(
            paintcraft_editor::execute::execute_with_budget(
                editor.document(),
                editor.selection(),
                &batch,
                0
            )
            .is_err()
        );
        assert!(editor.document().same_semantics(&original));
        let preview =
            paintcraft_editor::execute::execute(editor.document(), editor.selection(), &batch)
                .unwrap()
                .doc;
        editor.commit_text();
        editor.run_pending();
        assert!(editor.document().same_semantics(&preview));
        let raster = &editor.document().layers[&layer].raster;
        let mut partial = false;
        for y in 0..80 {
            for x in 0..128 {
                let a = raster.get(x, y).a();
                assert!(a <= 128);
                if !(12..=75).contains(&x) || !(9..=60).contains(&y) {
                    assert_eq!(a, 0);
                }
                partial |= a > 0 && a < 128;
            }
        }
        assert!(partial);
        let bytes = paintcraft_formats::pcnd::write(editor.document()).unwrap();
        assert!(
            paintcraft_formats::pcnd::read(&bytes)
                .unwrap()
                .same_semantics(editor.document())
        );
        editor.request(3, Action::Undo);
        editor.run_pending();
        assert!(editor.document().same_semantics(&original));
        editor.request(4, Action::Redo);
        editor.run_pending();
        assert!(editor.document().same_semantics(&preview));
    }
}
