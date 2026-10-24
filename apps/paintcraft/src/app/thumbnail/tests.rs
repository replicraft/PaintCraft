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
    assert!(matches!(service.preview(), Preview::Ready { .. }));
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
            scope: Some((doc.document_id, doc.state_id)),
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
        let source = doc.clone();
        let handle = executor
            .submit(
                WORK_BYTES,
                move |_| {
                    gate.recv_timeout(Duration::from_secs(5)).unwrap();
                    paintcraft_render::thumbnail::render_document(&source, SIDE, || Ok(()))
                },
                || {},
            )
            .unwrap();
        service.pending = Some(((doc.document_id, doc.state_id), handle));
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
    let source = doc.clone();
    let handle = executor
        .submit(
            WORK_BYTES,
            move |_| {
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
                paintcraft_render::thumbnail::render_document(&source, SIDE, || Ok(()))
            },
            || {},
        )
        .unwrap();
    let mut service = Thumbnail {
        scope: Some((doc.document_id, doc.state_id)),
        executor: Some(executor),
        pending: Some(((doc.document_id, doc.state_id), handle)),
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
    assert!(matches!(service.preview(), Preview::Ready { .. }));
}
