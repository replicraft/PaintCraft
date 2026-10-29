use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
fn poll(pending: &controller::background::MspExportPending) -> Result<worker::Prepared> {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(result) = pending.poll() {
            return result;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn exact_two_stage_denials_precede_their_allocations_and_one_render_is_retained() {
    for stage in 0..3 {
        let budget = paintcraft_runtime::SharedBudget::new(3 << 30);
        let service =
            controller::background::Service::with_budget(egui::Context::default(), budget.clone())
                .unwrap();
        let doc = Arc::new(document([255; 4]));
        let quote = worker::quote(&doc, Version::RowRleV2).unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let renders = count.clone();
        let pending = service
            .submit_msp_export(quote, move |cancel, capacity| {
                let limit = match stage {
                    0 => quote - 1,
                    1 => quote,
                    _ => capacity,
                };
                worker::prepare_with(
                    doc,
                    Version::RowRleV2,
                    quote,
                    limit,
                    cancel,
                    Policy::Refuse,
                    |doc, check| {
                        renders.fetch_add(1, Ordering::SeqCst);
                        paintcraft_render::render_cancellable(doc, check)
                    },
                )
            })
            .unwrap();
        let result = poll(&pending);
        assert_eq!(count.load(Ordering::SeqCst), usize::from(stage != 0));
        if stage < 2 {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
            drop(pending);
            assert_eq!(budget.usage().reserved, 0);
        } else {
            let prepared = result.unwrap();
            let exact = quote + prepared.artifact.plan().additional_bytes() as u64;
            assert_eq!(prepared.account.usage().reserved, exact);
            let local = prepared.account.clone();
            drop(pending);
            assert_eq!(budget.usage().reserved, 2 << 30);
            assert!(
                prepared
                    .preview
                    .as_ref()
                    .unwrap()
                    .pixels
                    .iter()
                    .all(|p| *p == egui::Color32::WHITE)
            );
            drop(prepared);
            assert_eq!(local.usage().reserved, 0);
            assert_eq!(budget.usage().reserved, 0);
        }
    }
}
#[test]
fn opaque_black_white_is_required_without_implicit_alpha_or_color_conversion() {
    let service = controller::background::Service::new(egui::Context::default()).unwrap();
    for pixel in [
        [17, 17, 17, 255],
        [255, 0, 0, 255],
        [255, 255, 255, 128],
        [17, 51, 93, 0],
    ] {
        for version in [Version::UncompressedV1, Version::RowRleV2] {
            let doc = Arc::new(document(pixel));
            let quote = worker::quote(&doc, version).unwrap();
            let pending = service
                .submit_msp_export(quote, move |cancel, capacity| {
                    worker::prepare(doc, version, quote, capacity, cancel, Policy::Refuse)
                })
                .unwrap();
            assert!(poll(&pending).is_err());
            drop(pending);
        }
    }
}
#[test]
fn only_live_work_retries_permanent_capacity_denial_is_final() {
    let budget = paintcraft_runtime::SharedBudget::new(3 << 30);
    let service =
        controller::background::Service::with_budget(egui::Context::default(), budget.clone())
            .unwrap();
    let (started, wait_started) = std::sync::mpsc::channel();
    let (release, wait_release) = std::sync::mpsc::channel();
    let pending = service
        .submit_msp_export(64, move |cancel, _| {
            started.send(()).unwrap();
            wait_release.recv_timeout(Duration::from_secs(5)).unwrap();
            cancel.check()?;
            Err(stale())
        })
        .unwrap();
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(pending);
    assert!(
        service
            .try_submit_msp_export(64, |_, _| unreachable!())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        service
            .try_submit_msp_export(u64::MAX, |_, _| unreachable!())
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    release.send(()).unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while budget.usage().reserved != 0 {
        service.reap_abandoned();
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    let held = budget.reserve(2 << 30).unwrap();
    assert_eq!(
        service
            .try_submit_msp_export(64, |_, _| unreachable!())
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    drop(held);
}
#[test]
fn cancel_during_render_retires_without_an_artifact_and_releases_admission() {
    let budget = paintcraft_runtime::SharedBudget::new(3 << 30);
    let service =
        controller::background::Service::with_budget(egui::Context::default(), budget.clone())
            .unwrap();
    let doc = Arc::new(document([255; 4]));
    let quote = worker::quote(&doc, Version::RowRleV2).unwrap();
    let (started, wait_started) = std::sync::mpsc::channel();
    let (release, wait_release) = std::sync::mpsc::channel();
    let (finished, wait_finished) = std::sync::mpsc::channel();
    let pending = service
        .submit_msp_export(quote, move |cancel, capacity| {
            let result = worker::prepare_with(
                doc,
                Version::RowRleV2,
                quote,
                capacity,
                cancel,
                Policy::Refuse,
                |doc, check| {
                    started.send(()).unwrap();
                    wait_release.recv_timeout(Duration::from_secs(5)).unwrap();
                    paintcraft_render::render_cancellable(doc, check)
                },
            );
            finished.send(result.as_ref().unwrap_err().code).unwrap();
            result
        })
        .unwrap();
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(pending);
    assert_eq!(budget.usage().reserved, 2 << 30);
    release.send(()).unwrap();
    assert_eq!(
        wait_finished.recv_timeout(Duration::from_secs(5)).unwrap(),
        ErrorCode::OperationCancelled
    );
    let until = Instant::now() + Duration::from_secs(5);
    while budget.usage().reserved != 0 {
        service.reap_abandoned();
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
}
