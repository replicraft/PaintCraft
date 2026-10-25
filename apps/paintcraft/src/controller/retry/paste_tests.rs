use super::tests::{setup, until};
use super::*;
use paintcraft_core::types::{clipboard::Image, policy::BlendMode, resources::SharedBudget};
use paintcraft_editor::clipboard::{PasteCanvas, PasteDraft};
use paintcraft_raster::RasterLayer;
const BYTES: u64 = RasterLayer::TILE_BYTES as u64;
const INK: Rgba = Rgba([200, 40, 10, 128]);
const KEY: Rgba = Rgba([10, 20, 30, 255]);

fn draft(editor: &Editor) -> PasteDraft {
    let image = Image::from_fn(257, 1, &SharedBudget::new(4096), &|| Ok(()), |x, _| {
        Ok(if x == 1 { KEY } else { INK })
    })
    .unwrap();
    let mut draft = editor.paste_draft(image).unwrap();
    draft.place((0, 0), PasteCanvas::Keep).unwrap();
    draft
}
fn settled(editor: &mut Editor) {
    until(|| {
        editor.run_pending();
        !editor.has_pending_edits()
    });
}
fn waiting(editor: &mut Editor) {
    until(|| {
        editor.run_pending();
        editor.allocation_wait().is_some()
    });
}
fn check_pixels(editor: &Editor, target: Uuid) {
    assert_eq!(editor.document().layers.len(), 1);
    for x in 0..257 {
        let expected = if x == 1 {
            Rgba([255; 4])
        } else {
            INK.source_over(Rgba([255; 4]))
        };
        assert_eq!(
            editor.document().layers[&target].raster.get(x, 0),
            expected,
            "x={x}"
        );
    }
}
#[test]
fn existing_layer_paste_waits_in_order_and_publishes_exactly_once_with_undo() {
    let (mut editor, _, budget) = setup(2);
    let blocker = budget.reserve(BYTES).unwrap();
    let original = editor.document_snapshot();
    let target = editor.target().unwrap();
    editor
        .start_paste_onto(draft(&editor), target, BlendMode::SourceOver, Some(KEY))
        .unwrap();
    let barrier = editor.queue_file_barrier();
    waiting(&mut editor);
    assert!(Arc::ptr_eq(&original, &editor.document_snapshot()));
    assert_eq!(editor.undo_depth(), 0);
    assert!(!editor.file_barrier_ready(barrier));
    let usage = budget.usage();
    for _ in 0..5 {
        editor.run_pending();
    }
    assert_eq!(usage, budget.usage());
    drop(blocker);
    until(|| {
        editor.run_pending();
        editor.file_barrier_ready(barrier)
    });
    check_pixels(&editor, target);
    assert_eq!(editor.publications(), 1);
    assert_eq!(editor.undo_depth(), 1);
    assert_eq!(editor.target(), Some(target));
    editor.release_file_barrier(barrier);
    editor.request(100, Action::Undo);
    settled(&mut editor);
    assert!(editor.document().same_semantics(&original));
    editor.request(101, Action::Redo);
    settled(&mut editor);
    check_pixels(&editor, target);
}
#[test]
fn cancelling_waiting_existing_layer_paste_drops_work_without_late_publication() {
    let (mut editor, _, budget) = setup(2);
    let blocker = budget.reserve(BYTES).unwrap();
    let original = editor.document_snapshot();
    let target = editor.target().unwrap();
    editor
        .start_paste_onto(draft(&editor), target, BlendMode::SourceOver, None)
        .unwrap();
    waiting(&mut editor);
    editor.cancel_pending_edits();
    drop(blocker);
    settled(&mut editor);
    assert!(Arc::ptr_eq(&original, &editor.document_snapshot()));
    assert_eq!(editor.publications(), 0);
    assert_eq!(editor.undo_depth(), 0);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn synchronous_and_worker_paste_match_with_expansion_and_stale_or_busy_rejection() {
    for asynchronous in [false, true] {
        let mut editor = Editor::new_document(257, 1).unwrap();
        if asynchronous {
            editor.connect_editing(
                background::Service::new(eframe::egui::Context::default()).unwrap(),
            );
        }
        let target = editor.target().unwrap();
        let mut source = draft(&editor);
        source.place((1, 0), PasteCanvas::Grow).unwrap();
        editor
            .start_paste_onto(source.clone(), target, BlendMode::Replace, None)
            .unwrap();
        settled(&mut editor);
        assert_eq!(editor.document().canvas, (258, 1));
        assert_eq!(editor.document().layers.len(), 1);
        assert_eq!(editor.document().layers[&target].raster.get(1, 0), INK);
        assert_eq!(editor.take_canvas_size_edit(), Some((258, 1)));
        assert_eq!(
            editor
                .start_paste_onto(source, target, BlendMode::Replace, None)
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
        let before = editor.document_snapshot();
        let valid = draft(&editor);
        assert!(
            editor
                .start_paste_onto(valid.clone(), Uuid::new_v4(), BlendMode::SourceOver, None)
                .is_err()
        );
        editor.request(10, Action::AddLayer);
        assert!(
            editor
                .start_paste_onto(valid, target, BlendMode::SourceOver, None)
                .is_err()
        );
        assert!(Arc::ptr_eq(&before, &editor.document_snapshot()));
        settled(&mut editor);
    }
}

#[test]
fn a_waiting_paste_is_not_rebased_onto_a_competing_publication() {
    let (mut editor, _, budget) = setup(2);
    let blocker = budget.reserve(BYTES).unwrap();
    let target = editor.target().unwrap();
    editor
        .start_paste_onto(draft(&editor), target, BlendMode::SourceOver, None)
        .unwrap();
    waiting(&mut editor);
    let ticket = editor.session.ticket();
    let batch = Batch::from_json(format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"canvas.clear","ref":"background","color":"#FF0000FF"}}]}}"##, ticket.state_id, ticket.publication_epoch).as_bytes()).unwrap();
    editor.session.apply(&batch).unwrap();
    let newer = editor.document_snapshot();
    let depth = editor.undo_depth();
    drop(blocker);
    settled(&mut editor);
    assert!(Arc::ptr_eq(&newer, &editor.document_snapshot()));
    assert_eq!(editor.undo_depth(), depth);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn impossible_paste_capacity_fails_without_publication_or_capacity_leaks() {
    let (mut editor, _, budget) = setup(1);
    let original = editor.document_snapshot();
    let target = editor.target().unwrap();
    editor
        .start_paste_onto(draft(&editor), target, BlendMode::SourceOver, None)
        .unwrap();
    settled(&mut editor);
    assert!(editor.allocation_wait().is_none());
    assert!(editor.status().contains("resource.limit"));
    assert!(Arc::ptr_eq(&original, &editor.document_snapshot()));
    assert_eq!(editor.undo_depth(), 0);
    assert_eq!(budget.usage().reserved, 0);
}
