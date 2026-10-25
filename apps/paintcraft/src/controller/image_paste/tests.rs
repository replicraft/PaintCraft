use super::*;
use paintcraft_core::types::{clipboard::Image, resources::SharedBudget};
use paintcraft_editor::clipboard::PasteCanvas;

fn draft(editor: &Editor) -> PasteDraft {
    let image = Image::from_fn(3, 2, &SharedBudget::new(1024), &|| Ok(()), |x, y| {
        Ok(Rgba([30 + x as u8, 40 + y as u8, 200, 128]))
    })
    .unwrap();
    let mut draft = editor.paste_draft(image).unwrap();
    draft.place((1, 1), PasteCanvas::Keep).unwrap();
    draft
}
fn preview(editor: &Editor) -> Document {
    editor
        .capture_image_paste()
        .unwrap()
        .build(&|| Ok(()))
        .unwrap()
}
#[test]
fn repeated_drags_and_nudges_are_private_and_place_exact_preview_in_one_undo_step() {
    let mut editor = Editor::new_document(12, 8).unwrap();
    let original = editor.document_snapshot();
    editor.begin_image_paste(draft(&editor), None).unwrap();
    assert!(editor.is_dirty());
    assert!(!editor.can_capture_file());
    let first = preview(&editor);
    let revision = editor.image_paste_revision();
    editor.pointer_down((1, 1));
    editor.pointer_move((4, 2));
    editor.pointer_up();
    assert!(editor.has_image_paste() && !editor.pointer_is_down());
    editor.pointer_down((4, 2));
    editor.pointer_move((6, 4));
    editor.pointer_up();
    editor.nudge_image_paste((-1, 0)).unwrap();
    assert_ne!(revision, editor.image_paste_revision());
    assert!(Arc::ptr_eq(&original, &editor.document_snapshot()));
    assert_eq!(editor.undo_depth(), 0);
    assert_ne!(first.layers, preview(&editor).layers);
    let expected = preview(&editor);
    editor.place_image_paste().unwrap();
    assert!(!editor.has_image_paste());
    assert_eq!(editor.document().layers, expected.layers);
    assert_eq!(editor.undo_depth(), 1);
    editor.request(1, Action::Undo);
    editor.run_pending();
    assert!(editor.document().same_semantics(&original));
}
#[test]
fn cancel_interrupt_invalid_position_and_guarded_actions_preserve_the_source() {
    let mut editor = Editor::new_document(12, 8).unwrap();
    let original = editor.document_snapshot();
    editor.begin_image_paste(draft(&editor), None).unwrap();
    editor.pointer_down((1, 1));
    editor.pointer_move((3, 2));
    editor.cancel_gesture();
    assert!(editor.has_image_paste() && !editor.pointer_is_down());
    let expected = preview(&editor);
    assert!(editor.nudge_image_paste((i64::MAX, 0)).is_err());
    assert_eq!(preview(&editor).layers, expected.layers);
    assert!(!editor.request(22, Action::AddLayer));
    assert!(editor.has_image_paste());
    assert!(!editor.request(23, Action::Undo));
    assert!(!editor.has_image_paste());
    assert!(Arc::ptr_eq(&original, &editor.document_snapshot()));
    assert!(!editor.is_dirty());
    assert_eq!(editor.undo_depth(), 0);
    // Retrying the consumed Undo activation cannot undo an unrelated older step.
    assert!(!editor.request(23, Action::Undo));
}
#[test]
fn growth_waits_for_placement_and_detached_preview_cancels_without_publication() {
    let mut editor = Editor::new_document(4, 4).unwrap();
    let mut source = draft(&editor);
    source.place((3, 3), PasteCanvas::Grow).unwrap();
    editor.begin_image_paste(source, None).unwrap();
    let current = editor.document_snapshot();
    assert_eq!(preview(&editor).canvas, (4, 4));
    let result = editor
        .capture_image_paste()
        .unwrap()
        .build(&|| Err(Error::new(ErrorCode::OperationCancelled, "cancelled")));
    assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
    assert!(Arc::ptr_eq(&current, &editor.document_snapshot()));
    editor.place_image_paste().unwrap();
    assert_eq!(editor.document().canvas, (6, 5));
    assert_eq!(editor.take_canvas_size_edit(), Some((6, 5)));
}

