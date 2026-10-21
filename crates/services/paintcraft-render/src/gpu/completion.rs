//! Uncertain GPU completion quarantines its lifetime before returning an error.
use super::*;

pub(super) fn failed(epoch: &DeviceEpoch, stage: &str, error: impl std::fmt::Display) -> Error {
    epoch.retire();
    unavailable(format!("{stage}: {error}"))
}

pub(super) fn wait(
    device: &wgpu::Device,
    epoch: &DeviceEpoch,
    submission: wgpu::SubmissionIndex,
) -> Result<()> {
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(30)),
        })
        .map_err(|error| failed(epoch, "GPU completion failed", error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_and_mapping_failures_retire_only_the_shared_failed_lifetime() {
        for error in [
            wgpu::PollError::Timeout,
            wgpu::PollError::WrongSubmissionIndex(7, 2),
        ] {
            let epoch = DeviceEpoch::default();
            let observer = epoch.clone();
            let replacement = DeviceEpoch::default();
            let result = failed(&epoch, "test completion", error);
            assert_eq!(result.code, ErrorCode::CapabilityUnsupported);
            assert!(!observer.is_current());
            assert!(replacement.is_current());
            assert!(result.message.contains("test completion"));
        }
        for callback in [
            mpsc::RecvTimeoutError::Timeout,
            mpsc::RecvTimeoutError::Disconnected,
        ] {
            let epoch = DeviceEpoch::default();
            assert_eq!(
                failed(&epoch, "mapping callback", callback).code,
                ErrorCode::CapabilityUnsupported
            );
            assert!(!epoch.is_current());
        }
    }

    #[test]
    fn invalid_real_submission_retires_renderer_and_frames_without_changing_document() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
            Ok(adapter) => adapter,
            Err(error) => {
                assert!(
                    std::env::var_os("PAINTCRAFT_REQUIRE_GPU").is_none(),
                    "GPU required: {error}"
                );
                return;
            }
        };
        eprintln!("completion adapter: {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let epoch = DeviceEpoch::default();
        let lost = epoch.clone();
        device.set_device_lost_callback(move |_, _| lost.retire());
        let gpu = GpuRenderer::with_working_cache_limits(
            device.clone(),
            queue.clone(),
            epoch,
            SourceCacheLimits::default(),
            1 << 20,
        )
        .unwrap();
        let doc = paintcraft_document::new_document(
            7,
            5,
            Some(paintcraft_core::types::Rgba([13, 71, 211, 137])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let pixels = crate::render_flat(&doc).unwrap();
        let frame = gpu.render_texture(&doc, || Ok(())).unwrap();
        assert!(gpu.source_cache_usage().buffers > 0);
        assert!(gpu.output_pool_usage() > 0);
        let submitted = queue.submit([]);
        wait(&device, &gpu.epoch, submitted).unwrap();
        assert!(gpu.epoch.is_current());
        let (_foreign, foreign_queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut foreign_index = foreign_queue.submit([]);
        for _ in 0..20 {
            foreign_index = foreign_queue.submit([]);
        }
        let error = wait(&device, &gpu.epoch, foreign_index).unwrap_err();
        assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
        assert!(!gpu.epoch.is_current());
        assert_eq!(
            gpu.render(&doc).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(gpu.source_cache_usage().buffers, 0);
        assert_eq!(gpu.output_pool_usage(), 0);
        assert_eq!(
            frame.readback(|| Ok(())).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(crate::render_flat(&doc).unwrap(), pixels);
        let (new_device, new_queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let replacement = GpuRenderer::new(new_device, new_queue).unwrap();
        assert_eq!(replacement.render(&doc).unwrap(), pixels);
    }
}
