use super::*;

#[test]
fn brush_preview_is_the_published_pixels_with_one_undo_step() {
    let mut editor = Editor::new_document(32, 32).unwrap();
    editor.request(1, Action::AddLayer);
    editor.run_pending();
    let layer = editor.target().unwrap();
    editor.request(
        2,
        Action::SetOpacity {
            layer,
            opacity: 128,
        },
    );
    editor.run_pending();
    editor.select_tool(Tool::Brush);
    editor.options.size = 5;
    editor.color = Rgba([32, 64, 128, 128]);
    let base = editor.document_snapshot();
    let depth = editor.undo_depth();
    editor.pointer_down((2, 3));
    editor.pointer_move((18, 14));
    editor.pointer_move((2, 3));
    let batch = editor.preview_batch().unwrap().unwrap();
    let preview =
        paintcraft_editor::execute::execute(editor.document(), editor.selection(), &batch)
            .unwrap()
            .doc;
    assert!(editor.document().same_semantics(&base));
    let pixels = paintcraft_render::render_flat(&preview).unwrap();
    assert!((0..32).any(|y| (0..32).any(|x| {
        let a = preview.layers[&layer].raster.get(x, y).a();
        a > 0 && a < 128
    })));
    editor.pointer_up();
    editor.run_pending();
    assert_eq!(
        pixels,
        paintcraft_render::render_flat(editor.document()).unwrap()
    );
    assert_eq!(editor.undo_depth(), depth + 1);
    editor.request(3, Action::Undo);
    editor.run_pending();
    assert!(editor.document().same_semantics(&base));
}

#[test]
fn cancelled_preview_never_publishes() {
    let mut editor = Editor::new_document(16, 16).unwrap();
    editor.request(1, Action::AddLayer);
    editor.run_pending();
    editor.select_tool(Tool::Brush);
    let before = editor.document_snapshot();
    editor.pointer_down((3, 3));
    assert!(editor.preview_batch().is_some());
    editor.cancel_gesture();
    assert!(editor.preview_batch().is_none());
    assert!(editor.document().same_semantics(&before));
}

#[test]
fn zero_opacity_and_transparent_paint_do_not_erase_or_create_history() {
    for tool in [Tool::Brush, Tool::Pencil] {
        let mut editor = Editor::new_document(16, 16).unwrap();
        editor.select_tool(tool);
        editor.options.brush_opacity = 0;
        if tool == Tool::Pencil {
            editor.color = Rgba([12, 34, 56, 0]);
        }
        let before = editor.document_snapshot();
        let key = editor.render_key();
        let depth = editor.undo_depth();
        editor.pointer_down((4, 4));
        editor.pointer_move((10, 10));
        let batch = editor.preview_batch().unwrap().unwrap();
        let preview =
            paintcraft_editor::execute::execute(editor.document(), editor.selection(), &batch)
                .unwrap()
                .doc;
        assert!(preview.same_semantics(&before));
        editor.pointer_up();
        editor.run_pending();
        assert!(editor.document().same_semantics(&before));
        assert_eq!(editor.render_key(), key);
        assert_eq!(editor.undo_depth(), depth);
        assert!(!editor.is_dirty());
    }
}

#[test]
fn repeated_segments_cap_opacity_and_cancellation_keeps_the_original() {
    let mut editor = Editor::new_document(32, 32).unwrap();
    editor.request(1, Action::AddLayer);
    editor.run_pending();
    editor.select_tool(Tool::Brush);
    editor.options.size = 5;
    editor.options.brush_opacity = 50;
    editor.color = Rgba([12, 34, 56, 128]);
    let layer = editor.target().unwrap();
    let before = editor.document_snapshot();
    editor.pointer_down((5, 5));
    editor.pointer_move((25, 25));
    editor.pointer_move((5, 5));
    let staged = paintcraft_editor::execute::execute(
        editor.document(),
        editor.selection(),
        &editor.preview_batch().unwrap().unwrap(),
    )
    .unwrap()
    .doc;
    assert_eq!(
        staged.layers[&layer].raster.get(10, 10),
        Rgba([12, 34, 56, 64])
    );
    assert!((0..32).all(|y| (0..32).all(|x| staged.layers[&layer].raster.get(x, y).a() <= 64)));
    editor.cancel_gesture();
    assert!(editor.document().same_semantics(&before));
}

#[test]
#[ignore = "manual CPU preview profile; prints timings rather than asserting a hardware threshold"]
fn profile_round_brush_preview() {
    let mut e = Editor::new_document(1000, 486).unwrap();
    e.select_tool(Tool::Brush);
    e.options.size = 32;
    for count in [100, 1000, 4000] {
        e.cancel_gesture();
        e.pointer_down((20, 100));
        for i in 1..count {
            e.pointer_move((
                20 + (i % 900),
                100 + ((i as f64 * 0.02).sin() * 50.0) as i64,
            ));
        }
        let start = std::time::Instant::now();
        let batch = e.preview_batch().unwrap().unwrap();
        let staged =
            paintcraft_editor::execute::execute(e.document(), e.selection(), &batch).unwrap();
        let raster = start.elapsed();
        let render = std::time::Instant::now();
        let _ = paintcraft_render::render_flat(&staged.doc).unwrap();
        eprintln!(
            "{count} samples: stage={:.2}ms CPU composite={:.2}ms",
            raster.as_secs_f64() * 1000.0,
            render.elapsed().as_secs_f64() * 1000.0
        );
    }
}
