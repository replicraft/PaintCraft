//! Causal original-factory and real epaint payload tests, not JS/opaque ABI mocks.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn all_three_data_slots_retire_in_distinct_original_account_phases() {
    let (budget, factory, released) = factory(1 << 20);
    let baseline = factory.scope().footprint().live;
    let counter =
        paintcraft_render::spatial::WorkspaceAdmission::control_allocation_quote().unwrap();
    let controls = controls::frame_bytes().unwrap();
    let stage = super::stage(factory.clone(), &[20, 60, 100, 64, 7, 9, 11, 0], 2);
    let funding = stage.funding();
    let d = 8;
    assert_eq!(
        factory.scope().footprint().live,
        baseline + counter + controls + 3 * d + d
    );
    let mut publication = stage.into_publication().unwrap();
    assert_eq!(
        factory.scope().footprint().live,
        baseline + counter + controls + 2 * d + d
    );
    assert_eq!(publication.image().pixels[0].to_array(), [5, 15, 25, 64]);
    assert_eq!(publication.image().pixels[1].to_array(), [0, 0, 0, 0]);
    funding.js_retired().unwrap();
    assert_eq!(
        factory.scope().footprint().live,
        baseline + counter + controls + d
    );
    assert!(publication.poll_retired().unwrap());
    assert_eq!(
        factory.scope().footprint().live,
        baseline + counter + controls
    );
    assert_eq!(released.get(), 0);
    drop(publication);
    drop(funding);
    assert_eq!(factory.scope().footprint().live, baseline);
    assert_eq!(budget.usage().reserved, 1 << 20);
    drop(factory);
    assert_eq!(released.get(), 1);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn actual_old_cpu_delta_blocks_distinct_view_on_same_original_until_data_retires() {
    let frame_controls = controls::frame_bytes().unwrap();
    let counter =
        paintcraft_render::spatial::WorkspaceAdmission::control_allocation_quote().unwrap();
    // Source-derived prospective boundary BEFORE creating the original scope.
    // No usage/footprint observation decides this grant.
    let cap = controls::factory_bytes().unwrap() + counter + frame_controls + 65536;
    let (budget, factory, released) = factory(cap);
    let pixels = vec![7; 16384];
    let mut stage = frame::FrameStage8::admit(
        operation::Operation8::view(factory.clone()).unwrap(),
        [2048, 2],
    )
    .unwrap();
    stage.fill_test(&pixels).unwrap();
    let funding = stage.funding();
    funding.js_retired().unwrap();
    let mut publication = stage.into_publication().unwrap();
    let ctx = eframe::egui::Context::default();
    let texture = publication.install(&ctx);
    let mut delta = ctx.tex_manager().write().take_delta();
    assert!(!publication.poll_retired().unwrap());
    let before = factory.scope().footprint().live;
    let refused = frame::FrameStage8::admit(
        operation::Operation8::view(factory.clone()).unwrap(),
        [2048, 2],
    );
    assert_eq!(refused.unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(factory.scope().footprint().live, before);
    assert_eq!(factory.cap(), cap);
    assert_eq!(budget.usage().reserved, cap);
    assert_eq!(released.get(), 0);
    delta.clear();
    assert!(publication.poll_retired().unwrap());
    drop(publication);
    drop(funding);
    let mut next = frame::FrameStage8::admit(
        operation::Operation8::view(factory.clone()).unwrap(),
        [2048, 2],
    )
    .unwrap();
    next.fill_test(&pixels).unwrap();
    let next_funding = next.funding();
    next_funding.js_retired().unwrap();
    drop(next);
    drop(next_funding);
    drop(texture);
    clear(&ctx);
    drop(factory);
    assert_eq!(released.get(), 1);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn retirement_slot_refuses_second_owner_without_discarding_actual_first_delta() {
    let (budget, factory, released) = factory(1 << 20);
    let stage = super::stage(factory.clone(), &[20, 60, 100, 64], 1);
    let funding = stage.funding();
    funding.js_retired().unwrap();
    let publication = stage.into_publication().unwrap();
    let ctx = eframe::egui::Context::default();
    let texture = publication.install(&ctx);
    let mut delta = ctx.tex_manager().write().take_delta();
    let mut retirement = publication::Retirement8::default();
    retirement.admit(publication).unwrap();
    assert!(retirement.is_pending());
    assert!(!retirement.poll(&ctx).unwrap());
    let before = factory.scope().footprint().live;
    let second = super::stage(factory.clone(), &[1, 2, 3, 255], 1);
    second.funding().js_retired().unwrap();
    assert_eq!(
        retirement
            .admit(second.into_publication().unwrap())
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(retirement.is_pending());
    assert_eq!(factory.scope().footprint().live, before);
    assert!(!retirement.poll(&ctx).unwrap());
    delta.clear();
    assert!(retirement.poll(&ctx).unwrap());
    assert!(!retirement.is_pending());
    assert!(ctx.tex_manager().read().meta(texture.id()).is_some());
    drop(funding);
    drop(factory);
    assert_eq!(released.get(), 1);
    assert_eq!(budget.usage().reserved, 0);
    drop(texture);
    clear(&ctx);
}
#[test]
fn unjoined_publication_drop_keeps_actual_pixels_with_full_quarantined_funding() {
    let (budget, factory, released) = factory(1 << 20);
    let stage = super::stage(factory.clone(), &[20, 60, 100, 64], 1);
    stage.funding().js_retired().unwrap();
    let publication = stage.into_publication().unwrap();
    let weak = Arc::downgrade(publication.image());
    let before = factory.scope().footprint().live;
    drop(publication);
    assert!(factory.is_quarantined());
    assert_eq!(factory.scope().footprint().live, before);
    assert_eq!(
        weak.upgrade().unwrap().pixels[0].to_array(),
        [5, 15, 25, 64]
    );
    assert!(operation::Operation8::view(factory.clone()).is_err());
    drop(weak);
    drop(factory);
    // Intentional bounded quarantine retains the WHOLE pair, never just credit.
    assert_eq!(released.get(), 0);
    assert_eq!(budget.usage().reserved, 1 << 20);
}
struct ReleaseObserver {
    watch: paintcraft_core::types::resources::BudgetWatch,
    credit: Rc<Cell<u32>>,
    zero: Arc<AtomicBool>,
}
thread_local! {
    static RELEASE_OBSERVER: RefCell<Option<ReleaseObserver>> = const { RefCell::new(None) };
}
struct ClearObserver;
impl Drop for ClearObserver {
    fn drop(&mut self) {
        RELEASE_OBSERVER.with(|value| drop(value.borrow_mut().take()));
    }
}
#[test]
fn real_delta_weak_fence_and_reservation_callback_precede_actual_credit_marker() {
    let (budget, factory, released) = factory(1 << 20);
    let stage = super::stage(factory.clone(), &[20, 60, 100, 64], 1);
    let funding = stage.funding();
    funding.js_retired().unwrap();
    let mut publication = stage.into_publication().unwrap();
    let ctx = eframe::egui::Context::default();
    let texture = publication.install(&ctx);
    let mut delta = ctx.tex_manager().write().take_delta();
    let weak = Arc::downgrade(publication.image());
    let origin = std::thread::current().id();
    let armed = Arc::new(AtomicBool::new(false));
    let active = armed.clone();
    let zero = Arc::new(AtomicBool::new(false));
    let watch = budget
        .watch(1, move || {
            if !active.load(Ordering::SeqCst) {
                return;
            }
            assert_eq!(std::thread::current().id(), origin);
            RELEASE_OBSERVER.with(|value| {
                let borrowed = value.borrow();
                let observer = borrowed.as_ref().unwrap();
                let actual = observer.watch.observe(); // Acknowledge every coalesced wake.
                assert_eq!(
                    observer.credit.get(),
                    0,
                    "credit cannot precede original reservation notification"
                );
                if actual.reserved == 0 {
                    observer.zero.store(true, Ordering::SeqCst);
                }
            });
        })
        .unwrap();
    assert_eq!(watch.observe().reserved, 1 << 20);
    watch.wait_for_release(true);
    RELEASE_OBSERVER.with(|value| {
        *value.borrow_mut() = Some(ReleaseObserver {
            watch,
            credit: released.clone(),
            zero: zero.clone(),
        })
    });
    let _clear = ClearObserver;
    armed.store(true, Ordering::SeqCst);
    assert!(!publication.poll_retired().unwrap());
    delta.clear();
    assert!(!publication.poll_retired().unwrap()); // Actual Weak still owns Arc control.
    assert_eq!(released.get(), 0);
    assert!(!zero.load(Ordering::SeqCst));
    drop(weak);
    assert!(publication.poll_retired().unwrap());
    drop(publication);
    drop(funding);
    assert_eq!(released.get(), 0);
    drop(factory);
    assert!(zero.load(Ordering::SeqCst));
    assert_eq!(released.get(), 1);
    assert_eq!(budget.usage().reserved, 0);
    armed.store(false, Ordering::SeqCst);
    drop(texture);
    clear(&ctx);
}
fn app() -> (eframe::egui::Context, crate::app::WebApp) {
    let ctx = eframe::egui::Context::default();
    let mut app = crate::app::WebApp::new(&ctx);
    app.frontend = paintcraft_ui::actions::Frontend::Studio;
    app.editor = crate::editor::Editor::new(2, 1).unwrap();
    app.canvas.filter_view = Some(
        crate::app::filter_editing::render::View::new(
            app.editor.session.document(),
            paintcraft_core::types::Rect::new(0, 0, 2, 1).unwrap(),
            0,
        )
        .unwrap(),
    );
    (ctx, app)
}
#[test]
fn normal_classification_fallback_and_refused_third_attempt_keep_original_base_debt() {
    let (_ctx, app) = app();
    assert_eq!(app.editor.session.document().roots.len(), 1);
    let work_budget = SharedBudget::new(64);
    let base = paintcraft_render::spatial::WorkspaceAdmission::new(
        work_budget.scope(AllocationClass::SharedBudget),
        1,
        200000,
    );
    base.reserve_work(17).unwrap();
    let (metadata, factory, released) = factory(8 << 20);
    let checks = Cell::new(0);
    for at in 1..=2 {
        let key = paintcraft_render::spatial::with_workspace(base.clone(), || {
            capture::Key8::capture(&app)
        })
        .unwrap();
        let prepared = snapshot::Preparation8::new(key, factory.clone(), &|| {
            checks.set(checks.get() + 1);
            Ok(())
        })
        .unwrap();
        assert!(!prepared.blend);
        assert_eq!(
            prepared.finish(&|| Ok(())).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(base.work_used(), 17 + 88128 * at);
        assert_eq!(checks.get(), 2 * at);
    }
    let key =
        paintcraft_render::spatial::with_workspace(base.clone(), || capture::Key8::capture(&app))
            .unwrap();
    assert_eq!(
        snapshot::Preparation8::new(key, factory.clone(), &|| {
            checks.set(checks.get() + 1);
            Ok(())
        })
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(base.work_used(), 176273);
    assert_eq!(checks.get(), 4);
    assert_eq!(metadata.usage().reserved, 8 << 20);
    drop(factory);
    assert_eq!(released.get(), 1);
    assert_eq!(metadata.usage().reserved, 0);
}
#[test]
fn actual_private_draft_commit_undo_and_equal_value_foreign_arc_fence_old_stamp() {
    let (ctx, mut app) = app();
    let identity = capture::Key8::capture(&app).unwrap().identity;
    assert!(capture::CurrentStamp8::read(&app).matches(&identity));
    let mut foreign = identity.clone();
    foreign.document = Arc::new((*identity.document).clone());
    assert!(!capture::CurrentStamp8::read(&app).matches(&foreign));
    app.begin_studio_filter(
        &ctx,
        paintcraft_studio::filters::Action::Add(paintcraft_studio::filters::Kind::Invert),
    );
    assert!(app.filter_editing.is_some());
    assert_eq!(app.editor.session.ticket(), identity.ticket);
    assert!(Arc::ptr_eq(
        &app.editor.session.snapshot(),
        &identity.document
    ));
    assert!(!capture::CurrentStamp8::read(&app).matches(&identity));
    assert!(capture::Key8::capture(&app).is_err());
    app.cancel_filter_edit();
    assert!(capture::CurrentStamp8::read(&app).matches(&identity));
    let ticket = app.editor.session.ticket();
    let id = app.editor.session.document().roots[0];
    let batch = format!(
        r##"{{"schema":"paintcraft.batch/v1","atomic":true,"policy":"paintcraft.core/default-v1","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":0,"y":0,"width":1,"height":1}},"color":"#010203FF","mode":"replace"}}]}}"##,
        ticket.state_id, ticket.publication_epoch
    );
    app.editor.session.apply_json(batch.as_bytes()).unwrap();
    assert!(!capture::CurrentStamp8::read(&app).matches(&identity));
    app.editor.session.undo().unwrap().unwrap();
    assert_eq!(
        app.editor.session.ticket().state_id,
        identity.ticket.state_id
    );
    assert!(app.editor.session.ticket().publication_epoch > identity.ticket.publication_epoch);
    assert!(!capture::CurrentStamp8::read(&app).matches(&identity));
}

#[test]
fn actual_paired_parent_preserves_noise_debt_and_uses_original_base_for_metadata() {
    use paintcraft_render::noise_work::{
        self, NoiseAdmission, NoiseJobPolicy, NoiseLimits, WorkComponents,
    };
    let (_ctx, app) = app();
    let original = SharedBudget::new(4096);
    let noise = NoiseAdmission::new_root(
        original.scope(AllocationClass::SharedBudget),
        NoiseJobPolicy::BrowserV1,
        NoiseLimits {
            bytes: 4096,
            base_work: 200000,
            noise_work: 1000,
        },
    )
    .unwrap();
    noise
        .reserve_transferred_work(WorkComponents {
            base: 17,
            noise: 11,
        })
        .unwrap();
    let key =
        noise_work::with_noise_workspace(noise.clone(), || capture::Key8::capture(&app)).unwrap();
    assert!(noise_work::capture_current_noise_workspace().is_none());
    let (metadata, factory, released) = factory(8 << 20);
    let prepared = snapshot::Preparation8::new(key, factory.clone(), &|| Ok(())).unwrap();
    assert!(!prepared.blend);
    assert_eq!(
        prepared.finish(&|| Ok(())).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(
        noise.used().unwrap(),
        WorkComponents {
            base: 88145,
            noise: 11
        }
    );
    assert_eq!(metadata.usage().reserved, 8 << 20);
    drop(factory);
    assert_eq!(released.get(), 1);
    assert_eq!(metadata.usage().reserved, 0);
}
