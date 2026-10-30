//! Resolve fonts before publication. Transactions store grayscale coverage only.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, text::TextOptions};
impl Editor {
    pub(crate) fn canvas_text_target(&self) -> Option<(u64, Uuid)> {
        self.tools.text_target()
    }
    pub(crate) fn canvas_text_content(&self) -> Option<&str> {
        self.tools.text_content()
    }
    pub(crate) fn canvas_text_revision(&self) -> paintcraft_tools::PreviewRevision {
        self.tools.preview_revision()
    }
    pub(crate) fn replace_canvas_text_tail(
        &mut self,
        token: u64,
        delete: usize,
        text: &str,
    ) -> Result<()> {
        self.tools.replace_text_tail(token, delete, text)?;
        self.reconcile_text_submission();
        Ok(())
    }
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
            Ok(_) => self.finish_text_attempt(self.text_submission.executing, true),
            Err(error) => {
                self.fail_text_attempt(self.text_submission.executing);
                self.status = format!("Cannot place text: {error}");
            }
        }
    }
}
pub(super) use paintcraft_tools::commands::font_text as text_operations;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::{Action, Tool};
    #[test]
    fn font_text_preview_commit_undo_and_saved_pixels_match_with_selection_and_alpha() {
        let renderer = Arc::new(std::sync::Mutex::new(crate::text::bundled()));
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
