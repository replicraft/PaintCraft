//! Ordered GPU composition using the same integer policy as the scalar oracle.
//!
//! This first backend reads back straight RGBA8 for the shared display/export
//! boundary. It is a correctness baseline, not a zero-copy or performance claim.
mod epoch;
mod presentation;
mod readback;
mod sources;
pub use epoch::DeviceEpoch;
pub use presentation::GpuTextureFrame;
use sources::SourceCache;
pub use sources::{GpuFrame, GpuRenderStats, SourceCacheLimits, SourceCacheUsage};

use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;
use std::{
    sync::{Mutex, mpsc},
    time::Duration,
};
use wgpu::util::DeviceExt as _;

/// Explicit WGSL `Params` layout; all integer fields are staged as little endian.
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct LayerParams {
    origin: [u32; 2],
    extent: [u32; 2],
    canvas_width: u32,
    opacity: u32,
    padding: [u32; 2],
}
impl LayerParams {
    fn new(origin: [u32; 2], extent: [u32; 2], canvas_width: u32, opacity: u8) -> Self {
        Self {
            origin: origin.map(u32::to_le),
            extent: extent.map(u32::to_le),
            canvas_width: canvas_width.to_le(),
            opacity: u32::from(opacity).to_le(),
            padding: [0; 2],
        }
    }
}

