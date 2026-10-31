//! Ordered GPU composition using the same integer policy as the scalar oracle.
//!
//! This first backend reads back straight RGBA8 for the shared display/export
//! boundary. It is a correctness baseline, not a zero-copy or performance claim.
mod admission;
#[cfg(test)]
mod admission_tests;
pub use admission::{with_admission, with_admission_progress};
mod progress;
#[cfg(test)]
mod progress_tests;
pub use progress::{CompletionDriver, CompletionProgress};
mod batch;
mod completion;
mod epoch;
mod output;
mod presentation;
pub use output::DEFAULT_OUTPUT_POOL_BYTES;
mod readback;
mod scaled;
mod single;
mod sources;
pub use epoch::DeviceEpoch;
pub use presentation::GpuTextureFrame;
use sources::SourceCache;
pub use sources::{GpuFrame, GpuRenderStats, SourceCacheLimits, SourceCacheUsage};

use paintcraft_core::types::{Error, ErrorCode, Rect, Result};
use paintcraft_document::Document;
use std::{
    sync::{Mutex, OnceLock, mpsc},
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
    // wgpu queue equality may alias IDs from independent instances.
    // Frames retain this private identity after the compositor is retired.
    submission_identity: std::sync::Arc<()>,
    pipeline: wgpu::ComputePipeline,
    presentation: Option<wgpu::ComputePipeline>,
    scaled: OnceLock<scaled::Pipelines>,
    single: OnceLock<wgpu::ComputePipeline>,
    single_layer_display: bool,
    scaled_edge: u32,
    submission_batching: bool,
    epoch: DeviceEpoch,
    sources: Mutex<SourceCache>,
    source_admission_bytes: u64,
    output: Mutex<output::Pool>,
    output_admission_bytes: u64,
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
        Self::with_working_cache_limits(device, queue, epoch, limits, DEFAULT_OUTPUT_POOL_BYTES)
    }
    /// Output retention is bounded independently from source residency. Zero disables
    /// reuse; active buffers still follow ordinary output admission and completion.
    pub fn with_working_cache_limits(
        device: wgpu::Device,
        queue: wgpu::Queue,
        epoch: DeviceEpoch,
        limits: SourceCacheLimits,
        output_pool_bytes: u64,
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
            submission_identity: std::sync::Arc::new(()),
            pipeline,
            presentation,
            scaled: OnceLock::new(),
            single: OnceLock::new(),
            single_layer_display: false,
            scaled_edge: 32,
            submission_batching: false,
            epoch,
            sources: Mutex::new(SourceCache::new(limits)?),
            output: Mutex::new(output::Pool::new(output_pool_bytes)),
            output_admission_bytes: output_pool_bytes,
            source_admission_bytes: limits
                .buffer_bytes
                .saturating_add((limits.metadata_bytes as u64).saturating_mul(3)),
        })
    }
    /// Internal output tiling changes work/residency, never the sampling grid or pixels.
    /// Both bounded configurations are exercised by native adoption comparisons.
    pub fn with_scaled_tile_edge(mut self, edge: u32) -> Result<Self> {
        if !matches!(edge, 32 | 64) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "scaled tile edge must be 32 or 64",
            ));
        }
        self.scaled_edge = edge;
        Ok(self)
    }

    /// Retain multiple bounded region views per layer instead of replacing its
    /// previous view. This changes residency only; source identities stay exact.
    pub fn with_region_source_cache(mut self, enabled: bool) -> Self {
        let cache = self.sources.get_mut().unwrap_or_else(|p| p.into_inner());
        cache.clear();
        cache.region_views = enabled;
        self
    }

    /// Present one fully covering flat layer directly from cached source samples.
    /// Experimental until native comparisons qualify the dispatch policy.
    pub fn with_single_layer_display(mut self, enabled: bool) -> Self {
        self.single_layer_display = enabled;
        self
    }

    /// Group ordered layer dispatches within bounded source/metadata windows.
    /// Disabled by default until completed native comparisons select a policy.
    pub fn with_submission_batching(mut self, enabled: bool) -> Self {
        self.submission_batching = enabled;
        self
    }
    /// Additional declared pending-source/key allowance; not physical driver memory.
    pub fn submission_admission_bytes(&self) -> u64 {
        if self.submission_batching {
            batch::ADMISSION_BYTES
        } else {
            0
        }
    }

    /// Bounded native block, wide accumulators/words and transfer bookkeeping.
    /// Per-output tile plans are covered by the caller's output-area declaration.
    pub fn scaled_scratch_admission_bytes(&self) -> u64 {
        2 << 20
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
        self.render_region_with_stats(doc, doc.canvas_rect(), caller_check)
    }
    /// Exact straight-alpha output for a nonempty region inside the canvas.
    /// Source residency remains keyed in document coordinates; output is tightly
    /// packed from the requested region's origin.
    pub fn render_region_with_stats(
        &self,
        doc: &Document,
        region: Rect,
        caller_check: impl Fn() -> Result<()>,
    ) -> Result<GpuFrame> {
        let levels_caller_check = &caller_check;
        let caller_check = || levels_caller_check();
        crate::prepared::with_document_programs(doc, &caller_check, || {
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
            let result = self.render_inner(doc, region, &check, &mut cache);
            if result.is_err() {
                cache.clear();
                self.clear_output_pool();
            }
            result
        })
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
        region: Rect,
        check: &impl Fn() -> Result<()>,
        cache: &mut SourceCache,
    ) -> Result<GpuFrame> {
        cache.begin_frame();
        let (mut output, mut stats) = self.compose(doc, region, check, cache)?;
        let size = crate::region_bytes(doc, region)? as u64;
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
        let submission = admission::submit(&self.queue, &self.submission_identity, encoder);
        let bytes = readback::copy_mapped(&self.device, &self.epoch, &readback, submission, check)?;
        check()?;
        output.completed();
        drop(output);
        stats.retained_output_bytes = self.output_pool_usage();
        stats.retained = cache.usage();
        Ok(GpuFrame {
            pixels: bytes,
            stats,
        })
    }
    fn checked_region_size(&self, doc: &Document, region: Rect) -> Result<u64> {
        crate::region_bytes(doc, region)?;
        let (w, h) = (region.width() as u32, region.height() as u32);
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
        Ok(size)
    }
    fn compose(
        &self,
        doc: &Document,
        region: Rect,
        check: &impl Fn() -> Result<()>,
        cache: &mut SourceCache,
    ) -> Result<(output::Output<'_>, GpuRenderStats)> {
        let mut stats = GpuRenderStats::default();
        check()?;
        crate::spatial::preflight(doc, region, check)?;
        let size = self.checked_region_size(doc, region)?;
        let w = region.width() as u32;
        // Fresh buffers are zero-initialized by wgpu. Reused buffers are cleared
        // in the first contributing layer's encoder, before any source-over reads.
        let output = self.acquire_output(size);
        stats.output_allocations = u64::from(!output.reused);
        stats.output_reuses = u64::from(output.reused);
        stats.output_cleared_bytes = if output.reused { size } else { 0 };
        let mut clear_pending = output.reused;
        let mut batch = batch::Batch::new(self);
        let canvas = region;
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
            batch.before(area.area() as u64 * 4, cache, check, &mut stats)?;
            let encoder = batch.encoder();
            if clear_pending {
                encoder.clear_buffer(&output, 0, Some(size));
                clear_pending = false;
            }
            let source =
                cache.prepare(&self.device, encoder, doc, layer, area, check, &mut stats)?;
            let params = LayerParams::new(
                [(area.x0 - region.x0) as u32, (area.y0 - region.y0) as u32],
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
            batch.push(source)?;
        }
        batch.finish(cache, check, &mut stats)?;
        if clear_pending {
            let mut encoder = self.device.create_command_encoder(&Default::default());
            encoder.clear_buffer(&output, 0, Some(size));
            check()?;
            let submission = admission::submit(&self.queue, &self.submission_identity, encoder);
            stats.composition_submissions += 1;
            submission.wait(&self.device, &self.epoch)?;
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
