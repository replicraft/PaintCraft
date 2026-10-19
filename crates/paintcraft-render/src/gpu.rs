//! Ordered GPU composition using the same integer policy as the scalar oracle.
//!
//! This first backend reads back straight RGBA8 for the shared display/export
//! boundary. It is a correctness baseline, not a zero-copy or performance claim.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;
use std::{sync::mpsc, time::Duration};
use wgpu::util::DeviceExt as _;

#[derive(Debug)]
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
}
fn unavailable(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
impl GpuRenderer {
    /// Share the desktop device rather than creating a second GPU context.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Result<Self> {
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
        Ok(Self {
            device,
            queue,
            pipeline,
        })
    }
    pub fn render(&self, doc: &Document) -> Result<Vec<u8>> {
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
            if !layer.visible || layer.opacity == 0 {
                continue;
            }
            let Some(area) = canvas.intersect(&layer.extent()) else {
                continue;
            };
            let width = area.width() as u32;
            let height = area.height() as u32;
            let mut bytes = vec![0; width as usize * height as usize * 4];
            for (row, y) in bytes
                .chunks_exact_mut(width as usize * 4)
                .zip(area.y0..area.y1)
            {
                layer
                    .raster
                    .read_span(y - layer.origin.1, area.x0 - layer.origin.0, row);
            }
            let source = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("PaintCraft layer samples"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::STORAGE,
                });
            let params: Vec<u8> = [
                area.x0 as u32,
                area.y0 as u32,
                width,
                height,
                w,
                u32::from(layer.opacity),
                0,
                0,
            ]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
            let uniform = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("PaintCraft layer bounds"),
                    contents: &params,
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("PaintCraft layer composition"),
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: source.as_entire_binding(),
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
            let mut encoder = self.device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &bindings, &[]);
                pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
            }
            self.queue.submit([encoder.finish()]);
        }
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PaintCraft composite readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, size);
        let submission = self.queue.submit([encoder.finish()]);
        let (send, recv) = mpsc::sync_channel(1);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = send.send(result);
            });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(30)),
            })
            .map_err(|e| unavailable(format!("GPU submission failed: {e}")))?;
        recv.recv_timeout(Duration::from_secs(30))
            .map_err(|e| unavailable(format!("GPU readback timed out: {e}")))?
            .map_err(|e| unavailable(format!("GPU readback failed: {e}")))?;
        let bytes = readback
            .slice(..)
            .get_mapped_range()
            .map_err(|e| unavailable(format!("GPU mapping failed: {e}")))?
            .to_vec();
        readback.unmap();
        Ok(bytes)
    }
}
