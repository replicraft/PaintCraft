use super::*;
use paintcraft_core::types::{Error, ErrorCode};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
#[test]
fn detached_preview_uses_its_captured_path_and_ticket_after_the_ui_changes() {
    let mut editor = Editor::new_document(32, 32).unwrap();
    editor.pointer_down((2, 3));
    editor.pointer_move((5, 3));
    let initial = editor.document_snapshot();
    let ticket = editor.session.ticket();
    let request = editor.prepare_preview(editor.preview());
    editor.pointer_move((20, 20));
    editor.cancel_gesture();
    editor.request(1, Action::AddLayer);
    editor.run_pending();
    let batch = std::thread::spawn(move || request.build(|| Ok(())).unwrap().unwrap())
        .join()
        .unwrap();
    assert_eq!(
        (batch.base_state, batch.base_epoch),
        (ticket.state_id, ticket.publication_epoch)
    );
    let doc = paintcraft_editor::execute::execute(&initial, Selection::Inactive, &batch)
        .unwrap()
        .doc;
    let layer = &doc.layers[&doc.roots[0]];
    assert_eq!(layer.raster.get(2, 3), Rgba([0, 0, 0, 255]));
    assert_eq!(layer.raster.get(5, 3), Rgba([0, 0, 0, 255]));
    assert_eq!(layer.raster.get(20, 20), Rgba([255; 4]));
    assert_eq!(
        editor.session.apply(&batch).unwrap_err().code,
        ErrorCode::ConflictStaleBase
    );
}
#[test]
fn cancellation_during_point_preparation_returns_no_partial_batch_or_publication() {
    let editor = Editor::new_document(32, 32).unwrap();
    let initial = editor.document_snapshot();
    let history = editor.history_view();
    let request = editor.prepare_preview(Some(Preview::Path(Stroke {
        layer: editor.target().unwrap(),
        points: vec![(2, 3); 2048],
        point_scale: 1,
        color: Rgba([0, 0, 0, 128]),
        size: 1,
        round: false,
        mode: BlendMode::SourceOver,
    })));
    let checks = AtomicUsize::new(0);
    let result = request.build(|| {
        if checks.fetch_add(1, Ordering::SeqCst) >= 5 {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "cancel preview preparation",
            ))
        } else {
            Ok(())
        }
    });
    assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    assert_eq!(editor.history_view(), history);
    assert!(Arc::ptr_eq(&editor.document_snapshot(), &initial));
}
#[test]
fn text_capture_does_not_rasterize_until_the_owned_request_runs_on_a_worker() {
    let fonts = Arc::new(Mutex::new(crate::text::TextRenderer::bundled()));
    let mut editor = Editor::new_document(128, 80)
        .unwrap()
        .with_text_renderer(fonts);
    editor.select_tool(Tool::Text);
    editor.pointer_down((8, 8));
    editor.type_text("Hello Ω");
    let preview = editor.preview();
    let Some(Preview::Text {
        style: Some(style), ..
    }) = &preview
    else {
        panic!("styled text");
    };
    assert!(
        editor
            .cached_font_text_mask("Hello Ω", style, (8, 8))
            .is_none()
    );
    let request = editor.prepare_preview(preview.clone());
    assert!(
        editor
            .cached_font_text_mask("Hello Ω", style, (8, 8))
            .is_none()
    );
    let batch = std::thread::spawn(move || request.build(|| Ok(())).unwrap().unwrap())
        .join()
        .unwrap();
    assert!(
        editor
            .cached_font_text_mask("Hello Ω", style, (8, 8))
            .is_some()
    );
    let staged =
        paintcraft_editor::execute::execute(editor.document(), editor.selection(), &batch).unwrap();
    assert!(
        paintcraft_render::render_flat(&staged.doc)
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| *pixel != [255; 4])
    );
    assert_eq!(editor.publications(), 0);
}

#[test]
fn an_empty_text_preview_prepares_caret_geometry_without_creating_an_edit_batch() {
    let fonts = Arc::new(Mutex::new(crate::text::TextRenderer::bundled()));
    let mut editor = Editor::new_document(128, 80)
        .unwrap()
        .with_text_renderer(fonts);
    editor.select_tool(Tool::Text);
    editor.pointer_down((8, 8));
    let Some(Preview::Text {
        style: Some(style), ..
    }) = editor.preview()
    else {
        panic!("styled text");
    };
    assert!(editor.cached_font_text_mask("", &style, (8, 8)).is_none());
    let request = editor.prepare_preview(editor.preview());
    assert!(
        std::thread::spawn(move || request.build(|| Ok(())).unwrap())
            .join()
            .unwrap()
            .is_none()
    );
    assert!(
        editor
            .cached_font_text_mask("", &style, (8, 8))
            .unwrap()
            .caret
            .2
            > 0
    );
    assert_eq!(editor.publications(), 0);
}
