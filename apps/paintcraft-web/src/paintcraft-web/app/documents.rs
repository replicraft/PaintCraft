//! Document labels follow successful opens and requested project downloads.
use super::*;
impl WebApp {
    pub(crate) fn document_title(&self) -> String {
        format!("{} - PaintCraft", self.name)
    }
    pub(crate) fn download_name(&self, format: Option<ImageExport>) -> String {
        let stem = self
            .name
            .rsplit_once('.')
            .filter(|(stem, _)| !stem.is_empty())
            .map_or(self.name.as_str(), |(stem, _)| stem);
        let extension = format.map_or("pcnd", ImageExport::extension);
        format!("{stem}.{extension}")
    }
    pub(super) fn accept_download_name(&mut self, owner: uuid::Uuid, name: Option<String>) {
        if owner == self.editor.session.history_ticket().owner
            && let Some(name) = name
        {
            self.name = name;
            self.update_title();
        }
    }
    pub(super) fn update_title(&self) {
        let title = self.document_title();
        #[cfg(target_arch = "wasm32")]
        if let Some(document) = web_sys::window().and_then(|window| window.document()) {
            document.set_title(&title);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = title;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn opens_project_downloads_exports_and_new_keep_the_document_label_consistent() {
        let ctx = egui::Context::default();
        let mut app = WebApp::new(&ctx);
        assert_eq!(app.document_title(), "Untitled - PaintCraft");
        app.install(crate::files::Loaded {
            document: paintcraft_document::new_document(
                12,
                8,
                Some(WHITE),
                0,
                paintcraft_document::POLICY,
            )
            .unwrap(),
            name: "旅行.photo.png".into(),
            losses: vec![],
        });
        assert_eq!(app.document_title(), "旅行.photo.png - PaintCraft");
        assert_eq!(app.download_name(None), "旅行.photo.pcnd");
        assert_eq!(app.download_name(Some(ImageExport::Jpeg)), "旅行.photo.jpg");
        let source = app.editor.session.history_ticket().owner;
        let original = app.editor.session.snapshot();
        app.accept_download_name(source, None);
        assert_eq!(app.document_title(), "旅行.photo.png - PaintCraft");
        app.accept_download_name(source, Some(app.download_name(None)));
        assert_eq!(app.document_title(), "旅行.photo.pcnd - PaintCraft");
        assert_eq!(app.download_name(None), "旅行.photo.pcnd");
        assert!(Arc::ptr_eq(&original, &app.editor.session.snapshot()));
        app.install(crate::files::Loaded {
            document: (*original).clone(),
            name: "reopened.png".into(),
            losses: vec![],
        });
        assert_ne!(source, app.editor.session.history_ticket().owner);
        app.accept_download_name(source, Some("stale.pcnd".into()));
        assert_eq!(app.document_title(), "reopened.png - PaintCraft");
        app.new_document();
        app.accept_download_name(source, Some("stale.pcnd".into()));
        assert_eq!(app.document_title(), "Untitled - PaintCraft");
    }
}
