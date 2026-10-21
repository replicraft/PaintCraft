use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
fn counted_context() -> (egui::Context, Arc<AtomicUsize>) {
    let ctx = egui::Context::default();
    let count = Arc::new(AtomicUsize::new(0));
    let wake = count.clone();
    ctx.set_request_repaint_callback(move |_| {
        wake.fetch_add(1, Ordering::AcqRel);
    });
    (ctx, count)
}
fn advance(ctx: &egui::Context) {
    // Retire egui's pending immediate repaint before observing a new wake.
    for _ in 0..3 {
        let mut output = ctx.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();
    }
}
#[test]
fn shared_budget_pressure_wakes_distinct_app_contexts_without_executing_edits() {
    let tiles = SharedBudget::new(4 << 20);
    let workers = SharedBudget::new(3 << 30);
    let (first_ctx, first_count) = counted_context();
    let (second_ctx, second_count) = counted_context();
    let mut first = App::new(None);
    let mut second = App::new(None);
    first.worker_budget = workers.clone();
    second.worker_budget = workers;
    first
        .connect_budget_wakes(&first_ctx, tiles.clone())
        .unwrap();
    second
        .connect_budget_wakes(&second_ctx, tiles.clone())
        .unwrap();
    let a = first.editor.render_key();
    let b = second.editor.render_key();
    advance(&first_ctx);
    advance(&second_ctx);
    let before_a = first_count.load(Ordering::Acquire);
    let before_b = second_count.load(Ordering::Acquire);
    let held = std::thread::spawn(move || tiles.reserve(4 << 20).unwrap())
        .join()
        .unwrap();
    assert!(first_count.load(Ordering::Acquire) > before_a);
    assert!(second_count.load(Ordering::Acquire) > before_b);
    assert_eq!(first.editor.render_key(), a);
    assert_eq!(second.editor.render_key(), b);
    first.poll_residency_pressure(&first_ctx);
    second.poll_residency_pressure(&second_ctx);
    advance(&first_ctx);
    advance(&second_ctx);
    let before_a = first_count.load(Ordering::Acquire);
    let before_b = second_count.load(Ordering::Acquire);
    drop(held);
    assert!(first_count.load(Ordering::Acquire) > before_a);
    assert!(second_count.load(Ordering::Acquire) > before_b);
    assert_eq!(first.editor.render_key(), a);
    assert_eq!(second.editor.render_key(), b);
}
#[test]
fn an_unread_worker_result_keeps_other_contexts_blocked_until_its_charge_returns() {
    use std::sync::mpsc;
    let workers = SharedBudget::new(3 << 30);
    let (ctx, count) = counted_context();
    let mut app = App::new(None);
    app.worker_budget = workers.clone();
    app.connect_budget_wakes(&ctx, SharedBudget::new(4 << 20))
        .unwrap();
    let mut executor = Executor::<u8>::with_budget(
        WorkerLimits {
            workers: 1,
            pending_jobs: 1,
            transient_bytes: 3 << 30,
        },
        workers,
    )
    .unwrap();
    let (done, wait_done) = mpsc::channel();
    let handle = executor
        .submit(
            2 << 30,
            |_| Ok(1),
            move || {
                done.send(()).unwrap();
            },
        )
        .unwrap();
    wait_done.recv_timeout(Duration::from_secs(5)).unwrap();
    app.poll_residency_pressure(&ctx);
    advance(&ctx);
    let before = count.load(Ordering::Acquire);
    assert_eq!(executor.poll_for(handle).unwrap().unwrap(), 1);
    assert!(count.load(Ordering::Acquire) > before);
    assert_eq!(app.editor.publications(), 0);
}
#[test]
fn waiting_resource_failure_receives_releases_above_the_pressure_threshold() {
    let tiles = SharedBudget::new(4 << 20);
    let (ctx, count) = counted_context();
    let mut app = App::new(None);
    app.connect_budget_wakes(&ctx, tiles.clone()).unwrap();
    let held = tiles.reserve(1 << 20).unwrap();
    let error = tiles
        .reserve(4 << 20)
        .unwrap_err()
        .with_allocation_requirement(
            paintcraft_core::types::resources::AllocationClass::DecodedTiles,
            4 << 20,
        );
    app.install_render(&ctx, app.presentation_key(), Err(error));
    app.poll_residency_pressure(&ctx);
    advance(&ctx);
    let before = count.load(Ordering::Acquire);
    drop(held);
    assert!(count.load(Ordering::Acquire) > before);
}
