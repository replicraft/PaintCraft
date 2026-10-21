//! Exact single-layer source-to-display conversion; no intermediate composite.
use super::*;
use paintcraft_document::Layer;

pub(super) fn eligible<'a>(
    doc: &'a Document,
    region: Rect,
    check: &impl Fn() -> Result<()>,
) -> Result<Option<&'a Layer>> {
    let mut candidate = None;
    for layer in doc.layers_in_order() {
        check()?;
        if !layer.visible
            || layer.opacity == 0
            || (layer.raster.resident_tiles() == 0 && layer.raster.default_pixel().a() == 0)
            || layer.extent().intersect(&region).is_none()
        {
            continue;
        }
        // Partial coverage needs a transparent output border. Keep the general
        // ordered compositor for that case, including sparse partial layers.
        if candidate.is_some() || layer.extent().intersect(&region) != Some(region) {
            return Ok(None);
        }
        candidate = Some(layer);
    }
    Ok(candidate)
}

impl GpuRenderer {
    pub(super) fn render_single_layer(
        &self,
        doc: &Document,
        layer: &Layer,
        region: Rect,
        check: &impl Fn() -> Result<()>,
        cache: &mut SourceCache,
    ) -> Result<GpuTextureFrame> {
        self.checked_region_size(doc, region)?;
        check()?;
        let pipeline = self.single.get_or_init(|| {
            let module = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("PaintCraft single-layer display"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("single.wgsl").into()),
                });
            self.device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("PaintCraft single-layer display"),
                    layout: None,
                    module: &module,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                })
        });
        check()?;
        let mut stats = GpuRenderStats::default();
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let source = cache.prepare(
            &self.device,
            &mut encoder,
            doc,
            layer,
            region,
            check,
            &mut stats,
        )?;
        let (texture, view) = self.display_texture(region.width() as u32, region.height() as u32);
        let opacity = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("PaintCraft single-layer opacity"),
                contents: bytemuck::cast_slice(&[u32::from(layer.opacity).to_le(), 0, 0, 0]),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PaintCraft single-layer samples"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: source.buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: opacity.as_entire_binding(),
                },
            ],
        });
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups(
                (region.width() as u32).div_ceil(8),
                (region.height() as u32).div_ceil(8),
                1,
            );
        }
        check()?;
        let submission = admission::submit(&self.queue, &self.submission_identity, encoder);
        submission.wait(&self.device, &self.epoch)?;
        check()?;
        cache.completed(source);
        stats.single_layer_displays = 1;
        stats.retained = cache.usage();
        stats.retained_output_bytes = self.output_pool_usage();
        Ok(GpuTextureFrame {
            region,
            level: 0,
            texture,
            view,
            device: self.device.clone(),
            queue: self.queue.clone(),
            submission_identity: self.submission_identity.clone(),
            epoch: self.epoch.clone(),
            stats,
        })
    }
}
