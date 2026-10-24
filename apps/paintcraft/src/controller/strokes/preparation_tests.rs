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

#[test]
fn shape_previews_use_captured_layers_and_the_same_selected_pixels_as_release() {
    for tool in [Tool::Line, Tool::Rect, Tool::Ellipse] {
        for style in [ShapeStyle::Outline, ShapeStyle::Filled, ShapeStyle::Both] {
            let mut editor = Editor::new_document(32, 24).unwrap();
            editor.request(
                1,
                Action::Select {
                    corners: Some(((3, 4), (15, 13))),
                },
            );
            editor.run_pending();
            editor.tool = tool;
            editor.options.style = style;
            editor.options.size = 3;
            editor.color = Rgba([190, 20, 30, 128]);
            editor.secondary = Rgba([20, 30, 190, 96]);
            editor.pointer_down((-2, -1));
            editor.pointer_move((14, 12));
            let before = editor.document_snapshot();
            let history = editor.history_view();
            let layer = editor.target().unwrap();
            let captured = editor.prepare_preview(editor.preview());
            let batch = captured.build(|| Ok(())).unwrap().unwrap();
            assert!(
                matches!(&batch.operations[0], paintcraft_commands::Op::Shape {
                layer: paintcraft_commands::LayerRef::Id(id), ..
            } if *id == layer)
            );
            let staged = paintcraft_editor::execute::execute(&before, editor.selection(), &batch)
                .unwrap()
                .doc;
            assert_eq!(editor.history_view(), history);
            assert!(Arc::ptr_eq(&before, &editor.document_snapshot()));
            assert_eq!(staged.layers[&layer].raster.get(2, 4), Rgba([255; 4]));
            assert_eq!(staged.layers[&layer].raster.get(16, 13), Rgba([255; 4]));
            editor.pointer_up();
            editor.run_pending();
            assert_eq!(
                paintcraft_render::render_flat(&staged).unwrap(),
                paintcraft_render::render_flat(editor.document()).unwrap(),
                "{tool:?} {style:?}"
            );
            assert_eq!(
                editor.publications(),
                history.ticket.base.publication_epoch + 1
            );
        }
    }
}

#[test]
fn captured_shape_does_not_retarget_when_the_editor_target_changes() {
    let mut editor = Editor::new_document(24, 24).unwrap();
    let first = editor.target().unwrap();
    editor.request(1, Action::AddLayer);
    editor.run_pending();
    let second = editor.target().unwrap();
    editor.tool = Tool::Rect;
    editor.pointer_down((2, 2));
    editor.pointer_move((12, 12));
    let captured = editor.prepare_preview(editor.preview());
    editor.target = Some(first);
    let batch = captured.build(|| Ok(())).unwrap().unwrap();
    assert!(
        matches!(&batch.operations[0], paintcraft_commands::Op::Shape {
        layer: paintcraft_commands::LayerRef::Id(id), ..
    } if *id == second)
    );
}

#[test]
fn bitmap_text_preview_and_commit_share_raster_commands_without_publication() {
    let mut editor = Editor::new_document(32, 24).unwrap();
    editor.tool = Tool::Text;
    editor.pointer_down((3, 4));
    editor.type_text("A");
    let before = editor.document_snapshot();
    let history = editor.history_view();
    let batch = editor
        .prepare_preview(editor.preview())
        .build(|| Ok(()))
        .unwrap()
        .unwrap();
    let staged = paintcraft_editor::execute::execute(&before, editor.selection(), &batch)
        .unwrap()
        .doc;
    let preview = paintcraft_render::render_flat(&staged).unwrap();
    assert!(preview.as_chunks::<4>().0.contains(&[0, 0, 0, 255]));
    assert_eq!(editor.history_view(), history);
    assert!(Arc::ptr_eq(&before, &editor.document_snapshot()));
    editor.commit_text();
    editor.run_pending();
    assert_eq!(
        preview,
        paintcraft_render::render_flat(editor.document()).unwrap()
    );
}
