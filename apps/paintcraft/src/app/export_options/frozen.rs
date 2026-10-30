//! Live intent authority is separate from the private text-export document.
use super::*;
#[derive(Clone)]
pub(crate) struct Frozen {
    pub document: std::sync::Arc<paintcraft_document::Document>,
    pub filter_bake: export::filter_bake::Policy,
    pub has_filters: bool,
    owner: Uuid,
    source: controller::PreviewStamp,
    frontend: paintcraft_formats::registry::Frontend,
}
impl std::fmt::Debug for Frozen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrozenExport")
            .field("document", &self.document.document_id)
            .field("state", &self.document.state_id)
            .field("owner", &self.owner)
            .finish()
    }
}
impl PartialEq for Frozen {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.document, &other.document)
            && self.owner == other.owner
            && self.source == other.source
            && self.frontend == other.frontend
            && self.filter_bake == other.filter_bake
    }
}
impl Frozen {
    pub(crate) fn capture(app: &App) -> paintcraft_core::types::Result<Self> {
        Ok(Self {
            document: app.editor.export_snapshot()?,
            filter_bake: Default::default(),
            has_filters: app
                .editor
                .document()
                .layers
                .values()
                .any(|l| !l.filters.is_empty()),
            owner: app.active_tab_id(),
            source: app.editor.preview_stamp(),
            frontend: app.format_frontend(),
        })
    }
    pub(crate) fn current(&self, app: &App) -> bool {
        self.owner == app.active_tab_id()
            && self.source == app.editor.preview_stamp()
            && self.frontend == app.format_frontend()
            && app.editor.can_capture_export()
    }
}
