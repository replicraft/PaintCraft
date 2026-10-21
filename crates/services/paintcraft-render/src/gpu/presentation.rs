//! Completed premultiplied display surfaces; never authoritative document storage.
use super::*;
#[derive(Debug)]
pub struct GpuTextureFrame {
    region: Rect,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    device: wgpu::Device,
    queue: wgpu::Queue,
    epoch: DeviceEpoch,
    pub stats: GpuRenderStats,
}
impl GpuTextureFrame {
    /// Half-open document-space bounds represented by this texture.
    pub fn region(&self) -> Rect {
        self.region
    }
    pub fn size(&self) -> [u32; 2] {
        [self.texture.width(), self.texture.height()]
    }
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
    pub fn epoch(&self) -> DeviceEpoch {
        self.epoch.clone()
    }
    /// Explicit verification adapter, not used by native presentation. Returns
    /// premultiplied encoded-sRGB RGBA8, not the straight document/export policy.
    pub fn readback(&self, caller_check: impl Fn() -> Result<()>) -> Result<Vec<u8>> {
        let check = || {
            caller_check()?;
            self.epoch.check()
        };
        check()?;
        let [width, height] = self.size();
        let stride = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let size = u64::from(stride) * u64::from(height);
        if size > self.device.limits().max_buffer_size {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "GPU texture readback exceeds limits",
            ));
        }
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PaintCraft display verification readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        check()?;
        let submission = self.queue.submit([encoder.finish()]);
        let padded = super::readback::copy_mapped(&self.device, &readback, submission, &check)?;
        let mut pixels = crate::output_buffer(width as usize * height as usize * 4)?;
        for (source, target) in padded
            .chunks_exact(stride as usize)
            .zip(pixels.chunks_exact_mut(width as usize * 4))
        {
            check()?;
            target.copy_from_slice(&source[..target.len()]);
        }
        check()?;
        Ok(pixels)
    }
}
impl GpuRenderer {
    /// Render directly to a completed Rgba8Unorm egui-compatible texture. It
    /// carries its originating epoch; consumers must also validate document keys.
    pub fn render_texture(
        &self,
        doc: &Document,
        caller_check: impl Fn() -> Result<()>,
    ) -> Result<GpuTextureFrame> {
        self.render_region_texture(doc, doc.canvas_rect(), caller_check)
    }
    /// Completed display texture covering only the requested document region.
    pub fn render_region_texture(
        &self,
        doc: &Document,
        region: Rect,
        caller_check: impl Fn() -> Result<()>,
    ) -> Result<GpuTextureFrame> {
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
        let result = (|| {
            check()?;
            let Some(pipeline) = &self.presentation else {
                return Err(unavailable("GPU display textures unsupported"));
            };
            crate::region_bytes(doc, region)?;
            let (width, height) = (region.width() as u32, region.height() as u32);
            if width > self.device.limits().max_texture_dimension_2d
                || height > self.device.limits().max_texture_dimension_2d
            {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "canvas exceeds GPU display texture limits",
                ));
            }
            let (output, mut stats) = self.compose(doc, region, &check, &mut cache)?;
            check()?;
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("PaintCraft native display"),
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
            let bindings = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("PaintCraft native display conversion"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: output.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                ],
            });
            let mut encoder = self.device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &bindings, &[]);
                pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
            }
            check()?;
            let submission = self.queue.submit([encoder.finish()]);
            self.device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: Some(Duration::from_secs(30)),
                })
                .map_err(|e| unavailable(format!("GPU display conversion failed: {e}")))?;
            check()?;
            stats.retained = cache.usage();
            Ok(GpuTextureFrame {
                region,
                texture,
                view,
                device: self.device.clone(),
                queue: self.queue.clone(),
                epoch: self.epoch.clone(),
                stats,
            })
        })();
        if result.is_err() {
            cache.clear()
        }
        result
    }
}
