//! Bounded GPU native blocks and exact wide-integer display reduction.
use super::*;
use crate::display::{DisplayRequest, Sum, uniform};
use paintcraft_core::types::Rgba;

#[derive(Debug)]
pub(super) struct Pipelines {
    fill: wgpu::ComputePipeline,
    accumulate: wgpu::ComputePipeline,
    resolve: wgpu::ComputePipeline,
}
impl Pipelines {
    fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PaintCraft exact wide display reduction"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{}\n{}",
                    include_str!("wide.wgsl"),
                    include_str!("scaled.wgsl")
                )
                .into(),
            ),
        });
        let pipeline = |entry| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        Self {
            fill: pipeline("fill_known"),
            accumulate: pipeline("accumulate"),
            resolve: pipeline("resolve"),
        }
    }
}
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct Params {
    native_origin: [u32; 2],
    native_size: [u32; 2],
    sample_origin: [u32; 2],
    output_size: [u32; 2],
    output_origin: [u32; 2],
    step: u32,
    edge: u32,
}
impl Params {
    fn new(native: Rect, samples: Rect, request: DisplayRequest, edge: u32) -> Self {
        let step = request.step();
        Self {
            native_origin: [native.x0 as u32, native.y0 as u32].map(u32::to_le),
            native_size: [native.width() as u32, native.height() as u32].map(u32::to_le),
            sample_origin: [samples.x0 as u32, samples.y0 as u32].map(u32::to_le),
            output_size: [
                (samples.width() as u32).div_ceil(step),
                (samples.height() as u32).div_ceil(step),
            ]
            .map(u32::to_le),
            output_origin: [
                ((samples.x0 - request.region().x0) as u32) / step,
                ((samples.y0 - request.region().y0) as u32) / step,
            ]
            .map(u32::to_le),
            step: step.to_le(),
            edge: edge.to_le(),
        }
    }
}
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
struct Words([[u32; 2]; 4]);
struct Tile {
    rect: Rect,
    color: Option<Rgba>,
}

