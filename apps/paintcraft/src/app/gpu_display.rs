//! Native egui registration is owned by the accepted frame, never by a worker.
use super::*;
use paintcraft_render::gpu::{DeviceEpoch, GpuTextureFrame};
#[derive(Clone)]
pub(super) struct Target {
    state: eframe::egui_wgpu::RenderState,
    epoch: DeviceEpoch,
}
pub(super) struct Texture {
    pub id: egui::TextureId,
    renderer: Arc<eframe::egui::mutex::RwLock<eframe::egui_wgpu::Renderer>>,
    _frame: GpuTextureFrame,
}
#[cfg(test)]
impl Texture {
    pub(crate) fn frame(&self) -> &GpuTextureFrame {
        &self._frame
    }
}
impl Drop for Texture {
    fn drop(&mut self) {
        self.renderer.write().free_texture(&self.id);
    }
}
impl Target {
    #[cfg(test)]
    pub(crate) fn adapter_info(&self) -> eframe::wgpu::AdapterInfo {
        self.state.adapter.get_info()
    }
    pub fn new(state: eframe::egui_wgpu::RenderState, epoch: DeviceEpoch) -> Self {
        Self { state, epoch }
    }
    pub fn install(&self, frame: GpuTextureFrame) -> paintcraft_core::types::Result<Texture> {
        self.epoch.check()?;
        if frame.epoch() != self.epoch {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::CapabilityUnsupported,
                "GPU display epoch changed",
            ));
        }
        let id = self.state.renderer.write().register_native_texture(
            &self.state.device,
            frame.view(),
            eframe::wgpu::FilterMode::Nearest,
        );
        Ok(Texture {
            id,
            renderer: self.state.renderer.clone(),
            _frame: frame,
        })
    }
}
impl App {
    pub(super) fn install_gpu_texture(
        &mut self,
        ctx: &egui::Context,
        key: RenderKey,
        frame: GpuTextureFrame,
    ) {
        if !self.accept_render_key(key) {
            return;
        }
        let result = (|| {
            let size = frame.size();
            if frame.level() != key.4 || frame.region() != key.3 || size != Self::display_size(key)
            {
                return Err(paintcraft_core::types::Error::new(
                    paintcraft_core::types::ErrorCode::InvariantViolation,
                    "GPU display bounds differ from requested region",
                ));
            }
            self.gpu_target
                .as_ref()
                .ok_or_else(|| {
                    paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::CapabilityUnsupported,
                        "GPU display target unavailable",
                    )
                })?
                .install(frame)
        })();
        match result {
            Ok(texture) => {
                self.gpu_texture = Some(texture);
                self.texture = None;
                self.rendered.clear();
                self.render_residency_retry = None;
                self.update_capacity_interest();
                if let Some(wakes) = &self.budget_wakes {
                    wakes.wait_for_release(false);
                }
                self.texture_key = Some(key);
            }
            Err(error) => {
                self.replace_gpu(None);
                self.editor
                    .set_status(format!("Using CPU display: {error}"));
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn target() -> Option<(Target, Arc<GpuRenderer>)> {
        target_with_output_pool(paintcraft_render::gpu::DEFAULT_OUTPUT_POOL_BYTES)
    }
    pub(crate) fn target_with_output_pool(bytes: u64) -> Option<(Target, Arc<GpuRenderer>)> {
        target_with_scaled_edge(bytes, 32)
    }
    pub(crate) fn target_with_scaled_edge(
        bytes: u64,
        edge: u32,
    ) -> Option<(Target, Arc<GpuRenderer>)> {
        target_with_limits(bytes, edge, None)
    }
    pub(crate) fn target_with_limits(
        bytes: u64,
        edge: u32,
        limits: Option<eframe::wgpu::Limits>,
    ) -> Option<(Target, Arc<GpuRenderer>)> {
        use eframe::wgpu;
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
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: limits.unwrap_or_default(),
            ..Default::default()
        }))
        .unwrap();
        let epoch = DeviceEpoch::default();
        let lost = epoch.clone();
        device.set_device_lost_callback(move |_, _| lost.retire());
        let gpu = Arc::new(
            GpuRenderer::with_working_cache_limits(
                device.clone(),
                queue.clone(),
                epoch,
                paintcraft_render::gpu::SourceCacheLimits::default(),
                bytes,
            )
            .unwrap()
            .with_scaled_tile_edge(edge)
            .unwrap(),
        );
        let renderer = eframe::egui_wgpu::Renderer::new(
            &device,
            wgpu::TextureFormat::Rgba8Unorm,
            eframe::egui_wgpu::RendererOptions {
                dithering: false,
                predictable_texture_filtering: true,
                ..Default::default()
            },
        );
        let state = eframe::egui_wgpu::RenderState {
            adapter: adapter.clone(),
            available_adapters: vec![adapter],
            instance,
            device,
            queue,
            target_format: wgpu::TextureFormat::Rgba8Unorm,
            renderer: Arc::new(egui::mutex::RwLock::new(renderer)),
        };
        Some((Target::new(state, gpu.epoch()), gpu))
    }
    #[test]
    fn native_display_matches_egui_conversion_and_releases_registration_on_cpu_fallback() {
        let Some((target, gpu)) = target() else {
            return;
        };
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        let data: Vec<u8> = (0u16..256)
            .flat_map(|alpha| {
                (0u16..256).flat_map(move |color| {
                    [
                        color as u8,
                        (255 - color) as u8,
                        (color as u8) ^ 0x5a,
                        alpha as u8,
                    ]
                })
            })
            .collect();
        let mut doc = paintcraft_document::new_document(
            256,
            256,
            Some(Rgba::TRANSPARENT),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let id = doc.roots[0];
        doc.layers.get_mut(&id).unwrap().raster = paintcraft_raster::RasterLayer::from_rgba(
            256,
            256,
            Rgba::TRANSPARENT,
            &data,
            &mut paintcraft_raster::Admission::new(65536),
        )
        .unwrap();
        app.editor = Editor::with_document(doc, None);
        app.gpu = Some(gpu.clone());
        app.gpu_target = Some(target.clone());
        let straight = paintcraft_render::render_flat(app.editor.document()).unwrap();
        let expected: Vec<u8> = egui::ColorImage::from_rgba_unmultiplied([256, 256], &straight)
            .pixels
            .iter()
            .flat_map(|p| p.to_array())
            .collect();
        let frame = gpu
            .render_texture(app.editor.document(), || Ok(()))
            .unwrap();
        assert_eq!(frame.readback(|| Ok(())).unwrap(), expected);
        let key = app.presentation_key();
        let ticket = app.editor.render_key();
        app.install_gpu_texture(&ctx, key, frame);
        let id = app.gpu_texture.as_ref().unwrap().id;
        assert!(target.state.renderer.read().texture(&id).is_some());
        assert!(app.texture.is_none());
        assert!(app.rendered.is_empty());
        assert_eq!(app.texture_key, Some(key));
        assert_eq!(app.editor.render_key(), ticket);
        app.install_render(&ctx, key, Ok(straight));
        assert!(app.gpu_texture.is_none());
        assert!(target.state.renderer.read().texture(&id).is_none());
    }
    #[test]
    fn stale_keys_and_backend_replacement_cannot_retain_native_registrations() {
        let Some((target, gpu)) = target() else {
            return;
        };
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        app.editor = Editor::new_document(19, 17).unwrap();
        app.gpu = Some(gpu.clone());
        app.gpu_target = Some(target.clone());
        let key = app.presentation_key();
        let stale = gpu
            .render_texture(app.editor.document(), || Ok(()))
            .unwrap();
        let mut wrong_key = key;
        wrong_key.1 += 1;
        app.install_gpu_texture(&ctx, wrong_key, stale);
        assert!(app.gpu_texture.is_none());
        let frame = gpu
            .render_texture(app.editor.document(), || Ok(()))
            .unwrap();
        app.install_gpu_texture(&ctx, key, frame);
        let id = app.gpu_texture.as_ref().unwrap().id;
        assert!(target.state.renderer.read().texture(&id).is_some());
        app.replace_gpu(None);
        assert!(app.gpu_texture.is_none());
        assert!(app.gpu_target.is_none());
        assert!(target.state.renderer.read().texture(&id).is_none());
        // An old completed payload cannot register against a replacement epoch.
        let old = gpu
            .render_texture(app.editor.document(), || Ok(()))
            .unwrap();
        let other_epoch = DeviceEpoch::default();
        let wrong = Target::new(target.state.clone(), other_epoch);
        assert!(wrong.install(old).is_err());
    }
}

