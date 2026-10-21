use super::*;
use admission::submit;
use admission_tests::device;
use paintcraft_core::types::resources::{ResourceLease, SharedBudget};

fn eventually(mut done: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(
            std::time::Instant::now() < deadline,
            "observer did not progress"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn idle_and_successful_requests_need_no_background_polls() {
    let Some((device, queue)) = device() else {
        return;
    };
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let driver = CompletionDriver::new(&gpu).unwrap();
    let budget = SharedBudget::new(64);
    let doc = paintcraft_document::new_document(
        7,
        5,
        Some(paintcraft_core::types::Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let frame = with_admission_progress(
        ResourceLease::new(budget.reserve(64).unwrap()),
        &driver,
        || gpu.render_texture(&doc, || Ok(())),
    )
    .unwrap();
    assert_eq!(frame.readback(|| Ok(())).unwrap(), vec![255; 7 * 5 * 4]);
    std::thread::sleep(Duration::from_millis(32));
    assert_eq!(
        driver.progress(),
        CompletionProgress {
            pending: 0,
            polls: 0,
            closed: false
        }
    );
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn retired_renderer_releases_nested_charges_without_canvas_or_caller_polling() {
    let Some((device, queue)) = device() else {
        return;
    };
    let Some((_foreign, foreign_queue)) = self::device() else {
        return;
    };
    let mut invalid = foreign_queue.submit([]);
    for _ in 0..20 {
        invalid = foreign_queue.submit([]);
    }
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let driver = CompletionDriver::new(&gpu).unwrap();
    let outer = SharedBudget::new(64);
    let inner = SharedBudget::new(32);
    let epoch = gpu.epoch().clone();
    let result = with_admission_progress(
        ResourceLease::new(outer.reserve(64).unwrap()),
        &driver,
        || {
            with_admission(ResourceLease::new(inner.reserve(32).unwrap()), || {
                let mut submitted = submit(
                    &gpu.queue,
                    &gpu.submission_identity,
                    gpu.device.create_command_encoder(&Default::default()),
                );
                submitted.index = Some(invalid);
                submitted.wait(&gpu.device, &epoch)
            })
        },
    );
    assert_eq!(result.unwrap_err().code, ErrorCode::CapabilityUnsupported);
    assert!(!epoch.is_current());
    // Keep the queue context alive so teardown cannot substitute for polling.
    let queue_guard = gpu.queue.clone();
    drop(gpu);
    eventually(|| {
        outer.usage().reserved == 0 && inner.usage().reserved == 0 && driver.progress().pending == 0
    });
    assert!(driver.progress().polls > 0);
    assert!(!driver.progress().closed);
    drop(queue_guard);
}

#[test]
fn unwind_after_dispatch_is_observed_without_caller_polling() {
    let Some((device, queue)) = device() else {
        return;
    };
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let driver = CompletionDriver::new(&gpu).unwrap();
    let budget = SharedBudget::new(64);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_admission_progress(
            ResourceLease::new(budget.reserve(64).unwrap()),
            &driver,
            || -> Result<()> {
                let _submitted = submit(
                    &gpu.queue,
                    &gpu.submission_identity,
                    gpu.device.create_command_encoder(&Default::default()),
                );
                panic!("unwind after observed submission");
            },
        )
    }));
    assert!(result.is_err());
    assert!(admission::ACTIVE.with(|active| active.borrow().is_none()));
    eventually(|| budget.usage().reserved == 0 && driver.progress().pending == 0);
    assert!(driver.progress().polls > 0);
}

#[test]
fn unmatched_queue_keeps_ownership_until_its_actual_completion_is_observed() {
    let Some((device, queue)) = device() else {
        return;
    };
    let Some((other_device, other_queue)) = self::device() else {
        return;
    };
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let driver = CompletionDriver::new(&gpu).unwrap();
    let budget = SharedBudget::new(64);
    with_admission_progress(
        ResourceLease::new(budget.reserve(64).unwrap()),
        &driver,
        || {
            let _submitted = submit(
                &other_queue,
                &std::sync::Arc::new(()),
                other_device.create_command_encoder(&Default::default()),
            );
            Ok(())
        },
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(32));
    assert_eq!(driver.progress().pending, 0);
    assert_eq!(driver.progress().polls, 0);
    assert_eq!(budget.usage().reserved, 64);
    other_device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(5)),
        })
        .unwrap();
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn last_owner_stops_the_worker_without_requiring_device_teardown() {
    let Some((device, queue)) = device() else {
        return;
    };
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let driver = CompletionDriver::new(&gpu).unwrap();
    let weak = driver.weak();
    let clone = driver.clone();
    drop(driver);
    assert!(!clone.progress().closed);
    drop(clone);
    eventually(|| weak.upgrade().is_none());
    // The original device/queue can still be used after the observer exits.
    let submitted = submit(
        &gpu.queue,
        &gpu.submission_identity,
        gpu.device.create_command_encoder(&Default::default()),
    );
    submitted.wait(&gpu.device, &gpu.epoch()).unwrap();
}

#[test]
fn pending_callbacks_do_not_keep_observer_or_queue_context_alive() {
    let Some((device, queue)) = device() else {
        return;
    };
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let driver = CompletionDriver::new(&gpu).unwrap();
    let weak = driver.weak();
    let budget = SharedBudget::new(64);
    with_admission_progress(
        ResourceLease::new(budget.reserve(64).unwrap()),
        &driver,
        || {
            let _submitted = submit(
                &gpu.queue,
                &gpu.submission_identity,
                gpu.device.create_command_encoder(&Default::default()),
            );
            Ok(())
        },
    )
    .unwrap();
    drop(driver);
    drop(gpu);
    eventually(|| weak.upgrade().is_none() && budget.usage().reserved == 0);
}
