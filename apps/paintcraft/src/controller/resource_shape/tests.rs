use super::*;
const SOURCE: &[u8] = include_bytes!(
    "../../../../../crates/services/paintcraft-formats/tests/csh-fixtures/placement.csh"
);
fn editor() -> Editor {
    let mut e = Editor::with_document(
        paintcraft_document::new_document(32, 32, None, 0, POLICY).unwrap(),
        None,
    );
    e.use_resource_shape(
        Reference {
            library: Uuid::from_u128(7),
            preset: 1,
            policy: Policy::CubicFillV1.id(),
        },
        Arc::new(csh::decode(SOURCE, &|| Ok(())).unwrap()),
        "Lens".into(),
    )
    .unwrap();
    e
}
fn drain(e: &mut Editor) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        e.run_pending();
        if !e.has_pending_edits() {
            break;
        }
        assert!(std::time::Instant::now() < end);
        std::thread::yield_now();
    }
}
#[test]
fn native_worker_drag_uses_exact_cubic_preview_and_shape_transaction_without_stock_fallback() {
    let mut e = editor();
    let before = e.document_snapshot();
    e.color = Rgba([40, 80, 120, 128]);
    e.connect_editing(background::Service::new(eframe::egui::Context::default()).unwrap());
    e.pointer_down((4, 5));
    e.pointer_move((20, 17));
    let expected = paintcraft_editor::execute::execute(
        &before,
        e.selection(),
        &e.preview_batch().unwrap().unwrap(),
    )
    .unwrap()
    .doc;
    e.pointer_up();
    drain(&mut e);
    assert_eq!(
        paintcraft_render::render_flat(e.document()).unwrap(),
        paintcraft_render::render_flat(&expected).unwrap()
    );
    assert_eq!(e.undo_depth(), 1);
    assert_eq!(
        e.history_view().entries.last().unwrap().label.text(),
        "Shape"
    );
    e.request(910, Action::Undo);
    drain(&mut e);
    assert!(e.document().layers.is_empty());
    let clean = e.document_snapshot();
    e.color = Rgba::TRANSPARENT;
    e.pointer_down((2, 2));
    e.pointer_move((8, 8));
    e.pointer_up();
    drain(&mut e);
    assert!(Arc::ptr_eq(&clean, &e.document_snapshot()));
    assert_eq!(e.undo_depth(), 0);
    e.color = Rgba([1, 2, 3, 255]);
    e.pointer_down((2, 2));
    e.pointer_move((8, 8));
    e.cancel_gesture();
    e.pointer_up();
    drain(&mut e);
    assert!(Arc::ptr_eq(&clean, &e.document_snapshot()));
    e.pointer_down((2, 2));
    e.pointer_move((8, 8));
    e.resource_shape = None;
    assert!(e.preview_batch().unwrap().is_err());
    e.pointer_up();
    drain(&mut e);
    assert!(Arc::ptr_eq(&clean, &e.document_snapshot()));
    assert_eq!(
        e.status(),
        "The selected custom shape is no longer available"
    );
}
#[test]
fn native_picker_restores_only_the_captured_resource_owner_and_rejects_missing_owner() {
    use paintcraft_core::types::picker::PickerAfter;
    let mut e = editor();
    e.options.picker_after = PickerAfter::Previous;
    e.options.picker_source = paintcraft_core::types::picker::SampleSource::Image;
    e.select_tool(Tool::ColorPicker);
    e.pointer_down((0, 0));
    e.pointer_up();
    drain(&mut e);
    assert_eq!(e.resource_shape_name(), Some("Lens"));
    assert_eq!(e.tool, Tool::Rect);
    assert_eq!(e.undo_depth(), 0);
    e.select_tool(Tool::ColorPicker);
    e.pointer_down((0, 0));
    let (_, action) = e.tools.pointer_up().unwrap();
    e.resource_shape = None;
    e.request(912, action);
    drain(&mut e);
    assert_eq!(e.tool, Tool::ColorPicker);
    assert!(e.resource_shape_name().is_none());
    assert_eq!(e.undo_depth(), 0);
}
