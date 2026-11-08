use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use std::time::{Duration, Instant};
fn editor() -> Editor {
    Editor::with_document(
        paintcraft_document::new_document(
            2,
            1,
            Some(Rgba([204, 77, 13, 255])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap(),
        None,
    )
}
fn request(editor: &Editor, mode: LayerBlendMode) -> PropertiesRequest {
    PropertiesRequest {
        capture: editor.capture_properties(editor.target().unwrap()).unwrap(),
        desired_mode: mode,
        values: paintcraft_studio::LayerProperties {
            name: "Combined".into(),
            visible: false,
            opacity: 127,
        },
    }
}
fn complete(editor: &mut Editor) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        editor.run_pending();
        if !editor.has_pending_edits() {
            break;
        }
        assert!(Instant::now() < deadline, "{}", editor.status());
        std::thread::yield_now();
    }
}
#[test]
fn combined_request_uses_actual_fifo_dedupe_one_undo_and_noop_with_paused_worker() {
    for asynchronous in [false, true] {
        let mut e = editor();
        // Match App's existing host budget, including its workspace headroom.
        let service = asynchronous.then(|| {
            background::Service::with_budget(
                eframe::egui::Context::default(),
                SharedBudget::new(3 << 30),
            )
            .unwrap()
        });
        if let Some(service) = &service {
            e.connect_editing(service.clone());
        }
        let source = e.document_snapshot();
        let layer = e.target().unwrap();
        let first = request(&e, LayerBlendMode::Screen);
        let duplicate = request(&e, LayerBlendMode::Multiply);
        let late_duplicate = request(&e, LayerBlendMode::Multiply);
        assert!(e.request_layer_properties(7001, first));
        assert!(!e.request_layer_properties(7001, duplicate));
        assert!(Arc::ptr_eq(&source, &e.document_snapshot()));
        if let Some(service) = &service {
            let release = service.pause();
            e.run_pending();
            assert!(e.has_pending_edits());
            assert!(Arc::ptr_eq(&source, &e.document_snapshot()));
            release.send(()).unwrap();
        }
        complete(&mut e);
        let status = e.status().to_owned();
        assert!(!e.request_layer_properties(7001, late_duplicate));
        assert_eq!(
            e.status(),
            status,
            "dedupe precedes now-stale capture validation"
        );
        assert_eq!(e.undo_depth(), 1);
        assert_eq!(
            e.history_view().entries.last().unwrap().label.text(),
            "Layer properties"
        );
        let applied = e.document_snapshot();
        let value = &applied.layers[&layer];
        assert_eq!(value.name, "Combined");
        assert_eq!(
            (value.visible, value.opacity, value.blend_mode),
            (false, 127, LayerBlendMode::Screen)
        );
        assert_eq!(value.raster, source.layers[&layer].raster);
        e.request(7002, Action::Undo);
        complete(&mut e);
        assert!(
            e.document().same_semantics(&source),
            "async={asynchronous} status={} undo={} current={:?} source={:?}",
            e.status(),
            e.undo_depth(),
            e.document().layer(&layer),
            source.layer(&layer)
        );
        e.request(7003, Action::Redo);
        complete(&mut e);
        assert!(e.document().same_semantics(&applied));
        let noop = request(&e, LayerBlendMode::Screen);
        assert!(e.request_layer_properties(7004, noop));
        complete(&mut e);
        assert_eq!(e.undo_depth(), 1);
        assert!(e.document().same_semantics(&applied));
    }
}
#[test]
fn preparation_only_realm_refuses_navigation_workspace_without_replacing_source() {
    let mut e = editor();
    let budget = SharedBudget::new(2 << 30);
    let service =
        background::Service::with_budget(eframe::egui::Context::default(), budget.clone()).unwrap();
    e.connect_editing(service);
    assert!(e.request_layer_properties(7051, request(&e, LayerBlendMode::Screen)));
    complete(&mut e);
    assert_eq!(e.undo_depth(), 1);
    let applied = e.document_snapshot();
    let ticket = e.history_view().ticket;
    let before = budget.usage();
    assert!(e.request(7052, Action::Undo));
    complete(&mut e);
    assert!(e.status().contains("resource.limit"));
    assert!(Arc::ptr_eq(&applied, &e.document_snapshot()));
    assert_eq!(e.history_view().ticket, ticket);
    assert_eq!(e.undo_depth(), 1);
    assert_eq!(budget.usage(), before);
}
#[test]
fn later_accepted_action_stays_after_combined_properties_and_stale_stamp_is_not_rebased() {
    let mut e = editor();
    let layer = e.target().unwrap();
    let source = e.document_snapshot();
    let edit = request(&e, LayerBlendMode::Multiply);
    assert!(e.request_layer_properties(7010, edit));
    assert!(e.request(
        7011,
        Action::Rename {
            layer,
            name: "Later".into()
        }
    ));
    complete(&mut e);
    assert_eq!(e.document().layers[&layer].name, "Later");
    assert_eq!(
        e.document().layers[&layer].blend_mode,
        LayerBlendMode::Multiply
    );
    assert_eq!(e.undo_depth(), 2);
    e.request(7012, Action::Undo);
    complete(&mut e);
    assert_eq!(e.document().layers[&layer].name, "Combined");
    assert_eq!(
        e.document().layers[&layer].blend_mode,
        LayerBlendMode::Multiply
    );
    e.request(7013, Action::Undo);
    complete(&mut e);
    assert!(e.document().same_semantics(&source));
    let stale = request(&e, LayerBlendMode::Screen);
    e.request(
        7014,
        Action::Rename {
            layer,
            name: "Earlier".into(),
        },
    );
    complete(&mut e);
    let before = e.document_snapshot();
    assert!(!e.request_layer_properties(7015, stale));
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
    assert_eq!(
        e.document().layers[&layer].blend_mode,
        LayerBlendMode::Normal
    );
    let mut invalid = request(&e, LayerBlendMode::Multiply);
    invalid.values.name = "x".repeat(1025);
    assert!(!e.request_layer_properties(7016, invalid));
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
    let cross_session = request(&e, LayerBlendMode::Screen);
    let mut other = Editor::with_document(e.document().clone(), None);
    let other_before = other.document_snapshot();
    assert!(!other.request_layer_properties(7017, cross_session));
    assert!(Arc::ptr_eq(&other_before, &other.document_snapshot()));
    assert_eq!(other.undo_depth(), 0);
}
fn reject_persistence(_: &Document, check: &dyn Fn() -> Result<()>) -> Result<()> {
    check()?;
    Err(Error::new(
        ErrorCode::ResourceLimit,
        "test persistent boundary",
    ))
}
#[test]
fn actual_persistent_refusal_rolls_back_both_operations_sync_and_background() {
    for asynchronous in [false, true] {
        let mut e = editor();
        e.session =
            Session::new(e.document().clone()).with_persistent_validator(Some(reject_persistence));
        if asynchronous {
            e.connect_editing(background::Service::new(eframe::egui::Context::default()).unwrap());
        }
        let source = e.document_snapshot();
        let ticket = e.session.ticket();
        let edit = request(&e, LayerBlendMode::Screen);
        assert!(e.request_layer_properties(7020, edit));
        complete(&mut e);
        assert!(Arc::ptr_eq(&source, &e.document_snapshot()));
        assert_eq!(e.session.ticket(), ticket);
        assert_eq!(e.undo_depth(), 0);
    }
}
#[test]
fn pure_metadata_request_keeps_actual_caller_workspace_without_new_grant() {
    use paintcraft_render::spatial::{
        WorkspaceAdmission, capture_current_workspace, with_workspace,
    };
    let mut e = editor();
    let budget = SharedBudget::new(crate::spatial_workspace::BYTES);
    let parent = WorkspaceAdmission::new(
        budget.scope(AllocationClass::SharedBudget),
        crate::spatial_workspace::BYTES,
        10_000,
    );
    with_workspace(parent.clone(), || {
        let edit = request(&e, LayerBlendMode::Multiply);
        assert!(e.request_layer_properties(7030, edit));
        complete(&mut e);
        assert!(capture_current_workspace().unwrap().same_job(&parent));
        assert_eq!(
            parent.work_used(),
            0,
            "metadata edit adds no invented RGB charge/grant"
        );
        Ok(())
    })
    .unwrap();
    assert!(capture_current_workspace().is_none());
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn actual_prepared_properties_recheck_retired_native_source_before_any_publication() {
    use paintcraft_ui::pen_input::{Owner, Source, View};
    let mut e = editor();
    let service = background::Service::new(eframe::egui::Context::default()).unwrap();
    e.connect_editing(service.clone());
    let before = e.document_snapshot();
    let ticket = e.session.ticket();
    let owner = Owner {
        session: e.session.owner_id().as_u128(),
        state: ticket.state_id.as_u128(),
        publication: ticket.publication_epoch,
        interface: 0,
    };
    let budget = SharedBudget::new(16 << 20);
    let source = Source::new(16, 16, &budget).unwrap();
    source
        .publish(View {
            owner,
            transform: paintcraft_input::tablet::Transform {
                owner: owner.session,
                epoch: owner.publication,
                origin: (0.0, 0.0),
                points_per_pixel: 1.0,
            },
            pixels_per_point: 1.0,
            canvas: eframe::egui::Rect::from_min_max(
                eframe::egui::pos2(0.0, 0.0),
                eframe::egui::pos2(16.0, 16.0),
            ),
            exclusions: [None; 64],
            focused: true,
            blocked: false,
            ime: false,
        })
        .unwrap();
    let guard = source.ui_publication_guard(owner).unwrap();
    let scope = native_publication::Scope::new(
        native_publication::Activation::new(owner.session, 1, 1),
        guard,
        None,
    );
    e.begin_native_activation(scope);
    let (ready, release) = service.pause_next_completion();
    let edit = request(&e, LayerBlendMode::Screen);
    assert!(e.request_layer_properties(7040, edit));
    e.end_native_activation();
    e.run_pending();
    ready.recv_timeout(Duration::from_secs(20)).unwrap();
    source.cancel();
    release.send(()).unwrap();
    complete(&mut e);
    assert!(Arc::ptr_eq(&before, &e.document_snapshot()));
    assert_eq!(e.session.ticket(), ticket);
    assert_eq!(e.undo_depth(), 0);
    assert!(e.take_native_publication().is_none());
}

#[test]
fn generic_property_and_blend_batches_keep_their_existing_batch_label() {
    for asynchronous in [false, true] {
        let mut e = editor();
        if asynchronous {
            e.connect_editing(
                background::Service::with_budget(
                    eframe::egui::Context::default(),
                    SharedBudget::new(3 << 30),
                )
                .unwrap(),
            );
        }
        let source = e.document_snapshot();
        let layer = e.target().unwrap();
        let batch = strokes::captured_batch(
            e.session.ticket(),
            vec![
                Value::object([
                    ("op", Value::str("layer.set_properties")),
                    ("layer_ref", layer_ref(layer)),
                    ("name", Value::str("Generic")),
                ]),
                Value::object([
                    ("op", Value::str("layer.set_blend_mode")),
                    ("layer_ref", layer_ref(layer)),
                    ("blend_mode", Value::str("screen")),
                ]),
            ],
        )
        .unwrap();
        e.commit_batch(batch, "Generic batch", None);
        complete(&mut e);
        assert_eq!(e.undo_depth(), 1);
        assert_eq!(
            e.history_view().entries.last().unwrap().label.text(),
            "Batch edit"
        );
        assert_eq!(e.document().layers[&layer].name, "Generic");
        assert_eq!(
            e.document().layers[&layer].blend_mode,
            LayerBlendMode::Screen
        );
        assert_eq!(
            e.document().layers[&layer].raster,
            source.layers[&layer].raster
        );
    }
}

#[test]
fn fields_only_properties_keep_one_layer_properties_intent_when_mode_is_unchanged() {
    for asynchronous in [false, true] {
        let mut e = editor();
        if asynchronous {
            e.connect_editing(
                background::Service::with_budget(
                    eframe::egui::Context::default(),
                    SharedBudget::new(3 << 30),
                )
                .unwrap(),
            );
        }
        let source = e.document_snapshot();
        let edit = request(&e, LayerBlendMode::Normal);
        assert!(e.request_layer_properties(7070, edit));
        complete(&mut e);
        assert_eq!(e.undo_depth(), 1);
        assert_eq!(
            e.history_view().entries.last().unwrap().label.text(),
            "Layer properties"
        );
        let layer = &e.document().layers[&e.target().unwrap()];
        assert_eq!(
            (&*layer.name, layer.visible, layer.opacity, layer.blend_mode),
            ("Combined", false, 127, LayerBlendMode::Normal)
        );
        assert!(e.request_layer_properties(7071, request(&e, LayerBlendMode::Normal)));
        complete(&mut e);
        assert_eq!(e.undo_depth(), 1);
        assert_eq!(
            e.history_view().entries.last().unwrap().label.text(),
            "Layer properties"
        );
        assert!(e.request(7072, Action::Undo));
        complete(&mut e);
        assert!(e.document().same_semantics(&source));
        assert_eq!(e.undo_depth(), 0);
    }
}