#[test]
fn pending_publication_retains_preview_and_cancel_or_success_clears_it_once() {
    use std::time::{Duration, Instant};
    for cancel in [false, true] {
        let budget = SharedBudget::new(2 * paintcraft_raster::RasterLayer::TILE_BYTES as u64);
        let mut service = background::Service::new(eframe::egui::Context::default()).unwrap();
        service.residency = budget.clone();
        let mut editor = Editor::new_document(257, 1).unwrap();
        editor.connect_editing(service);
        let original = editor.document_snapshot();
        let image = Image::from_fn(257, 1, &SharedBudget::new(4096), &|| Ok(()), |_, _| {
            Ok(Rgba([10, 20, 30, 255]))
        })
        .unwrap();
        let source = editor.paste_draft(image).unwrap();
        editor.begin_image_paste(source, None).unwrap();
        let expected = preview(&editor);
        let blocker = budget
            .reserve(paintcraft_raster::RasterLayer::TILE_BYTES as u64)
            .unwrap();
        editor.place_image_paste().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while editor.allocation_wait().is_none() {
            editor.run_pending();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(editor.has_image_paste());
        assert_eq!(preview(&editor).layers, expected.layers);
        assert!(Arc::ptr_eq(&original, &editor.document_snapshot()));
        // Repeated Apply cannot submit the same placement twice while it waits.
        editor.place_image_paste().unwrap();
        if cancel {
            editor.cancel_image_paste();
        }
        drop(blocker);
        while editor.has_pending_edits() {
            editor.run_pending();
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(!editor.has_image_paste());
        assert_eq!(editor.undo_depth(), usize::from(!cancel));
        if cancel {
            assert!(Arc::ptr_eq(&original, &editor.document_snapshot()));
        } else {
            assert_eq!(editor.document().layers, expected.layers);
        }
    }
}

#[test]
fn an_unchanged_worker_placement_closes_and_a_failed_one_stays_correctable() {
    use std::time::{Duration, Instant};
    let mut editor = Editor::new_document(12, 8).unwrap();
    let image = Image::from_fn(3, 2, &SharedBudget::new(1024), &|| Ok(()), |_, _| {
        Ok(Rgba([10, 20, 30, 255]))
    })
    .unwrap();
    let mut source = editor.paste_draft(image.clone()).unwrap();
    source.place((1, 1), PasteCanvas::Keep).unwrap();
    editor.begin_image_paste(source, None).unwrap();
    editor.place_image_paste().unwrap();
    let depth = editor.undo_depth();
    editor.connect_editing(background::Service::new(eframe::egui::Context::default()).unwrap());
    let mut source = editor.paste_draft(image).unwrap();
    source.place((1, 1), PasteCanvas::Keep).unwrap();
    editor.begin_image_paste(source, None).unwrap();
    editor.place_image_paste().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while editor.has_pending_edits() {
        editor.run_pending();
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(!editor.has_image_paste());
    assert_eq!(editor.undo_depth(), depth);

    let mut limited = Editor::new_document(12, 8).unwrap();
    let mut service = background::Service::new(eframe::egui::Context::default()).unwrap();
    service.residency = SharedBudget::new(1);
    limited.connect_editing(service);
    let original = limited.document_snapshot();
    limited.begin_image_paste(draft(&limited), None).unwrap();
    limited.place_image_paste().unwrap();
    while limited.has_pending_edits() {
        limited.run_pending();
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    limited.run_pending();
    assert!(limited.has_image_paste());
    assert_eq!(limited.undo_depth(), 0);
    assert!(Arc::ptr_eq(&original, &limited.document_snapshot()));
    limited.nudge_image_paste((1, 0)).unwrap();
    limited.cancel_image_paste();
    assert!(!limited.has_image_paste());
}
