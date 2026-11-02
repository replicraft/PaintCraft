use super::*;
use paintcraft_document::{POLICY, new_document};
use std::time::Instant;

fn document() -> Arc<paintcraft_document::Document> {
    Arc::new(new_document(1000, 486, Some(Rgba([25, 50, 75, 255])), 0, POLICY).unwrap())
}

fn drain(
    service: &mut Thumbnail,
    ctx: &egui::Context,
    doc: Option<Arc<paintcraft_document::Document>>,
    budget: &paintcraft_runtime::SharedBudget,
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while service.pending.is_some() {
        service.update(ctx, doc.clone(), budget, false);
        assert!(Instant::now() < deadline, "thumbnail worker timed out");
        std::thread::yield_now();
    }
}

#[test]
fn full_document_thumbnail_is_bounded_revision_checked_and_releases_when_hidden() {
    let ctx = egui::Context::default();
    let budget = paintcraft_runtime::SharedBudget::new(32 << 20);
    let doc = document();
    let mut service = Thumbnail::default();
    // Interactive editing defers background admission entirely.
    service.update(&ctx, Some(doc.clone()), &budget, false);
    assert!(service.pending.is_none());
    assert_eq!(budget.usage().reserved, 0);
    service.update(&ctx, Some(doc.clone()), &budget, true);
    drain(&mut service, &ctx, Some(doc.clone()), &budget);
    assert!(matches!(service.preview(), ImagePreview::Ready { .. }));
    let image = service.image.as_ref().unwrap();
    assert_eq!(image.texture.size(), [250, 122]);
    assert_eq!(image.aspect, 1000.0 / 486.0);
    assert_eq!(image.uv.max.x, 1.0);
    assert_eq!(image.uv.max.y, 486.0 / 488.0);
    assert_eq!(budget.usage().reserved, 250 * 122 * 12);
    let mut newer = (*doc).clone();
    newer.state_id = Uuid::new_v4();
    service.update(&ctx, Some(Arc::new(newer)), &budget, false);
    assert!(service.image.is_none());
    assert_eq!(budget.usage().reserved, 0);
    service.update(&ctx, None, &budget, false);
    assert!(service.scope.is_none());
}

