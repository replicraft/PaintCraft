use super::*;
use paintcraft_core::types::resources::AllocationClass;
use std::time::Instant;

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

#[test]
fn actual_cross_context_history_reclamation_resumes_an_accepted_edit_in_the_same_tile_realm() {
    let bytes = paintcraft_raster::RasterLayer::TILE_BYTES as u64;
    let tiles = SharedBudget::new(4 * bytes);
    let owner_ctx = egui::Context::default();
    let request_ctx = egui::Context::default();
    let mut owner = App::new(None);
    let mut requester = App::new(None);
    owner.editor = Editor::new_document(257, 1).unwrap();
    requester.editor = Editor::new_document(257, 1).unwrap();
    for (app, ctx) in [(&mut owner, &owner_ctx), (&mut requester, &request_ctx)] {
        let service =
            crate::controller::background::Service::with_residency(ctx.clone(), tiles.clone())
                .unwrap();
        app.editor.connect_editing(service.clone());
        app.editing = Some(service);
        app.connect_budget_wakes(ctx, tiles.clone()).unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    for x in 0..3 {
        owner.editor.pointer_down((x, 0));
        owner.editor.pointer_up();
        while owner.editor.has_pending_edits() {
            assert!(Instant::now() < deadline);
            owner.editor.run_pending();
            std::thread::yield_now();
        }
    }
    assert_eq!(tiles.usage().reserved, 3 * bytes);
    let history = owner.editor.history_view();
    let ticket = owner.editor.render_key();
    requester.editor.pointer_down((0, 0));
    requester.editor.pointer_move((256, 0));
    requester.editor.pointer_up();
    while requester.editor.allocation_wait().is_none() {
        assert!(Instant::now() < deadline);
        requester.editor.run_pending();
        std::thread::yield_now();
    }
    requester.update_capacity_interest();
    assert_eq!(
        owner
            .budget_wakes
            .as_ref()
            .unwrap()
            .tiles
            .observe_pressure()
            .requested_capacity,
        2 * bytes
    );
    while requester.editor.has_pending_edits() {
        assert!(Instant::now() < deadline);
        owner.poll_edits(&owner_ctx);
        requester.poll_edits(&request_ctx);
        assert!(tiles.usage().reserved <= 4 * bytes);
        std::thread::yield_now();
    }
    assert_eq!(requester.editor.publications(), 1);
    assert_eq!(owner.editor.history_view(), history);
    assert_eq!(owner.editor.render_key(), ticket);
    assert_eq!(tiles.usage().reserved, 3 * bytes);
    let raster = &requester.editor.document().layers[&requester.editor.target().unwrap()].raster;
    for x in 0..257 {
        assert_eq!(raster.get(x, 0), Rgba([0, 0, 0, 255]));
    }
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
    drop((owner, requester));
    assert_eq!(tiles.usage().reserved, 0);
}
