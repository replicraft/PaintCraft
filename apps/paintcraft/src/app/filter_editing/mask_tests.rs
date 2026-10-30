//! Mask intentions traverse the real host prefix/FIFO, never the preview DTO.
use super::*;
use paintcraft_document::{FilterInstance, FilterStack};
fn masked_app(asynchronous: bool) -> App {
    let mut a = app(asynchronous);
    let mut doc = (*a.editor.document_snapshot()).clone();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(71),
            true,
            Descriptor::InvertRgbV1,
        )])
        .unwrap();
    a.editor = Editor::with_document(doc, None);
    if asynchronous {
        a.initialize_editing(a.context.clone());
    }
    a
}
fn begin(a: &mut App, ctx: &egui::Context, intent: Intent<Uuid>) {
    // Separate explicit interactions have separate frame tokens; layout passes
    // within a frame intentionally coalesce in the production FIFO.
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
    a.begin_studio_filter(ctx, intent);
}
fn current_mask(a: &App) -> paintcraft_document::FilterMask {
    a.editor.document().layers[&a.editor.target().unwrap()]
        .filters
        .as_slice()[0]
        .mask()
        .unwrap()
        .clone()
}
#[test]
fn actual_menu_selection_snapshot_disable_remove_and_undo_preserve_source() {
    for asynchronous in [false, true] {
        let mut h = harness(masked_app(asynchronous));
        h.run();
        let ctx = h.ctx.clone();
        let source = h.state().editor.document_snapshot();
        // Selection is an accepted prefix, including a real async completion.
        h.state_mut().editor.request(
            7001,
            Action::Select {
                corners: Some(((0, 0), (0, 0))),
            },
        );
        h.state_mut()
            .begin_studio_filter(&ctx, Intent::UseSelection(Uuid::from_u128(71)));
        drain(h.state_mut());
        h.run();
        let mask = current_mask(h.state());
        assert!(mask.enabled());
        assert_eq!(mask.coverage().coverage(0, 0), 255);
        assert_eq!(mask.coverage().coverage(1, 0), 0);
        // The accepted selection and mask publication each own one Undo step.
        assert_eq!(h.state().editor.undo_depth(), 2);
        frame(h.state_mut(), &ctx);
        assert_eq!(h.state().rendered, [245, 235, 225, 255, 100, 150, 200, 255]);
        h.state_mut()
            .editor
            .request(7002, Action::Select { corners: None });
        drain(h.state_mut());
        assert_eq!(
            current_mask(h.state()).coverage().digest(),
            mask.coverage().digest()
        );
        h.get_by_label("Filter actions").click();
        h.run();
        h.get_by_label("Enable filter mask").click();
        h.run_steps(1);
        drain(h.state_mut());
        h.run();
        assert!(!current_mask(h.state()).enabled());
        h.get_by_label("Filter actions").click();
        h.run();
        h.get_by_label("Remove mask").click();
        h.run_steps(1);
        drain(h.state_mut());
        h.run();
        assert!(
            h.state().editor.document().layers[&h.state().editor.target().unwrap()]
                .filters
                .as_slice()[0]
                .mask()
                .is_none()
        );
        h.state_mut().editor.request(7003, Action::Undo);
        drain(h.state_mut());
        assert!(!current_mask(h.state()).enabled());
        let doc = h.state().editor.document();
        assert_eq!(
            doc.layers[&doc.roots[0]].raster,
            source.layers[&source.roots[0]].raster
        );
        let bytes = paintcraft_formats::pcnd::write_compact(doc).unwrap();
        assert_eq!(bytes[4], 4);
        assert!(doc.same_semantics(&read_project(&bytes)));
    }
}
#[test]
fn active_empty_is_stored_and_recapture_is_noop_but_inactive_is_refused() {
    for asynchronous in [false, true] {
        let mut a = masked_app(asynchronous);
        let ctx = a.context.clone();
        let before = a.editor.document_snapshot();
        begin(&mut a, &ctx, Intent::UseSelection(Uuid::from_u128(71)));
        drain(&mut a);
        assert!(Arc::ptr_eq(&before, &a.editor.document_snapshot()));
        assert_eq!(a.editor.undo_depth(), 0);
        a.editor.request(
            7101,
            Action::Select {
                corners: Some(((90, 90), (90, 90))),
            },
        );
        begin(&mut a, &ctx, Intent::UseSelection(Uuid::from_u128(71)));
        drain(&mut a);
        assert_eq!(
            a.editor.selection(),
            paintcraft_editor::execute::Selection::Empty
        );
        let mask = current_mask(&a);
        assert_eq!(mask.coverage().stats().runs, 0);
        let source = a.editor.document_snapshot();
        let depth = a.editor.undo_depth();
        begin(&mut a, &ctx, Intent::UseSelection(Uuid::from_u128(71)));
        drain(&mut a);
        assert_eq!(a.editor.undo_depth(), depth);
        assert!(Arc::ptr_eq(&source, &a.editor.document_snapshot()));
        begin(&mut a, &ctx, Intent::Edit(Uuid::from_u128(71)));
        let base = &a
            .filter_editing
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .base;
        let preview = edited(
            base,
            Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(30, 0).unwrap()),
        );
        a.editor.set_filter_preview(Some(preview));
        frame(&mut a, &ctx);
        assert_eq!(a.rendered, RAW);
        assert_eq!(current_mask(&a), mask);
        a.cancel_filter_edit();
    }
}
#[test]
fn mask_publication_refusal_retains_selection_and_can_retry_without_private_draft_loss() {
    fn refuse(
        _: &paintcraft_document::Document,
        _: &dyn Fn() -> paintcraft_core::types::Result<()>,
    ) -> paintcraft_core::types::Result<()> {
        Err(paintcraft_core::types::Error::new(
            paintcraft_core::types::ErrorCode::ResourceLimit,
            "mask admission sentinel",
        ))
    }
    for asynchronous in [false, true] {
        let mut a = masked_app(asynchronous);
        let ctx = a.context.clone();
        a.editor.request(
            7201,
            Action::Select {
                corners: Some(((0, 0), (0, 0))),
            },
        );
        drain(&mut a);
        let source = a.editor.document_snapshot();
        let selection = a.editor.selection();
        let depth = a.editor.undo_depth();
        let dirty = a.editor.published_is_dirty();
        a.editor.test_persistent_validator(Some(refuse));
        begin(&mut a, &ctx, Intent::UseSelection(Uuid::from_u128(71)));
        drain(&mut a);
        assert!(Arc::ptr_eq(&source, &a.editor.document_snapshot()));
        assert_eq!(a.editor.selection(), selection);
        assert_eq!(a.editor.undo_depth(), depth);
        assert_eq!(a.editor.published_is_dirty(), dirty);
        a.editor.test_persistent_validator(Some(
            paintcraft_formats::pcnd::check_compact_metadata_admission,
        ));
        begin(&mut a, &ctx, Intent::UseSelection(Uuid::from_u128(71)));
        drain(&mut a);
        assert_eq!(a.editor.undo_depth(), depth + 1);
        let saved = current_mask(&a);
        begin(&mut a, &ctx, Intent::Edit(Uuid::from_u128(71)));
        a.filter_editing
            .as_mut()
            .unwrap()
            .draft
            .as_mut()
            .unwrap()
            .parameters
            .brightness = 44;
        begin(&mut a, &ctx, Intent::RemoveMask(Uuid::from_u128(71)));
        assert_eq!(
            a.filter_editing
                .as_ref()
                .unwrap()
                .draft
                .as_ref()
                .unwrap()
                .parameters
                .brightness,
            44
        );
        assert_eq!(current_mask(&a), saved);
        a.cancel_filter_edit();
        // An externally queued stale mask request must not place unrelated text.
        let edit = a.editor.filter_capture(
            operation(
                &a.editor,
                a.editor.target().unwrap(),
                Intent::RemoveMask(Uuid::from_u128(71)),
            )
            .unwrap(),
        );
        a.editor.request(
            7202,
            Action::Rename {
                layer: a.editor.target().unwrap(),
                name: "changed".into(),
            },
        );
        drain(&mut a);
        a.editor.select_tool(paintcraft_tools::Tool::Text);
        a.editor.pointer_down((0, 0));
        a.editor.pointer_up();
        a.editor.type_text("retained");
        let doc = a.editor.document_snapshot();
        assert!(!a.editor.request(7203, Action::Filter(edit)));
        assert!(a.editor.is_editing_text());
        assert!(Arc::ptr_eq(&doc, &a.editor.document_snapshot()));
        assert_eq!(current_mask(&a), saved);
    }
}
#[test]
fn mask_capture_fences_unaccepted_ime_and_live_gesture_without_retiring_them() {
    let mut a = masked_app(false);
    let ctx = a.context.clone();
    let source = a.editor.document_snapshot();
    a.ime_composing = true;
    begin(&mut a, &ctx, Intent::UseSelection(Uuid::from_u128(71)));
    assert!(a.ime_composing);
    assert!(a.filter_editing.is_none());
    a.ime_composing = false;
    a.editor.select_tool(paintcraft_tools::Tool::Select);
    a.editor.pointer_down((0, 0));
    a.editor.pointer_move((1, 0));
    assert!(a.editor.gesture_in_progress());
    let preview = a.editor.preview();
    begin(&mut a, &ctx, Intent::UseSelection(Uuid::from_u128(71)));
    assert!(a.editor.gesture_in_progress());
    assert_eq!(a.editor.preview(), preview);
    assert!(a.filter_editing.is_none());
    assert!(Arc::ptr_eq(&source, &a.editor.document_snapshot()));
}

fn read_project(bytes: &[u8]) -> paintcraft_document::Document {
    use paintcraft_formats::pcnd::{self, CoverageAdmission, MetadataAdmission, ReadAdmission};
    let realm = paintcraft_core::types::resources::default_coverage_budget();
    pcnd::read_with_admission_checked(
        bytes,
        2,
        65536,
        1,
        ReadAdmission {
            metadata: MetadataAdmission {
                bytes: 0,
                budget: &realm,
            },
            coverage: CoverageAdmission {
                bytes: 1 << 20,
                budget: &realm,
                limits: paintcraft_raster::coverage_mask::Limits::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap()
}
