use super::*;
use paintcraft_core::types::resources::AllocationClass;

#[test]
fn another_contexts_demand_reclaims_history_above_static_headroom_without_switching() {
    let tiles = SharedBudget::new(4 << 20);
    let blocker = tiles.reserve(1 << 20).unwrap();
    let request_ctx = egui::Context::default();
    let owner_ctx = egui::Context::default();
    let mut requester = App::new(None);
    let mut owner = App::new(None);
    requester
        .connect_budget_wakes(&request_ctx, tiles.clone())
        .unwrap();
    owner
        .connect_budget_wakes(&owner_ctx, tiles.clone())
        .unwrap();
    owner.editor = Editor::new_document(8, 8).unwrap();
    tests::paint(&mut owner.editor, 0);
    let old = tests::version(&owner.editor);
    tests::paint(&mut owner.editor, 1);
    tests::paint(&mut owner.editor, 2);
    let history = owner.editor.history_view();
    let key = owner.editor.render_key();
    let active = owner.active_tab_id();
    owner.initialize_editing(owner_ctx.clone());
    owner.poll_residency_pressure(&owner_ctx);
    assert!(!owner.editor.has_pending_maintenance()); // 3 MiB free exceeds 1 MiB baseline.
    let error = tiles
        .reserve(4 << 20)
        .unwrap_err()
        .with_allocation_requirement(AllocationClass::DecodedTiles, 4 << 20);
    requester.install_render(&request_ctx, requester.presentation_key(), Err(error));
    owner.poll_residency_pressure(&owner_ctx);
    assert!(owner.editor.has_pending_maintenance());
    // Telemetry is scaled; the history worker, publication and reader release are real.
    tests::pump(&mut owner, &owner_ctx, |_| old.resident_bytes() == 0);
    assert_eq!(owner.active_tab_id(), active);
    assert_eq!(owner.editor.render_key(), key);
    assert_eq!(owner.editor.history_view(), history);
    let pixels = render_flat(requester.editor.document()).unwrap();
    requester.install_render(&request_ctx, requester.presentation_key(), Ok(pixels));
    assert_eq!(
        owner
            .budget_wakes
            .as_ref()
            .unwrap()
            .tiles
            .observe_pressure()
            .requested_capacity,
        0
    );
    drop(blocker);
}

#[test]
fn changed_document_or_foreign_allocation_realm_cannot_leave_shared_pressure_interest() {
    let tiles = SharedBudget::new(4 << 20);
    let blocker = tiles.reserve(1 << 20).unwrap();
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.connect_budget_wakes(&ctx, tiles.clone()).unwrap();
    let error = tiles
        .reserve(4 << 20)
        .unwrap_err()
        .with_allocation_requirement(AllocationClass::DecodedTiles, 4 << 20);
    app.install_render(&ctx, app.presentation_key(), Err(error));
    assert_eq!(
        app.budget_wakes
            .as_ref()
            .unwrap()
            .tiles
            .observe_pressure()
            .requested_capacity,
        4 << 20
    );
    app.new_document();
    app.poll_residency_pressure(&ctx);
    assert_eq!(
        app.budget_wakes
            .as_ref()
            .unwrap()
            .tiles
            .observe_pressure()
            .requested_capacity,
        0
    );
    let foreign = SharedBudget::new(4 << 20);
    let held = foreign.reserve(1 << 20).unwrap();
    let error = foreign
        .reserve(4 << 20)
        .unwrap_err()
        .with_allocation_requirement(AllocationClass::DecodedTiles, 4 << 20);
    app.install_render(&ctx, app.presentation_key(), Err(error));
    assert_eq!(
        app.budget_wakes
            .as_ref()
            .unwrap()
            .tiles
            .observe_pressure()
            .requested_capacity,
        0
    );
    drop((blocker, held));
}
