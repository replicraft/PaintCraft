//! Actual Rust originals/epaint aliases; no JS transport-double provenance claim.
use super::*;
use paintcraft_core::types::{
    Error, ErrorCode, LayerBlendMode,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_raster::Admission;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
fn factory(cap: u64) -> (SharedBudget, factory::Factory8, Rc<Cell<u32>>) {
    let budget = SharedBudget::new(cap);
    let original = budget
        .reserve_capacity(cap)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let admission = Admission::new(0).with_paint_metadata_scope(original);
    let released = Rc::new(Cell::new(0));
    let factory = factory::Factory8::observed(&admission, cap, released.clone()).unwrap();
    drop(admission);
    (budget, factory, released)
}
fn clear(ctx: &eframe::egui::Context) {
    ctx.tex_manager().write().take_delta().clear();
}
fn stage(factory: factory::Factory8, pixels: &[u8], width: u32) -> frame::FrameStage8 {
    let mut stage =
        frame::FrameStage8::admit(operation::Operation8::view(factory).unwrap(), [width, 1])
            .unwrap();
    assert_eq!(stage.byte_len(), pixels.len());
    stage.fill_test(pixels).unwrap();
    stage
}
#[test]
fn reserved_original_is_exact_and_never_replaced_by_global_headroom() {
    let budget = SharedBudget::new(256 << 20);
    let original = budget
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let admission = Admission::new(0).with_paint_metadata_scope(original.clone());
    let captured = factory::original_scope(&admission, 1 << 20).unwrap();
    let mut held = original.account();
    held.resize(37).unwrap();
    assert_eq!(captured.footprint().live, 37);
    assert!(factory::original_scope(&admission, (1 << 20) - 1).is_err());
    assert!(factory::original_scope(&admission, (1 << 20) + 1).is_err());
    let oversized =
        Admission::new(0).with_reserved_paint_metadata(budget.reserve_capacity(128 << 20).unwrap());
    assert!(factory::original_scope(&oversized, 64 << 20).is_err());
    let tight = admission.clone().with_paint_metadata_limit(4096);
    assert!(factory::original_scope(&tight, 1 << 20).is_err());
    assert_eq!(original.footprint().live, 37);
}
#[test]
fn unreserved_root_acquires_only_original_budget_and_preserves_real_denial() {
    let budget = SharedBudget::new(65536);
    let admission = Admission::new(0).with_paint_metadata_budget(budget.clone());
    let original = factory::original_scope(&admission, 49152).unwrap();
    assert!(original.capacity_snapshot().reserved);
    assert_eq!(original.capacity_snapshot().ceiling, 49152);
    assert_eq!(budget.usage().reserved, 49152);
    assert!(factory::original_scope(&admission, 32768).is_err());
    assert_eq!(budget.usage().reserved, 49152);
    drop(original);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn independent_metadata_bytes_do_not_tighten_from_unrelated_base_but_work_debt_is_original() {
    let budget = SharedBudget::new(64);
    let base = paintcraft_render::spatial::WorkspaceAdmission::new(
        budget.scope(AllocationClass::SharedBudget),
        1,
        100,
    );
    base.reserve_work(17).unwrap();
    let parent =
        paintcraft_render::spatial::with_workspace(base.clone(), operation::Parent8::capture)
            .unwrap();
    let (metadata, factory, released) = factory(1 << 20);
    let op = operation::Operation8::preparation(parent, factory).unwrap();
    op.spend(7).unwrap();
    assert_eq!(base.work_used(), 24);
    assert!(op.spend(77).is_err());
    assert_eq!(base.work_used(), 24);
    assert!(op.spend(77).is_err());
    assert_eq!(base.work_used(), 24);
    assert_eq!(metadata.usage().reserved, 1 << 20);
    drop(op);
    assert_eq!(released.get(), 1);
    assert_eq!(metadata.usage().reserved, 0);
}
#[test]
fn actual_frame_counter_prepays_full_copy_and_conversion_then_remains_same_through_retirement() {
    let (budget, factory, released) = factory(1 << 20);
    let stage = stage(factory, &[20, 60, 100, 64, 7, 9, 11, 0], 2);
    let funded = stage.funding();
    assert_eq!(funded.used(), 331184); //131072+64+46*8+12*16384+3*1024.
    let mut publication = stage.into_publication().unwrap();
    assert_eq!(
        publication
            .image()
            .pixels
            .iter()
            .map(|p| p.to_array())
            .collect::<Vec<_>>(),
        vec![[5, 15, 25, 64], [0, 0, 0, 0]]
    );
    assert!(!funded.js_is_retired());
    funded.js_retired().unwrap();
    assert!(publication.poll_retired().unwrap());
    assert_eq!(funded.used(), 332208);
    assert_eq!(released.get(), 0); //last actual original owner still held.
    drop(publication);
    drop(funded);
    assert_eq!(released.get(), 1);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn actual_pending_delta_survives_texture_handle_drop_until_its_own_image_retires() {
    let (budget, factory, released) = factory(1 << 20);
    let stage = stage(factory, &[20, 60, 100, 64], 1);
    let funded = stage.funding();
    funded.js_retired().unwrap();
    let mut publication = stage.into_publication().unwrap();
    let ctx = eframe::egui::Context::default();
    let texture = publication.install(&ctx);
    let mut delta = ctx.tex_manager().write().take_delta();
    assert!(!publication.poll_retired().unwrap());
    drop(texture);
    assert!(!publication.poll_retired().unwrap());
    assert_eq!(released.get(), 0);
    delta.clear();
    assert!(publication.poll_retired().unwrap());
    clear(&ctx);
    drop(publication);
    drop(funded);
    assert_eq!(released.get(), 1);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn visible_gpu_handle_can_remain_after_actual_cpu_delta_and_original_credit_retire() {
    let (budget, factory, released) = factory(1 << 20);
    let stage = stage(factory, &[1, 2, 3, 255], 1);
    let funded = stage.funding();
    funded.js_retired().unwrap();
    let mut publication = stage.into_publication().unwrap();
    let ctx = eframe::egui::Context::default();
    let texture = publication.install(&ctx);
    assert!(!publication.poll_retired().unwrap());
    clear(&ctx);
    assert!(publication.poll_retired().unwrap());
    drop(publication);
    drop(funded);
    assert_eq!(released.get(), 1);
    assert_eq!(budget.usage().reserved, 0);
    assert!(ctx.tex_manager().read().meta(texture.id()).is_some());
    drop(texture);
    clear(&ctx);
}
#[test]
fn weak_or_extra_strong_alias_refuses_even_when_javascript_root_is_gone() {
    let (budget, factory, released) = factory(1 << 20);
    let stage = stage(factory, &[1, 2, 3, 4], 1);
    let funded = stage.funding();
    funded.js_retired().unwrap();
    let mut publication = stage.into_publication().unwrap();
    let strong = publication.image().clone();
    let weak = Arc::downgrade(publication.image());
    assert!(!publication.poll_retired().unwrap());
    drop(strong);
    assert!(!publication.poll_retired().unwrap());
    assert_eq!(released.get(), 0);
    drop(weak);
    assert!(publication.poll_retired().unwrap());
    drop(publication);
    drop(funded);
    assert_eq!(released.get(), 1);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn quarantine_blocks_new_view_before_any_replacement_counter_is_created() {
    let (_budget, factory, _released) = factory(1 << 20);
    factory.quarantine();
    let before = factory.scope().footprint();
    assert!(operation::Operation8::view(factory.clone()).is_err());
    assert_eq!(factory.scope().footprint(), before);
}
#[test]
fn cancelled_unpublished_stage_retires_data_but_waits_for_actual_js_root_release() {
    let (budget, factory, released) = factory(1 << 20);
    let stage = stage(factory, &[1, 2, 3, 4], 1);
    let funded = stage.funding();
    drop(stage);
    assert_eq!(released.get(), 0);
    assert!(!funded.js_is_retired());
    funded.js_retired().unwrap();
    drop(funded);
    assert_eq!(released.get(), 1);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn raw_callback_error_identity_is_preserved_after_original_debit() {
    let (_budget, factory, _released) = factory(1 << 20);
    let op = operation::Operation8::view(factory).unwrap();
    let error = Error::new(
        ErrorCode::OperationCancelled,
        String::from("original Main arbitrary callback diagnostic"),
    )
    .at(23);
    let pointer = error.message.as_ptr();
    let error = RefCell::new(Some(error));
    let returned = op
        .step(&|| Err(error.borrow_mut().take().unwrap()))
        .unwrap_err();
    assert_eq!(returned.message.as_ptr(), pointer);
    assert_eq!(returned.operation_index, Some(23));
    assert_eq!(op.used(), 1024);
}
#[test]
fn genuine_counted_snapshot_and_role_forecasts_bind_same_source_and_original_factory() {
    let (budget, factory, released) = factory(8 << 20);
    let mut document =
        paintcraft_document::import_image(2, 1, &[20, 60, 100, 128, 7, 9, 11, 0], "Main8").unwrap();
    document
        .layers
        .get_mut(&document.roots[0])
        .unwrap()
        .blend_mode = LayerBlendMode::Multiply;
    let document = Arc::new(document);
    let identity = capture::Identity8 {
        owner: uuid::Uuid::from_u128(4),
        ticket: paintcraft_editor::session::Ticket {
            state_id: document.state_id,
            publication_epoch: 17,
        },
        document: document.clone(),
        draft: uuid::Uuid::from_u128(5),
        profile: paintcraft_web_render_worker::blend_service::LayerBlendService::compiled_profile()
            .unwrap(),
    };
    let key = capture::Key8 {
        identity,
        parent: operation::Parent8::capture().unwrap(),
    };
    let mut snapshot = snapshot::Snapshot8::prepare(key, factory, &|| Ok(())).unwrap();
    assert!(Arc::ptr_eq(&snapshot.identity.document, &document));
    assert_eq!(snapshot.identity.owner, uuid::Uuid::from_u128(4));
    assert_eq!(snapshot.identity.ticket.state_id, document.state_id);
    assert_eq!(snapshot.identity.ticket.publication_epoch, 17);
    assert_eq!(snapshot.identity.draft, uuid::Uuid::from_u128(5));
    assert_eq!(snapshot.identity.profile, snapshot.roles.compiled_profile);
    let wire = snapshot.bytes().unwrap().to_vec();
    assert!(snapshot.demand.stored_non_normal);
    assert_eq!(
        snapshot.demand.document_id,
        *document.document_id.as_bytes()
    );
    assert_eq!(snapshot.demand.state_id, *document.state_id.as_bytes());
    assert_eq!(snapshot.roles.roles[0], wire.len() as u64);
    assert_eq!(snapshot.workspace.workspace_bytes, snapshot.roles.roles[8]);
    let verified = paintcraft_formats::pcnd::CheckedContainer::verify(&wire, &|| Ok(())).unwrap();
    assert!(
        verified
            .portable_source8_plan_checked(
                Default::default(),
                snapshot.factory().scope(),
                &|| Ok(()),
                &|_| Ok(())
            )
            .is_ok()
    );
    assert!(!snapshot.transfer_pending());
    snapshot.begin_transfer().unwrap();
    assert!(snapshot.transfer_pending());
    snapshot.opened();
    assert!(!snapshot.transfer_pending());
    assert!(snapshot.bytes().is_err());
    assert_eq!(released.get(), 0);
    drop(snapshot);
    assert_eq!(released.get(), 1);
    assert_eq!(budget.usage().reserved, 0);
}

#[path = "tests/causal_extensions.rs"]
mod causal_extensions;

#[test]
fn actual_source_identity_and_canonical_view_refuse_distinct_arc_or_geometry() {
    let ctx = eframe::egui::Context::default();
    let app = crate::app::WebApp::new(&ctx);
    let identity = capture::Key8::capture(&app).unwrap().identity;
    assert!(identity.current(&app));
    assert!(identity.same_source(&identity.clone()));
    let mut foreign = identity.clone();
    foreign.document = Arc::new((*identity.document).clone());
    assert!(!identity.same_source(&foreign));
    let doc = &identity.document;
    let visible =
        paintcraft_core::types::Rect::from_origin_size(0, 0, doc.canvas.0, doc.canvas.1).unwrap();
    let view = crate::app::filter_editing::render::View::new(doc, visible, 0).unwrap();
    identity.validate_view(view).unwrap();
    let mut altered = view;
    altered.size[0] += 1;
    assert!(identity.validate_view(altered).is_err());
    let expected = capture::Expected8 {
        token: 7,
        view,
        calls: 12,
    };
    assert_eq!(
        (expected.token, expected.view, expected.calls),
        (7, view, 12)
    );
}

#[test]
fn wider_original_preparation_ceiling_tightens_without_replacing_counter_or_metadata() {
    let original_budget = SharedBudget::new(64);
    let original = paintcraft_render::spatial::WorkspaceAdmission::new(
        original_budget.scope(AllocationClass::SharedBudget),
        1,
        operation::WORK + 100,
    );
    original.reserve_work(operation::WORK - 3).unwrap();
    let captured = paintcraft_render::spatial::with_workspace(original.clone(), || {
        operation::Parent8::capture()
    })
    .unwrap();
    let (metadata, factory, released) = factory(1 << 20);
    let operation = operation::Operation8::preparation(captured, factory).unwrap();
    operation.spend(3).unwrap();
    assert_eq!(original.work_used(), operation::WORK);
    assert_eq!(original.work_limit(), operation::WORK + 100);
    assert_eq!(
        operation.spend(1).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(original.work_used(), operation::WORK);
    assert_eq!(
        operation.spend(1).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(original.work_used(), operation::WORK);
    assert_eq!(metadata.usage().reserved, 1 << 20);
    drop(operation);
    assert_eq!(released.get(), 1);
    assert_eq!(metadata.usage().reserved, 0);
}

#[test]
fn joined_handoff_blocks_every_actual_egui_pass_until_a_later_frame() {
    let ctx = eframe::egui::Context::default();
    ctx.options_mut(|o| o.max_passes = 2.try_into().unwrap());
    let mut retired = None;
    let mut passes = Vec::new();
    let mut output = ctx.run_ui(Default::default(), |ui| {
        let frame = ui.ctx().cumulative_frame_nr();
        let pass = ui.ctx().cumulative_pass_nr();
        if retired.is_none() {
            retired = Some(frame);
            ui.ctx()
                .request_discard("actual handoff multipass regression");
        }
        passes.push((frame, pass, super::handoff_pending(retired, ui.ctx())));
    });
    output.textures_delta.clear();
    assert_eq!(passes.len(), 2);
    assert_eq!(passes[0].0, passes[1].0);
    assert!(passes[1].1 > passes[0].1);
    assert!(passes.iter().all(|(_, _, blocked)| *blocked));
    let mut next = None;
    let mut output = ctx.run_ui(Default::default(), |ui| {
        next = Some((
            ui.ctx().cumulative_frame_nr(),
            super::handoff_pending(retired, ui.ctx()),
        ));
    });
    output.textures_delta.clear();
    let (frame, blocked) = next.unwrap();
    assert!(frame > retired.unwrap());
    assert!(!blocked); // Only scheduling eligibility; actual broker may still deny.
}
