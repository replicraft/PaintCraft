use super::*;
use paintcraft_formats::jxl::{self, Samples};
use std::sync::Arc;

#[test]
fn studio_exports_a_single_pixel_jxl_without_resizing_or_changing_the_document() {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    let pixel = [17, 29, 91, 255];
    app.editor = Editor::with_document(import_image(1, 1, &pixel, &"0".repeat(64)).unwrap(), None);
    let before = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    let dirty = app.editor.is_dirty();
    let path = std::env::temp_dir().join(format!("paintcraft-jxl-single-{}.jxl", Uuid::new_v4()));
    app.export_selected_quality(path.clone(), 90);
    let Some(Confirm::Export { format, losses, .. }) = app.confirm.take() else {
        panic!("single-pixel export should reach the existing confirmation");
    };
    assert_eq!(format, ImageFormat::Jxl);
    app.do_export_quality(&path, format, &losses, 90);
    let decoded = jxl::decode_with_pixel_limit(&std::fs::read(&path).unwrap(), 1).unwrap();
    assert_eq!((decoded.width, decoded.height), (1, 1));
    assert_eq!(decoded.samples, Samples::Rgba8(pixel.to_vec()));
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(ticket, app.editor.history_view().ticket);
    assert_eq!(dirty, app.editor.is_dirty());
    std::fs::remove_file(path).unwrap();
}
