//! A real classified allocation refusal retains the explicit intent on retry.
use super::*;
use paintcraft_core::types::LayerBlendMode;
#[test]
fn deferred_layer_properties_retry_keeps_original_batch_label_and_source() {
    let doc =
        paintcraft_document::new_document(2, 1, Some(Rgba([20, 40, 60, 255])), 0, POLICY).unwrap();
    let mut editor = Editor::with_document(doc, None);
    editor.connect_editing(background::Service::new(eframe::egui::Context::default()).unwrap());
    let source = editor.document_snapshot();
    let layer = editor.target().unwrap();
    let batch = Arc::new(
        strokes::captured_batch(
            editor.session.ticket(),
            vec![
                Value::object([
                    ("op", Value::str("layer.set_properties")),
                    ("layer_ref", layer_ref(layer)),
                    ("name", Value::str("Retried")),
                ]),
                Value::object([
                    ("op", Value::str("layer.set_blend_mode")),
                    ("layer_ref", layer_ref(layer)),
                    ("blend_mode", Value::str("screen")),
                ]),
            ],
        )
        .unwrap(),
    );
    let metadata = paintcraft_core::types::resources::SharedBudget::new(1 << 20);
    let blocker = metadata.reserve(1 << 20).unwrap();
    let mut account = metadata.account(AllocationClass::PaintMetadata);
    let refusal = account.resize(1).unwrap_err();
    let admission = Admission::new(u64::MAX).with_paint_metadata_budget(metadata.clone());
    let base = editor.session.history_ticket();
    let result = defer_failure(
        base,
        Request::LayerProperties(batch.clone()),
        Err(refusal),
        Realms::capture(&admission),
        || Ok(()),
    )
    .unwrap();
    let Completion::Deferred(deferred) = result else {
        panic!("classified refusal must defer");
    };
    assert!(
        matches!(&deferred.request, Request::LayerProperties(actual) if Arc::ptr_eq(actual, &batch))
    );
    editor.waiting_edit = Some(Waiting {
        text: None,
        filter: None,
        native_activation: None,
        deferred: *deferred,
        done: "Layer properties changed".into(),
        empty: "",
        target_alias: None,
        last_probe: std::time::Instant::now(),
    });
    editor.run_pending();
    assert!(Arc::ptr_eq(&source, &editor.document_snapshot()));
    assert_eq!(editor.undo_depth(), 0);
    assert!(editor.metadata_wait().is_some());
    drop(blocker);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while editor.has_pending_edits() {
        editor.run_pending();
        assert!(std::time::Instant::now() < deadline, "{}", editor.status());
        std::thread::yield_now();
    }
    assert_eq!(editor.undo_depth(), 1);
    assert_eq!(
        editor.history_view().entries.last().unwrap().label.text(),
        "Layer properties"
    );
    assert_eq!(editor.document().layers[&layer].name, "Retried");
    assert_eq!(
        editor.document().layers[&layer].blend_mode,
        LayerBlendMode::Screen
    );
    assert_eq!(
        editor.document().layers[&layer].raster,
        source.layers[&layer].raster
    );
    assert_eq!(metadata.usage().reserved, 0);
}
