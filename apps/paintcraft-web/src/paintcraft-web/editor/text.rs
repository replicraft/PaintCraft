//! Draft text is privately rasterized; only explicit placement publishes pixels.
use super::*;
use paintcraft_core::types::text::{TextMask, TextOptions};
use paintcraft_tools::text::{Database, FontSystem, TextRenderer};
use std::{cell::RefCell, sync::Arc};

pub(super) fn fonts() -> RefCell<TextRenderer> {
    let mut db = Database::new();
    for bytes in paintcraft_ui::fonts::bundled_data() {
        db.load_font_data(bytes.to_vec());
    }
    db.set_sans_serif_family("Selawik");
    RefCell::new(TextRenderer::new(FontSystem::new_with_locale_and_db(
        "en-US".into(),
        db,
    )))
}

impl Editor {
    pub fn has_text(&self) -> bool {
        self.tools
            .text_content()
            .is_some_and(|text| !text.is_empty())
    }
    pub fn is_dirty(&self) -> bool {
        self.session.is_dirty() || self.has_text() || self.tools.floating_selection().is_some()
    }

    pub fn text_mask(
        &self,
        text: &str,
        style: &TextOptions,
        at: (i64, i64),
    ) -> Result<Arc<TextMask>> {
        let mut renderer = self
            .fonts
            .try_borrow_mut()
            .map_err(|_| Error::new(ErrorCode::InvariantViolation, "Text renderer is busy"))?;
        crate::local_fonts::prepare(&mut renderer, &style.family)?;
        renderer.rasterize(text, style, at, self.session.document().canvas_rect())
    }

    pub(super) fn text_operations(&self, action: &Action) -> Result<Vec<Value>> {
        match action {
            Action::Text {
                layer,
                style: Some(style),
                at,
                text,
                color,
                background,
                ..
            } => {
                let mask = self.text_mask(text, style, *at)?;
                commands::font_text(
                    &mask,
                    self.session.document().canvas_rect(),
                    *layer,
                    style,
                    *color,
                    *background,
                    &|| Ok(()),
                )
            }
            Action::Text {
                layer,
                style: None,
                at,
                text,
                scale,
                color,
                ..
            } => Ok(vec![commands::bitmap_text(
                *layer, *at, text, *scale, *color,
            )]),
            _ => Err(Error::new(
                ErrorCode::InputSchema,
                "Expected a text intention",
            )),
        }
    }

    /// A failed preparation/publication retains the draft so it can be corrected.
    pub fn place_text(&mut self) -> Result<()> {
        if let Some(action) = self.tools.pending_text_action() {
            let operations = self.text_operations(&action)?;
            if !operations.is_empty() {
                self.commit(operations)?;
            }
        }
        self.tools.cancel_text();
        Ok(())
    }

    /// An immutable export revision has the same state ID/pixels as placement,
    /// while the editable draft, owning session and history remain unchanged.
    pub fn copy_snapshot(&self) -> Result<Arc<Document>> {
        if self.tools.floating_selection().is_some() {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Finish moving the selection before opening, saving or exporting a file",
            ));
        }
        let Some(action) = self.tools.pending_text_action() else {
            return Ok(self.session.snapshot());
        };
        let operations = self.text_operations(&action)?;
        if operations.is_empty() {
            return Ok(self.session.snapshot());
        }
        let prepared = self
            .session
            .preparation()?
            .apply(&self.batch(operations)?, || Ok(()))?;
        Ok(prepared.document_snapshot())
    }
}
