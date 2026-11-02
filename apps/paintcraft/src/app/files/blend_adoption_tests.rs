//! Actual acquired PCND8 and typed frontend install, with old owner preserved.
use super::*;
use paintcraft_core::types::{ErrorCode, LayerBlendMode, Rgba};
use paintcraft_document::{Document, POLICY};
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("paintcraft-blend-open-{}", Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn source(mode: LayerBlendMode) -> Document {
    let mut doc =
        paintcraft_document::new_document(2, 1, Some(Rgba([17, 32, 255, 0])), 0, POLICY).unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.blend_mode = mode;
    layer.visible = false;
    layer.opacity = 0;
    doc
}
#[test]
fn pcnd8_open_and_typed_install_refuse_before_replacing_drafts_history_or_source() {
    let folder = Folder::new();
    for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        let doc = source(mode);
        let bytes = paintcraft_formats::pcnd::write(&doc).unwrap();
        assert_eq!(bytes[4], 8);
        let path = folder.0.join(format!("{mode:?}.pcnd"));
        std::fs::write(&path, &bytes).unwrap();
        let mut app = App::new(None);
        app.editor.tool = Tool::Text;
        app.editor.pointer_down((3, 2));
        app.editor.type_text("keep private draft");
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let stamp = app.editor.preview_stamp();
        let draft = app.editor.canvas_text_content().unwrap().to_owned();
        let bound = app.bound.clone();
        let tabs = app.tabs.len();
        // Generic host reader still adopts editable metadata; only UI adoption closes.
        let decoded = app.read_project_with_metadata(&bytes).unwrap();
        assert!(decoded.same_semantics(&doc));
        match Editor::open(&path) {
            Err(error) => assert_eq!(error.code, ErrorCode::CapabilityUnsupported),
            Ok(_) => panic!("standalone frontend must refuse stored blend intent"),
        }
        assert_eq!(
            app.open_path(&path).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            app.install(Editor::with_document(decoded, None))
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        assert!(std::sync::Arc::ptr_eq(
            &before,
            &app.editor.document_snapshot()
        ));
        assert_eq!(app.editor.history_view().ticket, ticket);
        assert_eq!(app.editor.preview_stamp(), stamp);
        assert_eq!(app.editor.canvas_text_content(), Some(draft.as_str()));
        assert_eq!(app.bound, bound);
        assert_eq!(app.tabs.len(), tabs);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    let normal = source(LayerBlendMode::Normal);
    let path = folder.0.join("normal.pcnd");
    std::fs::write(&path, paintcraft_formats::pcnd::write(&normal).unwrap()).unwrap();
    assert!(
        Editor::open(&path)
            .unwrap()
            .document()
            .same_semantics(&normal)
    );
}
