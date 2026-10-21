use super::*;
use admission::{ACTIVE, submit};
use paintcraft_core::types::resources::{ResourceLease, SharedBudget};

pub(super) fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
        Ok(adapter) => adapter,
        Err(error) => {
            assert!(
                std::env::var_os("PAINTCRAFT_REQUIRE_GPU").is_none(),
                "GPU required: {error}"
            );
            return None;
        }
    };
    eprintln!("admission adapter: {:?}", adapter.get_info());
    Some(pollster::block_on(adapter.request_device(&Default::default())).unwrap())
}
#[test]
fn failed_wait_keeps_nested_charges_until_real_queue_completion_is_observed() {
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
    let outer = SharedBudget::new(64);
    let inner = SharedBudget::new(32);
    let epoch = DeviceEpoch::default();
    let result = with_admission(ResourceLease::new(outer.reserve(64).unwrap()), || {
        with_admission(ResourceLease::new(inner.reserve(32).unwrap()), || {
            let mut submitted = submit(
                &queue,
                &std::sync::Arc::new(()),
                device.create_command_encoder(&Default::default()),
            );
            submitted.index = Some(invalid);
            submitted.wait(&device, &epoch)
        })
    });
    assert_eq!(result.unwrap_err().code, ErrorCode::CapabilityUnsupported);
    assert!(!epoch.is_current());
    assert!(ACTIVE.with(|active| active.borrow().is_none()));
    assert_eq!(outer.usage().reserved, 64);
    assert_eq!(inner.usage().reserved, 32);
    // Observation is allowed after quarantine; no new PaintCraft work is submitted.
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(5)),
        })
        .unwrap();
    assert_eq!(outer.usage().reserved, 0);
    assert_eq!(inner.usage().reserved, 0);
}
#[test]
fn unwind_restores_scope_but_retains_submitted_resources_until_completion() {
    let Some((device, queue)) = device() else {
        return;
    };
    let budget = SharedBudget::new(64);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_admission(
            ResourceLease::new(budget.reserve(64).unwrap()),
            || -> Result<()> {
                let _submitted = submit(
                    &queue,
                    &std::sync::Arc::new(()),
                    device.create_command_encoder(&Default::default()),
                );
                panic!("unwind after submission");
            },
        )
    }));
    assert!(result.is_err());
    assert!(ACTIVE.with(|active| active.borrow().is_none()));
    assert_eq!(budget.usage().reserved, 64);
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(5)),
        })
        .unwrap();
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn successful_render_and_readback_release_scope_without_pinning_returned_frames() {
    let Some((device, queue)) = device() else {
        return;
    };
    let gpu = GpuRenderer::new(device, queue).unwrap();
    let budget = SharedBudget::new(64);
    let doc = paintcraft_document::new_document(
        7,
        5,
        Some(paintcraft_core::types::Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let frame = with_admission(ResourceLease::new(budget.reserve(64).unwrap()), || {
        gpu.render_texture(&doc, || Ok(()))
    })
    .unwrap();
    assert_eq!(budget.usage().reserved, 0);
    let pixels = with_admission(ResourceLease::new(budget.reserve(64).unwrap()), || {
        frame.readback(|| Ok(()))
    })
    .unwrap();
    assert_eq!(pixels, vec![255; 7 * 5 * 4]);
    assert_eq!(budget.usage().reserved, 0);
    let error = with_admission(ResourceLease::new(budget.reserve(64).unwrap()), || {
        Err::<(), _>(Error::new(ErrorCode::OperationCancelled, "before submit"))
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn ending_the_queue_context_does_not_leave_a_permanent_admission_cycle() {
    let Some((device, queue)) = device() else {
        return;
    };
    let budget = SharedBudget::new(64);
    with_admission(ResourceLease::new(budget.reserve(64).unwrap()), || {
        let _unobserved = submit(
            &queue,
            &std::sync::Arc::new(()),
            device.create_command_encoder(&Default::default()),
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.usage().reserved, 64);
    drop(queue);
    drop(device);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn excessive_scope_nesting_rejects_without_leaking_or_poisoning_the_next_request() {
    fn nested(budget: &SharedBudget, remaining: usize) -> Result<()> {
        with_admission(ResourceLease::new(budget.reserve(1)?), || {
            if remaining == 0 {
                Ok(())
            } else {
                nested(budget, remaining - 1)
            }
        })
    }
    let budget = SharedBudget::new(32);
    assert_eq!(
        nested(&budget, 16).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(budget.usage().reserved, 0);
    assert!(ACTIVE.with(|active| active.borrow().is_none()));
    nested(&budget, 15).unwrap();
    assert_eq!(budget.usage().reserved, 0);
}
