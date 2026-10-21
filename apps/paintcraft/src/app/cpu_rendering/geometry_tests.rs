use super::*;
use crate::{Editor, Tool};
use paintcraft_editor::execute::execute;

#[test]
fn growing_preview_geometry_is_disposable_and_uses_the_render_cancellation_predicate() {
    let renderer = CpuRenderer::new();
    let mut editor = Editor::new_document(64, 64).unwrap();
    editor.select_tool(Tool::Brush);
    editor.pointer_down((4, 4));
    for i in 5..20 {
        editor.pointer_move((i, i));
        let batch = editor.preview_batch().unwrap().unwrap();
        let actual = renderer
            .stage(editor.document(), editor.selection(), &batch, &|| Ok(()))
            .unwrap();
        let expected = execute(editor.document(), editor.selection(), &batch).unwrap();
        assert_eq!(
            paintcraft_render::render_flat(&actual.doc).unwrap(),
            paintcraft_render::render_flat(&expected.doc).unwrap()
        );
    }
    assert!(renderer.1.lock().unwrap().metadata_charge() > 0);
    let batch = editor.preview_batch().unwrap().unwrap();
    let seen = std::cell::Cell::new(0);
    assert_eq!(
        renderer
            .stage(editor.document(), editor.selection(), &batch, &|| {
                seen.set(seen.get() + 1);
                if seen.get() == 5 {
                    Err(paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::OperationCancelled,
                        "cancel render staging",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .code,
        paintcraft_core::types::ErrorCode::OperationCancelled
    );
    assert_eq!(renderer.1.lock().unwrap().metadata_charge(), 0);
    renderer
        .stage(editor.document(), editor.selection(), &batch, &|| Ok(()))
        .unwrap();
    renderer
        .render(editor.document(), false, || Ok(()))
        .unwrap();
    assert_eq!(renderer.1.lock().unwrap().metadata_charge(), 0);
}

#[test]
fn a_poisoned_geometry_cache_is_retired_before_reuse() {
    let renderer = CpuRenderer::new();
    let failed = renderer.clone();
    assert!(
        std::thread::spawn(move || {
            let _guard = failed.1.lock().unwrap();
            panic!("injected geometry cache failure");
        })
        .join()
        .is_err()
    );
    let mut editor = Editor::new_document(32, 32).unwrap();
    editor.select_tool(Tool::Brush);
    editor.pointer_down((4, 4));
    let batch = editor.preview_batch().unwrap().unwrap();
    let staged = renderer
        .stage(editor.document(), editor.selection(), &batch, &|| Ok(()))
        .unwrap();
    assert_eq!(
        paintcraft_render::render_flat(&staged.doc).unwrap(),
        paintcraft_render::render_flat(
            &execute(editor.document(), editor.selection(), &batch)
                .unwrap()
                .doc
        )
        .unwrap()
    );
    assert!(!renderer.1.is_poisoned());
}