impl GpuRenderer {
    /// Completed scaled display, using the CPU oracle's exact composite/filter order.
    /// Native blocks and accumulators stay bounded; no pixel readback is performed.
    pub fn render_display_texture(
        &self,
        doc: &Document,
        request: DisplayRequest,
        caller_check: impl Fn() -> Result<()>,
    ) -> Result<GpuTextureFrame> {
        if doc.canvas.0 > paintcraft_core::types::policy::CANVAS_SIDE_MAX
            || doc.canvas.1 > paintcraft_core::types::policy::CANVAS_SIDE_MAX
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "scaled GPU canvas exceeds dimension policy",
            ));
        }
        let request = DisplayRequest::new(doc, request.region(), request.level())?;
        if request.level() == 0 {
            return self.render_region_texture(doc, request.region(), caller_check);
        }
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
        let result = self.scaled_inner(doc, request, &check, &mut cache);
        if result.is_err() {
            cache.clear();
            self.clear_output_pool();
        }
        result
    }
    fn scaled_inner(
        &self,
        doc: &Document,
        request: DisplayRequest,
        check: &dyn Fn() -> Result<()>,
        cache: &mut SourceCache,
    ) -> Result<GpuTextureFrame> {
        check()?;
        crate::spatial::preflight(doc, request.region(), check)?;
        cache.begin_frame();
        if self.presentation.is_none() {
            return Err(unavailable("GPU display textures unsupported"));
        }
        let [width, height] = request.size();
        let limits = self.device.limits();
        if width > limits.max_texture_dimension_2d
            || height > limits.max_texture_dimension_2d
            || width.div_ceil(8) > limits.max_compute_workgroups_per_dimension
            || height.div_ceil(8) > limits.max_compute_workgroups_per_dimension
        {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "scaled display exceeds GPU texture/dispatch limits",
            ));
        }
        let pipelines = self.scaled.get_or_init(|| Pipelines::new(&self.device));
        check()?;
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("PaintCraft scaled native display"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let region = request.region();
        let edge = self.scaled_edge;
        let span = i64::from(edge) * i64::from(request.step());
        let columns = (region.x1 - 1) / span - region.x0 / span + 1;
        let rows = (region.y1 - 1) / span - region.y0 / span + 1;
        let count = (columns * rows) as usize;
        let mut tiles = Vec::new();
        let mut known = Vec::new();
        tiles.try_reserve_exact(count).map_err(|_| {
            Error::new(ErrorCode::ResourceLimit, "cannot allocate scaled tile plan")
        })?;
        known.try_reserve_exact(count).map_err(|_| {
            Error::new(
                ErrorCode::ResourceLimit,
                "cannot allocate scaled uniform plan",
            )
        })?;
        let mut stats = GpuRenderStats::default();
        for ty in region.y0 / span..=(region.y1 - 1) / span {
            for tx in region.x0 / span..=(region.x1 - 1) / span {
                check()?;
                let rect = Rect::new(tx * span, ty * span, span, span)
                    .unwrap()
                    .intersect(&region)
                    .unwrap();
                let color = uniform(doc, rect, check)?;
                let mut words = Words([[0; 2]; 4]);
                if let Some(color) = color {
                    words.0[0] = [u32::from_le_bytes(color.0).to_le(), 1u32.to_le()];
                    stats.scaled_uniform_pixels += rect.area() as u64;
                }
                known.push(words);
                tiles.push(Tile { rect, color });
            }
        }
        stats.scaled_tiles = tiles.len() as u64;
        let known_bytes = (known.len() as u64) * 32;
        let has_unknown = tiles.iter().any(|tile| tile.color.is_none());
        let required = known_bytes.max(if has_unknown {
            u64::from(edge * edge) * 32
        } else {
            0
        });
        if required > limits.max_buffer_size || required > limits.max_storage_buffer_binding_size {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "scaled GPU metadata exceeds storage limits",
            ));
        }
        let known_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("PaintCraft uniform scaled tiles"),
                contents: bytemuck::cast_slice(&known),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let params = Params::new(region, region, request, edge);
        self.scaled_pass(
            &pipelines.fill,
            &params,
            &[
                (3, wgpu::BindingResource::TextureView(&view)),
                (4, known_buffer.as_entire_binding()),
            ],
            check,
        )?;
        // Exactly one accumulator and native block may be in flight. The completed
        // waits below retain ownership/admission through every dependent GPU read.
        if has_unknown {
            let accumulator = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("PaintCraft wide display accumulators"),
                size: u64::from(edge * edge) * 32,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            for tile in tiles.into_iter().filter(|tile| tile.color.is_none()) {
                check()?;
                let mut encoder = self.device.create_command_encoder(&Default::default());
                encoder.clear_buffer(&accumulator, 0, None);
                let submission = admission::submit(&self.queue, &self.submission_identity, encoder);
                submission.wait(&self.device, &self.epoch)?;
                let [w, h] = [
                    (tile.rect.width() as u32).div_ceil(request.step()),
                    (tile.rect.height() as u32).div_ceil(request.step()),
                ];
                let count = (w * h) as usize;
                let mut sums = Vec::new();
                sums.try_reserve_exact(count).map_err(|_| {
                    Error::new(
                        ErrorCode::ResourceLimit,
                        "cannot allocate scaled uniform sums",
                    )
                })?;
                sums.resize(count, Sum::default());
                let mut reduction = Reduction {
                    gpu: self,
                    pipelines,
                    doc,
                    request,
                    tile: tile.rect,
                    accumulator: &accumulator,
                    cache,
                    stats: &mut stats,
                    sums: &mut sums,
                    check,
                };
                reduction.visit(tile.rect)?;
                let mut words = Vec::new();
                words.try_reserve_exact(count).map_err(|_| {
                    Error::new(
                        ErrorCode::ResourceLimit,
                        "cannot allocate wide uniform words",
                    )
                })?;
                words.resize(count, Words([[0; 2]; 4]));
                for (word, sum) in words.iter_mut().zip(&sums).take((w * h) as usize) {
                    for (out, value) in word.0.iter_mut().zip(sum.0) {
                        *out = [(value as u32).to_le(), ((value >> 32) as u32).to_le()];
                    }
                }
                let uniform = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("PaintCraft exact uniform contributions"),
                        contents: bytemuck::cast_slice(&words[..(w * h) as usize]),
                        usage: wgpu::BufferUsages::STORAGE,
                    });
                let params = Params::new(tile.rect, tile.rect, request, edge);
                self.scaled_pass(
                    &pipelines.resolve,
                    &params,
                    &[
                        (1, accumulator.as_entire_binding()),
                        (3, wgpu::BindingResource::TextureView(&view)),
                        (4, uniform.as_entire_binding()),
                    ],
                    check,
                )?;
            }
        }
        check()?;
        stats.retained = cache.usage();
        stats.retained_output_bytes = self.output_pool_usage();
        Ok(GpuTextureFrame {
            region,
            level: request.level(),
            texture,
            view,
            device: self.device.clone(),
            queue: self.queue.clone(),
            submission_identity: self.submission_identity.clone(),
            epoch: self.epoch.clone(),
            stats,
        })
    }
    fn scaled_pass(
        &self,
        pipeline: &wgpu::ComputePipeline,
        params: &Params,
        entries: &[(u32, wgpu::BindingResource<'_>)],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        check()?;
        let uniform = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("PaintCraft scaled bounds"),
                contents: bytemuck::bytes_of(params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let mut bindings = Vec::new();
        bindings
            .try_reserve_exact(entries.len() + 1)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "cannot allocate scaled bindings"))?;
        bindings.push(wgpu::BindGroupEntry {
            binding: 2,
            resource: uniform.as_entire_binding(),
        });
        for (binding, resource) in entries {
            bindings.push(wgpu::BindGroupEntry {
                binding: *binding,
                resource: resource.clone(),
            });
        }
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PaintCraft scaled operation"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &bindings,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(
                u32::from_le(params.output_size[0]).div_ceil(8),
                u32::from_le(params.output_size[1]).div_ceil(8),
                1,
            );
        }
        check()?;
        let submission = admission::submit(&self.queue, &self.submission_identity, encoder);
        submission.wait(&self.device, &self.epoch)?;
        check()
    }
}
struct Reduction<'a> {
    gpu: &'a GpuRenderer,
    pipelines: &'a Pipelines,
    doc: &'a Document,
    request: DisplayRequest,
    tile: Rect,
    accumulator: &'a wgpu::Buffer,
    cache: &'a mut SourceCache,
    stats: &'a mut GpuRenderStats,
    sums: &'a mut [Sum],
    check: &'a dyn Fn() -> Result<()>,
}
impl Reduction<'_> {
    fn visit(&mut self, rect: Rect) -> Result<()> {
        (self.check)()?;
        if let Some(color) = uniform(self.doc, rect, self.check)? {
            self.stats.scaled_uniform_pixels += rect.area() as u64;
            let step = i64::from(self.request.step());
            let width = (self.tile.width() as u32).div_ceil(self.request.step()) as usize;
            for y in (rect.y0 - self.tile.y0) / step..=(rect.y1 - self.tile.y0 - 1) / step {
                for x in (rect.x0 - self.tile.x0) / step..=(rect.x1 - self.tile.x0 - 1) / step {
                    let cell =
                        Rect::new(self.tile.x0 + x * step, self.tile.y0 + y * step, step, step)
                            .unwrap();
                    self.sums[y as usize * width + x as usize]
                        .add(color, cell.intersect(&rect).unwrap().area() as u64);
                }
            }
        } else if rect.width() <= 256 && rect.height() <= 256 {
            let gpu = self.gpu;
            let (mut output, part) = gpu.compose(self.doc, rect, &|| (self.check)(), self.cache)?;
            self.stats.composition_submissions += part.composition_submissions;
            self.stats.max_submission_layers = self
                .stats
                .max_submission_layers
                .max(part.max_submission_layers);
            self.stats.max_submission_source_bytes = self
                .stats
                .max_submission_source_bytes
                .max(part.max_submission_source_bytes);
            self.stats.max_submission_metadata_bytes = self
                .stats
                .max_submission_metadata_bytes
                .max(part.max_submission_metadata_bytes);
            self.stats.uploaded_bytes += part.uploaded_bytes;
            self.stats.upload_strips += part.upload_strips;
            self.stats.source_hits += part.source_hits;
            self.stats.patched_sources += part.patched_sources;
            self.stats.source_allocations += part.source_allocations;
            self.stats.output_allocations += part.output_allocations;
            self.stats.output_reuses += part.output_reuses;
            self.stats.output_cleared_bytes += part.output_cleared_bytes;
            self.stats.scaled_native_pixels += rect.area() as u64;
            let params = Params::new(rect, self.tile, self.request, gpu.scaled_edge);
            gpu.scaled_pass(
                &self.pipelines.accumulate,
                &params,
                &[
                    (0, output.as_entire_binding()),
                    (1, self.accumulator.as_entire_binding()),
                ],
                self.check,
            )?;
            output.completed();
        } else {
            let (a, b) = if rect.width() >= rect.height() {
                let mid = rect.x0 + rect.width() / 2;
                (Rect { x1: mid, ..rect }, Rect { x0: mid, ..rect })
            } else {
                let mid = rect.y0 + rect.height() / 2;
                (Rect { y1: mid, ..rect }, Rect { y0: mid, ..rect })
            };
            self.visit(a)?;
            self.visit(b)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn scaled_params_match_wgsl_offsets_and_pair_layout() {
        assert_eq!(std::mem::size_of::<Params>(), 48);
        assert_eq!(std::mem::offset_of!(Params, output_size), 24);
        assert_eq!(std::mem::offset_of!(Params, output_origin), 32);
        assert_eq!(std::mem::offset_of!(Params, step), 40);
        assert_eq!(std::mem::size_of::<Words>(), 32);
    }
}
