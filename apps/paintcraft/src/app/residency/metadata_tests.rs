use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
fn context() -> (egui::Context, Arc<AtomicUsize>) {
    let ctx = egui::Context::default();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    ctx.set_request_repaint_callback(move |_| {
        seen.fetch_add(1, Ordering::AcqRel);
    });
    (ctx, count)
}
fn advance(ctx: &egui::Context) {
    for _ in 0..3 {
        let mut output = ctx.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();
    }
}
#[test]
fn metadata_releases_wake_distinct_contexts_without_executing_edits_or_advertising_tile_demand() {
    let tiles = SharedBudget::new(4 << 20);
    let metadata = SharedBudget::new(512);
    let (a, seen_a) = context();
    let (b, seen_b) = context();
    let mut first = App::new(None);
    let mut second = App::new(None);
    first
        .connect_budget_wakes_with_metadata(&a, tiles.clone(), metadata.clone())
        .unwrap();
    second
        .connect_budget_wakes_with_metadata(&b, tiles, metadata.clone())
        .unwrap();
    let before_a = first.editor.render_key();
    let before_b = second.editor.render_key();
    advance(&a);
    advance(&b);
    let held = metadata.reserve(512).unwrap();
    first.poll_residency_pressure(&a);
    second.poll_residency_pressure(&b);
    advance(&a);
    advance(&b);
    let count_a = seen_a.load(Ordering::Acquire);
    let count_b = seen_b.load(Ordering::Acquire);
    drop(held);
    assert!(seen_a.load(Ordering::Acquire) > count_a);
    assert!(seen_b.load(Ordering::Acquire) > count_b);
    assert_eq!(first.editor.render_key(), before_a);
    assert_eq!(second.editor.render_key(), before_b);
    assert_eq!(
        first
            .budget_wakes
            .as_ref()
            .unwrap()
            .tiles
            .observe_pressure()
            .requested_capacity,
        0
    );
}
#[test]
fn metadata_render_denial_waits_in_its_own_realm_and_invalidates_only_the_current_frame() {
    let metadata = SharedBudget::new(512);
    let tiles = SharedBudget::new(4 << 20);
    let (ctx, count) = context();
    let mut app = App::new(None);
    app.connect_budget_wakes_with_metadata(&ctx, tiles, metadata.clone())
        .unwrap();
    let held = metadata.reserve(512).unwrap();
    let mut account = metadata.account(AllocationClass::PaintMetadata);
    let error = account.resize(16).unwrap_err();
    let key = app.presentation_key();
    let ticket = app.editor.render_key();
    app.install_render(&ctx, key, Err(error));
    app.poll_residency_pressure(&ctx);
    assert!(app.render_residency_retry.is_some());
    let wakes = app.budget_wakes.as_ref().unwrap();
    assert_eq!(wakes.tiles.observe_pressure().requested_capacity, 0);
    assert_eq!(wakes.metadata.observe_pressure().requested_capacity, 16);
    advance(&ctx);
    let before = count.load(Ordering::Acquire);
    drop(held);
    assert!(count.load(Ordering::Acquire) > before);
    app.retry_reclaimed_render();
    assert!(app.texture_key.is_none());
    assert!(app.render_residency_retry.is_none());
    assert_eq!(app.editor.render_key(), ticket);
}
#[test]
fn accepted_metadata_wait_leaves_history_resident_and_clears_interest_on_escape() {
    let metadata = SharedBudget::new(512);
    let tiles = SharedBudget::new(4 << 20);
    let (ctx, _) = context();
    let mut app = App::new(None);
    app.editor = Editor::new_document(8, 8).unwrap();
    tests::paint(&mut app.editor, 0);
    let old = tests::version(&app.editor);
    tests::paint(&mut app.editor, 1);
    let view = app.editor.history_view();
    let ticket = app.editor.render_key();
    let service =
        crate::controller::background::Service::with_residency(ctx.clone(), tiles.clone())
            .unwrap()
            .with_metadata(metadata.clone());
    // Keep the usual service constructor path; this field binds the App coordinator.
    app.editor.connect_editing(service.clone());
    app.editing = Some(service.clone());
    app.connect_budget_wakes_with_metadata(&ctx, tiles, metadata.clone())
        .unwrap();
    let held = metadata.reserve(400).unwrap();
    app.editor.pointer_down((2, 1));
    app.editor.pointer_up();
    tests::pump(&mut app, &ctx, |app| app.editor.metadata_wait().is_some());
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
    assert_ne!(
        app.budget_wakes
            .as_ref()
            .unwrap()
            .metadata
            .observe_pressure()
            .requested_capacity,
        0
    );
    assert!(!app.editor.has_pending_maintenance());
    assert_ne!(old.resident_bytes(), 0);
    assert_eq!(app.editor.history_view(), view);
    assert_eq!(app.editor.render_key(), ticket);
    app.editor.cancel_pending_edits();
    app.update_capacity_interest();
    assert_eq!(
        app.budget_wakes
            .as_ref()
            .unwrap()
            .metadata
            .observe_pressure()
            .requested_capacity,
        0
    );
    drop(held);
    // Reaping abandoned work does not resurrect the cancelled captured request.
    service.reap_abandoned();
    app.poll_edits(&ctx);
    assert_eq!(app.editor.render_key(), ticket);
}
