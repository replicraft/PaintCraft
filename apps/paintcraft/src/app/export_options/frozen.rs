//! Live intent authority is separate from the private text-export document.
use super::*;
#[derive(Clone)]
pub(crate) struct Frozen {
    pub document: std::sync::Arc<paintcraft_document::Document>,
    pub filter_bake: export::filter_bake::Policy,
    pub appearance_bake: export::appearance_bake::Policy,
    pub appearance_counts: [u64; 5],
    pub has_styles: bool,
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
            && self.appearance_bake == other.appearance_bake
    }
}
impl Frozen {
    pub(crate) fn capture(app: &App) -> paintcraft_core::types::Result<Self> {
        let document = app.editor.export_snapshot()?;
        let appearance_counts = if document.has_styles() {
            let i = export::appearance_bake::inspect(&document, &|| Ok(()))?;
            [
                i.style_count(),
                i.filter_count(),
                i.filter_mask_count(),
                i.layer_mask_count(),
                i.non_normal_layer_count(),
            ]
        } else {
            [0; 5]
        };
        Ok(Self {
            document,
            appearance_counts,
            filter_bake: Default::default(),
            appearance_bake: Default::default(),
            has_styles: app.editor.document().has_styles(),
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
