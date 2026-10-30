//! Freeze the actual copy once, including private text pixels, before consent.
use super::*;
#[derive(Clone, Debug)]
pub(crate) struct Frozen {
    pub document: std::sync::Arc<paintcraft_document::Document>,
    pub has_filters: bool,
    base: std::sync::Arc<paintcraft_document::Document>,
    ticket: paintcraft_editor::session::HistoryTicket,
    preview: paintcraft_tools::PreviewRevision,
    frontend: Frontend,
    variant: ClassicVariant,
}
impl Frozen {
    pub(crate) fn capture(app: &mut WebApp) -> paintcraft_core::types::Result<Self> {
        app.sync_text();
        if app.text.is_composing() {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::InputSchema,
                "Finish text composition before downloading",
            ));
        }
        Ok(Self {
            document: app.editor.copy_snapshot()?,
            has_filters: app
                .editor
                .session
                .document()
                .layers
                .values()
                .any(|l| !l.filters.is_empty()),
            base: app.editor.session.snapshot(),
            ticket: app.editor.session.history_ticket(),
            preview: app.editor.tools.preview_revision(),
            frontend: app.frontend,
            variant: app.variant,
        })
    }
    pub(crate) fn current(&self, app: &WebApp) -> bool {
        std::sync::Arc::ptr_eq(&self.base, &app.editor.session.snapshot())
            && self.ticket == app.editor.session.history_ticket()
            && self.preview == app.editor.tools.preview_revision()
            && self.frontend == app.frontend
            && self.variant == app.variant
            && !app.text.is_composing()
    }
}