#[derive(Debug)]
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    presentation: Option<wgpu::ComputePipeline>,
    epoch: DeviceEpoch,
    sources: Mutex<SourceCache>,
    source_admission_bytes: u64,
}
fn unavailable(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
impl GpuRenderer {
    /// Install this device's loss callback and share its context. Owners with an
    /// existing callback must use `with_epoch` and retire that epoch themselves.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Result<Self> {
        let epoch = DeviceEpoch::default();
        let lost = epoch.clone();
        device.set_device_lost_callback(move |_, _| lost.retire());
        Self::with_epoch(device, queue, epoch)
    }
    /// Integrate with a host-owned loss callback without replacing it. Use one
    /// epoch per device lifetime; retire it before reset and never reuse it.
    pub fn with_epoch(
        device: wgpu::Device,
        queue: wgpu::Queue,
        epoch: DeviceEpoch,
    ) -> Result<Self> {
        Self::with_source_cache(device, queue, epoch, SourceCacheLimits::default())
    }
    pub fn with_source_cache(
        device: wgpu::Device,
        queue: wgpu::Queue,
        epoch: DeviceEpoch,
        limits: SourceCacheLimits,
    ) -> Result<Self> {
        epoch.check()?;
        if device.limits().max_storage_buffers_per_shader_stage < 2
            || device.limits().max_compute_invocations_per_workgroup < 64
        {
            return Err(unavailable(
                "GPU does not support the compositor's compute limits",
            ));
        }
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PaintCraft exact RGBA8 compositor"),
            source: wgpu::ShaderSource::Wgsl(include_str!("composite.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("PaintCraft ordered composition"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let presentation = if device.limits().max_storage_textures_per_shader_stage > 0 {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("PaintCraft exact egui display conversion"),
                source: wgpu::ShaderSource::Wgsl(include_str!("present.wgsl").into()),
            });
            Some(
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("PaintCraft GPU display surface"),
                    layout: None,
                    module: &module,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                }),
            )
        } else {
            None
        };
        epoch.check()?;
        Ok(Self {
            device,
            queue,
            pipeline,
            presentation,
            epoch,
            sources: Mutex::new(SourceCache::new(limits)?),
            source_admission_bytes: limits
                .buffer_bytes
                .saturating_add((limits.metadata_bytes as u64).saturating_mul(3)),
        })
    }
    pub fn epoch(&self) -> DeviceEpoch {
        self.epoch.clone()
    }
    pub fn render(&self, doc: &Document) -> Result<Vec<u8>> {
        self.render_cancellable(doc, || Ok(()))
    }
    pub fn render_cancellable(
        &self,
        doc: &Document,
        caller_check: impl Fn() -> Result<()>,
    ) -> Result<Vec<u8>> {
        self.render_with_stats(doc, caller_check)
            .map(|frame| frame.pixels)
    }
    /// Calls sharing a renderer serialize its derived source views. Cancellation
    /// callbacks must not reenter this renderer's render/cache methods.
    pub fn render_with_stats(
        &self,
        doc: &Document,
        caller_check: impl Fn() -> Result<()>,
    ) -> Result<GpuFrame> {
        let mut cache = self.sources.lock().unwrap_or_else(|poisoned| {
            let mut cache = poisoned.into_inner();
            cache.clear();
            self.sources.clear_poison();
            cache
        });
        let check = || {
            caller_check()?;
            self.epoch.check()
        };
        let result = self.render_inner(doc, &check, &mut cache);
        if result.is_err() {
            cache.clear();
        }
        result
    }
    pub fn source_cache_usage(&self) -> SourceCacheUsage {
        self.sources
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .usage()
    }
    /// Conservative native worker declaration, not physical GPU allocation/RSS.
    pub fn source_cache_admission_bytes(&self) -> u64 {
        self.source_admission_bytes
    }
    fn render_inner(
        &self,
        doc: &Document,
        check: &impl Fn() -> Result<()>,
        cache: &mut SourceCache,
    ) -> Result<GpuFrame> {
        let (output, mut stats) = self.compose(doc, check, cache)?;
        let size = u64::from(doc.canvas.0) * u64::from(doc.canvas.1) * 4;
        check()?;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PaintCraft composite readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, size);
        check()?;
        let submission = self.queue.submit([encoder.finish()]);
        let bytes = readback::copy_mapped(&self.device, &readback, submission, check)?;
        check()?;
        stats.retained = cache.usage();
        Ok(GpuFrame {
            pixels: bytes,
            stats,
        })
    }
    fn compose(
        &self,
        doc: &Document,
        check: &impl Fn() -> Result<()>,
        cache: &mut SourceCache,
    ) -> Result<(wgpu::Buffer, GpuRenderStats)> {
        cache.begin_frame();
        let mut stats = GpuRenderStats::default();
        check()?;
        let (w, h) = doc.canvas;
        let pixels = u64::from(w) * u64::from(h);
        let size = pixels * 4;
        let limits = self.device.limits();
        if pixels > crate::RENDER_PIXELS_MAX
            || size > limits.max_buffer_size
            || size > limits.max_storage_buffer_binding_size
            || w.div_ceil(8) > limits.max_compute_workgroups_per_dimension
            || h.div_ceil(8) > limits.max_compute_workgroups_per_dimension
        {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "canvas exceeds GPU compositor limits",
            ));
        }
        // wgpu guarantees fresh buffer contents are zero-initialized.
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PaintCraft composite"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let canvas = doc.canvas_rect();
        for layer in doc.layers_in_order() {
            check()?;
            if !layer.visible
                || layer.opacity == 0
                || (layer.raster.resident_tiles() == 0 && layer.raster.default_pixel().a() == 0)
            {
                continue;
            }
            let Some(area) = canvas.intersect(&layer.extent()) else {
                continue;
            };
            let width = area.width() as u32;
            let height = area.height() as u32;
            let mut encoder = self.device.create_command_encoder(&Default::default());
            let source = cache.prepare(
                &self.device,
                &mut encoder,
                doc,
                layer,
                area,
                check,
                &mut stats,
            )?;
            let params = LayerParams::new(
                [area.x0 as u32, area.y0 as u32],
                [width, height],
                w,
                layer.opacity,
            );
            let uniform = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("PaintCraft layer bounds"),
                    contents: bytemuck::bytes_of(&params),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("PaintCraft layer composition"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: source.buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: output.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform.as_entire_binding(),
                    },
                ],
            });
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &bindings, &[]);
                pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
            }
            check()?;
            let submission = self.queue.submit([encoder.finish()]);
            // Finish one bounded layer before admitting another source buffer.
            // Submitted GPU work cannot be cancelled; its reservation stays live.
            self.device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: Some(Duration::from_secs(30)),
                })
                .map_err(|e| unavailable(format!("GPU layer failed: {e}")))?;
            check()?;
            cache.completed(source);
        }
        check()?;
        Ok((output, stats))
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn staging_matches_wgsl_offsets_and_endianness() {
        assert_eq!(std::mem::size_of::<LayerParams>(), 32);
        assert_eq!(std::mem::offset_of!(LayerParams, origin), 0);
        assert_eq!(std::mem::offset_of!(LayerParams, extent), 8);
        assert_eq!(std::mem::offset_of!(LayerParams, canvas_width), 16);
        assert_eq!(std::mem::offset_of!(LayerParams, opacity), 20);
        assert_eq!(std::mem::offset_of!(LayerParams, padding), 24);
        let params = LayerParams::new([1, 2], [0x12345678, 4], 5, 127);
        let bytes = bytemuck::bytes_of(&params);
        assert_eq!(&bytes[8..12], &[0x78, 0x56, 0x34, 0x12]);
        assert_eq!(&bytes[20..24], &[127, 0, 0, 0]);
        assert_eq!(&bytes[24..], &[0; 8]);
    }
}