#[test]
fn hidden_or_stale_work_cannot_publish_and_releases_only_after_completion() {
    for hidden in [true, false] {
        let ctx = egui::Context::default();
        let budget = paintcraft_runtime::SharedBudget::new(32 << 20);
        let doc = document();
        let mut service = Thumbnail {
            scope: Some(Scope::committed(&doc)),
            ..Default::default()
        };
        let mut executor = Executor::with_budget(
            WorkerLimits {
                workers: 1,
                pending_jobs: 1,
                transient_bytes: SOURCE_LIMIT,
            },
            budget.clone(),
        )
        .unwrap();
        let (release, gate) = std::sync::mpsc::channel();
        let (started, wait_started) = std::sync::mpsc::sync_channel(1);
        let source = doc.clone();
        let handle = executor
            .submit(
                WORK_BYTES,
                move |_| {
                    started.send(()).unwrap();
                    gate.recv_timeout(Duration::from_secs(5)).unwrap();
                    paintcraft_render::thumbnail::render_document(&source, SIDE, || Ok(()))
                },
                || {},
            )
            .unwrap();
        service.pending = Some((Scope::committed(&doc), handle));
        wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
        service.executor = Some(executor);
        let mut next = (*doc).clone();
        // A different document can share a state identity; document identity is mandatory.
        next.document_id = Uuid::new_v4();
        let next = (!hidden).then(|| Arc::new(next));
        service.update(&ctx, next.clone(), &budget, false);
        assert_eq!(budget.usage().reserved, WORK_BYTES);
        release.send(()).unwrap();
        drain(&mut service, &ctx, next, &budget);
        assert!(service.image.is_none());
        assert!(!service.failed);
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn denied_worker_and_texture_admission_do_not_leak_or_publish() {
    let ctx = egui::Context::default();
    let budget = paintcraft_runtime::SharedBudget::new(1);
    let doc = document();
    let mut service = Thumbnail::default();
    service.update(&ctx, Some(doc.clone()), &budget, true);
    assert!(service.pending.is_none());
    assert!(service.image.is_none());
    assert_eq!(budget.usage().reserved, 0);
    // Output fits, but the combined worker/output reservation does not.
    let budget = paintcraft_runtime::SharedBudget::new(WORK_BYTES + 250 * 122 * 12 - 1);
    let mut service = Thumbnail::default();
    service.update(&ctx, Some(doc), &budget, true);
    assert!(service.pending.is_none());
    assert!(service.output.is_none());
    assert!(service.image.is_none());
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn reopening_same_source_during_cancellation_remains_eligible() {
    let ctx = egui::Context::default();
    let budget = paintcraft_runtime::SharedBudget::new(32 << 20);
    let doc = document();
    let mut executor = Executor::with_budget(
        WorkerLimits {
            workers: 1,
            pending_jobs: 1,
            transient_bytes: SOURCE_LIMIT,
        },
        budget.clone(),
    )
    .unwrap();
    let (release, gate) = std::sync::mpsc::channel();
    let (started, wait_started) = std::sync::mpsc::sync_channel(1);
    let source = doc.clone();
    let handle = executor
        .submit(
            WORK_BYTES,
            move |_| {
                started.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
                paintcraft_render::thumbnail::render_document(&source, SIDE, || Ok(()))
            },
            || {},
        )
        .unwrap();
    // Cancellation must happen after the closure starts; cancelling a queued
    // job legitimately drops its gate before it is ever called.
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    let mut service = Thumbnail {
        scope: Some(Scope::committed(&doc)),
        executor: Some(executor),
        pending: Some((Scope::committed(&doc), handle)),
        output: Some(budget.reserve(250 * 122 * 12).unwrap()),
        ..Default::default()
    };
    service.update(&ctx, None, &budget, false);
    service.update(&ctx, Some(doc.clone()), &budget, false);
    assert!(
        service.pending.is_some(),
        "cancelled work is still gated while reopening"
    );
    release.send(()).unwrap();
    drain(&mut service, &ctx, Some(doc.clone()), &budget);
    assert!(!service.failed);
    assert!(service.image.is_none());
    assert!(service.output.is_none());
    service.update(&ctx, Some(doc.clone()), &budget, true);
    drain(&mut service, &ctx, Some(doc), &budget);
    assert!(matches!(service.preview(), ImagePreview::Ready { .. }));
}

#[test]
fn actual_document_thumbnail_job_composes_blend_mask_box_noise_and_releases_owners() {
    use crate::files::blend_adoption_tests::{expected, source as blend_source};
    use paintcraft_core::types::LayerBlendMode;
    let ctx = egui::Context::default();
    for mode in [LayerBlendMode::Multiply, LayerBlendMode::Screen] {
        let doc = blend_source(mode, 2, 1, true);
        let editor = controller::Editor::with_document(doc.clone(), None);
        let before = editor.document_snapshot();
        let ticket = editor.history_view().ticket;
        let capture = Source::capture(&editor);
        assert!(capture.requires_spatial_workspace());
        let budget = paintcraft_runtime::SharedBudget::new(128 << 20);
        let mut service = Thumbnail::default();
        service.update_source(&ctx, Some(capture), &budget, true);
        let (_, handle) = service
            .pending
            .expect("real admitted document-thumbnail job");
        let deadline = Instant::now() + Duration::from_secs(20);
        let frame = loop {
            if let Some(result) = service.executor.as_mut().unwrap().poll_for(handle) {
                break result.unwrap();
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        };
        service.pending = None;
        assert_eq!(frame.request.size(), [2, 1]);
        assert_eq!(frame.pixels, expected(mode, true).repeat(2));
        assert!(Arc::ptr_eq(&before, &editor.document_snapshot()));
        assert_eq!(editor.history_view().ticket, ticket);
        assert!(editor.document().same_semantics(&doc));
        drop(frame);
        drop(service);
        assert_eq!(budget.usage().reserved, 0);
    }
}