#[cfg(test)]
mod pixel_tests {
    use super::*;
    #[test]
    fn native_and_managed_images_draw_identical_pixels_in_the_actual_egui_renderer() {
        for region in [
            paintcraft_core::types::Rect::new(0, 0, 32, 32).unwrap(),
            paintcraft_core::types::Rect::new(5, 3, 23, 29).unwrap(),
        ] {
            for level in [0, 1, 2, 3] {
                compare_region(region, level);
            }
        }
    }
    fn compare_region(region: paintcraft_core::types::Rect, level: u8) {
        use eframe::wgpu;
        let Some((target, gpu)) = super::tests::target_with_output_pool(64 << 20) else {
            return;
        };
        let ctx = egui::Context::default();
        let data: Vec<u8> = (0u32..32 * 32)
            .flat_map(|p| {
                [
                    (p * 7) as u8,
                    (p / 32 * 9) as u8,
                    (p * 13) as u8,
                    (p / 32 * 8) as u8,
                ]
            })
            .collect();
        let mut doc = paintcraft_document::new_document(
            31,
            29,
            Some(Rgba::TRANSPARENT),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let id = doc.roots[0];
        doc.layers.get_mut(&id).unwrap().raster = paintcraft_raster::RasterLayer::from_rgba(
            32,
            32,
            Rgba::TRANSPARENT,
            &data,
            &mut paintcraft_raster::Admission::new(65536),
        )
        .unwrap();
        let request = paintcraft_render::display::DisplayRequest::new(
            &doc,
            region.intersect(&doc.canvas_rect()).unwrap(),
            level,
        )
        .unwrap();
        let native = target
            .install(
                gpu.render_display_texture(&doc, request, || Ok(()))
                    .unwrap(),
            )
            .unwrap();
        // Later reuse of native scratch cannot overwrite this scaled/native image.
        let empty = paintcraft_document::new_document(31, 29, None, 0, paintcraft_document::POLICY)
            .unwrap();
        let recycled = gpu
            .render_region_texture(&empty, request.region(), || Ok(()))
            .unwrap();
        assert_eq!(recycled.stats.output_reuses, 1);
        drop(recycled);
        let straight =
            paintcraft_render::display::render_display_cancellable(&doc, request, || Ok(()))
                .unwrap()
                .pixels;
        let size = request.size();
        let managed = ctx.load_texture(
            "cpu-reference",
            egui::ColorImage::from_rgba_unmultiplied(
                [size[0] as usize, size[1] as usize],
                &straight,
            ),
            egui::TextureOptions::NEAREST,
        );
        let footprint = request.footprint();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(64.0, 32.0),
                )),
                ..Default::default()
            },
            |ui| {
                let painter = ui.ctx().layer_painter(egui::LayerId::background());
                painter.rect_filled(
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(64.0, 32.0)),
                    0.0,
                    egui::Color32::from_rgb(37, 19, 83),
                );
                let uv = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
                let image = egui::Rect::from_min_size(
                    egui::pos2(footprint.x0 as f32, footprint.y0 as f32),
                    egui::vec2(footprint.width() as f32, footprint.height() as f32),
                );
                painter
                    .with_clip_rect(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(31.0, 29.0),
                    ))
                    .image(native.id, image, uv, egui::Color32::WHITE);
                painter
                    .with_clip_rect(egui::Rect::from_min_size(
                        egui::pos2(32.0, 0.0),
                        egui::vec2(31.0, 29.0),
                    ))
                    .image(
                        managed.id(),
                        image.translate(egui::vec2(32.0, 0.0)),
                        uv,
                        egui::Color32::WHITE,
                    );
            },
        );
        let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
        let descriptor = eframe::egui_wgpu::ScreenDescriptor {
            size_in_pixels: [64, 32],
            pixels_per_point: 1.0,
        };
        let texture = target
            .state
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("native/managed display parity"),
                size: wgpu::Extent3d {
                    width: 64,
                    height: 32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
        let view = texture.create_view(&Default::default());
        let buffer = target.state.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("display parity readback"),
            size: 64 * 32 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = target
            .state
            .device
            .create_command_encoder(&Default::default());
        let callbacks = {
            let mut renderer = target.state.renderer.write();
            for (id, deltas) in &output.textures_delta.set {
                for delta in deltas {
                    renderer.update_texture(&target.state.device, &target.state.queue, *id, delta);
                }
            }
            output.textures_delta.clear();
            let callbacks = renderer.update_buffers(
                &target.state.device,
                &target.state.queue,
                &mut encoder,
                &jobs,
                &descriptor,
            );
            {
                let mut pass = encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        ..Default::default()
                    })
                    .forget_lifetime();
                renderer.render(&mut pass, &jobs, &descriptor);
            }
            callbacks
        };
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(32),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 32,
                depth_or_array_layers: 1,
            },
        );
        let submission = target
            .state
            .queue
            .submit(callbacks.into_iter().chain([encoder.finish()]));
        let (send, recv) = std::sync::mpsc::sync_channel(1);
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = send.send(r);
        });
        target
            .state
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(30)),
            })
            .unwrap();
        recv.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
        let bytes = buffer.slice(..).get_mapped_range().unwrap();
        for row in bytes.as_chunks::<256>().0 {
            assert_eq!(&row[..128], &row[128..]);
        }
        assert!(bytes.iter().any(|&b| b != 0));
        let changed = (19 * 64 + 7) * 4;
        assert_ne!(&bytes[changed..changed + 4], &bytes[..4]);
        drop(bytes);
        buffer.unmap();
    }
}

#[cfg(test)]
#[path = "gpu_display/benchmark.rs"]
pub(crate) mod benchmark;

#[cfg(test)]
#[path = "gpu_display/region_benchmark.rs"]
mod region_benchmark;
