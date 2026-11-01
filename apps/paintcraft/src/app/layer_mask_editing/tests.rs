//! Real controls plus admitted native backend tasks; no A8 work on the UI path.
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_core::types::{
    Error, ErrorCode,
    resources::{AllocationClass, SharedBudget},
};
use std::time::Instant;
const RAW: [u8; 8] = [64, 16, 128, 255, 200, 100, 100, 255];
fn app() -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::with_document(
        paintcraft_document::import_image(2, 1, &RAW, "native-layer-mask-actions/v1").unwrap(),
        None,
    );
    app.initialize_editing(app.context.clone());
    app
}
fn harness(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1000., 800.))
        .build_ui_state(
            |ui, app: &mut App| {
                app.studio_layer_mask_panel(ui);
                app.confirm_dialog(ui);
            },
            app,
        )
}
fn drain(app: &mut App) {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        app.editor.run_pending();
        app.pump_layer_mask_action(&app.context.clone());
        if !app.editor.has_pending_edits() && app.layer_mask_editing.is_none() {
            break;
        }
        assert!(Instant::now() < until, "{}", app.editor.status());
        std::thread::yield_now();
    }
}
fn begin(app: &mut App, intent: Intent) {
    let ctx = app.context.clone();
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
    app.begin_layer_mask_action(&ctx, intent);
}
fn pixels(app: &App) -> Vec<u8> {
    let budget = SharedBudget::new(crate::spatial_workspace::BYTES);
    let admission = paintcraft_render::spatial::WorkspaceAdmission::new(
        budget.scope(AllocationClass::SharedBudget),
        crate::spatial_workspace::BYTES,
        crate::spatial_workspace::WORK,
    );
    paintcraft_render::spatial::with_workspace(admission, || {
        paintcraft_render::render_flat(app.editor.document())
    })
    .unwrap()
}
fn source(app: &App) -> [u8; 8] {
    let d = app.editor.document();
    let r = &d.layers[&d.roots[0]].raster;
    [r.get(0, 0).0, r.get(1, 0).0].concat().try_into().unwrap()
}
#[test]
fn real_four_controls_publish_one_undo_preserve_raw_and_fresh_pcnd7_reopens_mask() {
    let mut h = harness(app());
    h.run();
    // Selection is a real accepted predecessor, not a persisted PCND property.
    h.state_mut().editor.request(
        9001,
        Action::Select {
            corners: Some(((0, 0), (0, 0))),
        },
    );
    drain(h.state_mut());
    h.run();
    let before = h.state().editor.undo_depth();
    h.get_by_label("Use selection as layer mask").click();
    h.run();
    drain(h.state_mut());
    h.run();
    assert_eq!(h.state().editor.undo_depth(), before + 1);
    assert_eq!(source(h.state()), RAW);
    assert_eq!(pixels(h.state()), [64, 16, 128, 255, 0, 0, 0, 0]);
    let d = h.state().editor.document();
    let bytes = paintcraft_formats::pcnd::write(d).unwrap();
    assert_eq!(bytes[4], 7);
    let budget = SharedBudget::new(1 << 20);
    let reopened = paintcraft_formats::pcnd::read_with_admission_checked(
        &bytes,
        2,
        131072,
        2,
        paintcraft_formats::pcnd::ReadAdmission {
            metadata: paintcraft_formats::pcnd::MetadataAdmission {
                bytes: 8 << 20,
                budget: &budget,
            },
            coverage: paintcraft_formats::pcnd::CoverageAdmission {
                bytes: 1 << 20,
                budget: &budget,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        reopened.layers[&reopened.roots[0]].mask,
        d.layers[&d.roots[0]].mask
    );
    h.get_by_label("Invert layer mask").click();
    h.run();
    drain(h.state_mut());
    h.run();
    assert_eq!(pixels(h.state()), [0, 0, 0, 0, 200, 100, 100, 255]);
    h.get_by_label("Enable layer mask").click();
    h.run();
    drain(h.state_mut());
    h.run();
    assert_eq!(pixels(h.state()), RAW);
    let disabled = h.state().editor.document_snapshot();
    h.get_by_label("Remove layer mask").click();
    h.run();
    drain(h.state_mut());
    h.run();
    assert_eq!(h.state().editor.undo_depth(), before + 4);
    let id = h.state().editor.target().unwrap();
    assert!(h.state().editor.document().layers[&id].mask.is_none());
    assert_eq!(source(h.state()), RAW);
    h.state_mut().editor.request(9002, Action::Undo);
    drain(h.state_mut());
    assert!(h.state().editor.document().same_semantics(&disabled));
    h.state_mut().editor.request(9003, Action::Redo);
    drain(h.state_mut());
    assert!(h.state().editor.document().layers[&id].mask.is_none());
}
#[test]
fn accepted_selection_prefix_is_captured_after_completion_and_cancel_does_not_cancel_prefix() {
    let mut a = app();
    let service = a.editing.as_ref().unwrap().clone();
    let (prepared, release) = service.pause_next_completion();
    a.editor.request(
        9101,
        Action::Select {
            corners: Some(((0, 0), (0, 0))),
        },
    );
    a.editor.run_pending();
    prepared.recv_timeout(Duration::from_secs(10)).unwrap();
    let mut h = harness(a);
    begin(h.state_mut(), Intent::UseSelection);
    assert!(h.state().layer_mask_editing.is_some());
    // The paused predecessor intentionally keeps the spinner repainting.
    h.run_steps(2);
    h.get_by_label("Cancel").click();
    h.run_steps(2);
    assert!(h.state().layer_mask_editing.is_none());
    release.send(()).unwrap();
    drain(h.state_mut());
    let id = h.state().editor.target().unwrap();
    assert!(h.state().editor.document().layers[&id].mask.is_none());
    assert!(matches!(h.state().editor.selection(), Selection::Rect(_)));
    begin(h.state_mut(), Intent::UseSelection);
    drain(h.state_mut());
    assert_eq!(pixels(h.state()), [64, 16, 128, 255, 0, 0, 0, 0]);
    assert_eq!(h.state().editor.undo_depth(), 2);
    assert_eq!(source(h.state()), RAW);
}
#[test]
fn no_service_and_private_input_refuse_before_side_effects_and_queued_old_ticket_cannot_apply() {
    let mut a = app();
    let id = a.editor.target().unwrap();
    let original = a.editor.document_snapshot();
    a.ime_composing = true;
    begin(&mut a, Intent::UseSelection);
    assert!(a.layer_mask_editing.is_none());
    assert!(a.ime_composing);
    assert!(Arc::ptr_eq(&original, &a.editor.document_snapshot()));
    a.ime_composing = false;
    let edit = a.editor.layer_mask_capture(LayerMaskOp::Remove {
        layer: LayerRef::Id(id),
    });
    a.editor.request(
        9201,
        Action::Select {
            corners: Some(((0, 0), (0, 0))),
        },
    );
    drain(&mut a);
    assert!(!a.editor.request(9202, Action::LayerMask(edit)));
    assert!(a.editor.document().layers[&id].mask.is_none());
    let mut sync = Editor::with_document((*original).clone(), None);
    let edit = sync.layer_mask_capture(LayerMaskOp::Remove {
        layer: LayerRef::Id(id),
    });
    assert_eq!(
        sync.validate_layer_mask_edit(&edit).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert!(!sync.request(9203, Action::LayerMask(edit)));
    assert_eq!(sync.undo_depth(), 0);
    assert_eq!(source(&a), RAW);
}
#[test]
fn worker_prepared_output_is_private_cancellable_and_validator_refusal_can_retry() {
    fn refuse(
        _: &paintcraft_document::Document,
        check: &dyn Fn() -> paintcraft_core::types::Result<()>,
    ) -> paintcraft_core::types::Result<()> {
        check()?;
        Err(Error::new(
            ErrorCode::ResourceLimit,
            "layer mask result ceiling",
        ))
    }
    let mut a = app();
    let id = a.editor.target().unwrap();
    a.editor.request(
        9301,
        Action::Select {
            corners: Some(((0, 0), (0, 0))),
        },
    );
    drain(&mut a);
    let before = a.editor.document_snapshot();
    let depth = a.editor.undo_depth();
    let ticket = a.editor.session_owner_id();
    let (prepared, release) = a.editing.as_ref().unwrap().pause_next_completion();
    begin(&mut a, Intent::UseSelection);
    a.editor.run_pending();
    prepared.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(Arc::ptr_eq(&before, &a.editor.document_snapshot()));
    assert_eq!(a.editor.undo_depth(), depth);
    a.editor.cancel_pending_edits();
    release.send(()).unwrap();
    drain(&mut a);
    assert!(Arc::ptr_eq(&before, &a.editor.document_snapshot()));
    a.editor.test_persistent_validator(Some(refuse));
    begin(&mut a, Intent::UseSelection);
    drain(&mut a);
    assert!(Arc::ptr_eq(&before, &a.editor.document_snapshot()));
    assert_eq!(a.editor.undo_depth(), depth);
    assert_eq!(a.editor.session_owner_id(), ticket);
    a.editor.test_persistent_validator(Some(
        paintcraft_formats::pcnd::check_compact_metadata_admission,
    ));
    begin(&mut a, Intent::UseSelection);
    drain(&mut a);
    assert_eq!(a.editor.undo_depth(), depth + 1);
    assert!(a.editor.document().layers[&id].mask.is_some());
    begin(&mut a, Intent::SetEnabled(true));
    drain(&mut a);
    assert_eq!(a.editor.undo_depth(), depth + 1);
    assert_eq!(source(&a), RAW);
}
#[test]
fn inactive_control_is_disabled_empty_is_enabled_and_waiting_foreign_owner_is_retired() {
    let mut h = harness(app());
    h.run();
    assert!(
        h.get_by_label("Use selection as layer mask")
            .accesskit_node()
            .is_disabled()
    );
    h.state_mut().editor.request(
        9401,
        Action::Select {
            corners: Some(((3, 3), (3, 3))),
        },
    );
    drain(h.state_mut());
    assert_eq!(h.state().editor.selection(), Selection::Empty);
    h.run();
    assert!(
        !h.get_by_label("Use selection as layer mask")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Use selection as layer mask").click();
    h.run();
    drain(h.state_mut());
    let id = h.state().editor.target().unwrap();
    let mask = h.state().editor.document().layers[&id]
        .mask
        .as_ref()
        .unwrap();
    assert!(mask.enabled());
    assert_eq!(mask.coverage().stats().runs, 0);
    assert_eq!(pixels(h.state()), [0; 8]);
    assert_eq!(source(h.state()), RAW);
    let mut a = app();
    a.editor.request(
        9402,
        Action::Select {
            corners: Some(((0, 0), (0, 0))),
        },
    );
    begin(&mut a, Intent::UseSelection);
    assert!(a.layer_mask_editing.is_some());
    a.editor = Editor::with_document(
        paintcraft_document::import_image(2, 1, &RAW, "foreign-layer-mask-owner").unwrap(),
        None,
    );
    let before = a.editor.document_snapshot();
    a.pump_layer_mask_action(&a.context.clone());
    assert!(a.layer_mask_editing.is_none());
    assert!(a.confirm.is_none());
    assert!(Arc::ptr_eq(&before, &a.editor.document_snapshot()));
    assert_eq!(a.editor.undo_depth(), 0);
}
