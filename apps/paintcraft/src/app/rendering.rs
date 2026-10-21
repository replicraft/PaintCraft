//! Desktop rendering integration.

use super::*;

impl App {
    pub(super) fn presentation_key(&self) -> RenderKey {
        let (state, epoch) = self.editor.render_key();
        (
            state,
            epoch,
            self.preview_revision,
            self.render_region
                .and_then(|region| region.intersect(&self.editor.document().canvas_rect()))
                .unwrap_or_else(|| self.editor.document().canvas_rect()),
            self.render_level,
        )
    }

    pub(super) fn display_size(key: RenderKey) -> [u32; 2] {
        let step = 1u32 << key.4;
        [
            (key.3.width() as u32).div_ceil(step),
            (key.3.height() as u32).div_ceil(step),
        ]
    }

    /// Connect native services only for a real desktop window; harnesses stay deterministic.
    pub fn with_native_services(mut self, cc: &eframe::CreationContext<'_>) -> Self {
        self.worker_budget = editing::native_worker_budget();
        if let Err(error) = self.connect_budget_wakes(
            &cc.egui_ctx,
            paintcraft_raster::Admission::new(0).residency_budget(),
        ) {
            self.editor.set_status(format!(
                "Memory pressure notifications unavailable: {error}"
            ));
        }
        self.dialogs = Some(NativeDialogs::new(cc));
        self.initialize_editing(cc.egui_ctx.clone());
        self.initialize_fonts(&cc.egui_ctx);
        #[cfg(target_os = "macos")]
        self.initialize_native_menu(&cc.egui_ctx);
        match saving::NativeSaving::new(cc.egui_ctx.clone()) {
            Ok(saving) => self.saving = Some(saving),
            Err(error) => self
                .editor
                .set_status(format!("Background saving unavailable: {error}")),
        }
        self.gpu_scaled_display =
            std::env::var("PAINTCRAFT_GPU_SCALED").is_ok_and(|value| value == "1");
        if let Some(state) = &cc.wgpu_render_state {
            let epoch = paintcraft_render::gpu::DeviceEpoch::default();
            let lost = epoch.clone();
            let wake = cc.egui_ctx.clone();
            state.device.set_device_lost_callback(move |_, _| {
                lost.retire();
                wake.request_repaint();
            });
            match GpuRenderer::with_epoch(state.device.clone(), state.queue.clone(), epoch.clone())
                .and_then(|gpu| gpu.with_scaled_tile_edge(64))
            {
                Ok(gpu) => {
                    self.replace_gpu(Some(Arc::new(gpu)));
                    self.gpu_target = Some(gpu_display::Target::new(state.clone(), epoch));
                }
                Err(e) => self.editor.set_status(format!("Using CPU rendering: {e}")),
            }
        }
        match Executor::with_budget(
            WorkerLimits {
                workers: 1,
                pending_jobs: 2,
                transient_bytes: 1 << 30,
            },
            self.worker_budget.clone(),
        ) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => self
                .editor
                .set_status(format!("Background renderer unavailable: {error}")),
        }
        self
    }

    pub(super) fn sync_texture(&mut self, ctx: &egui::Context) {
        if self
            .gpu
            .as_ref()
            .is_some_and(|gpu| !gpu.epoch().is_current())
        {
            self.replace_gpu(None);
            self.editor
                .set_status("GPU device lost; rebuilding canvas with CPU rendering");
        }
        let preview = self.editor.preview_snapshot().filter(|p| {
            matches!(
                p.as_ref(),
                Preview::Path(_) | Preview::Paths(_) | Preview::Text { style: Some(_), .. }
            )
        });
        let owner = preview.as_ref().map(|_| self.editor.preview_owner());
        let owner_changed = owner != self.preview_gesture;
        let same_snapshot = match (&self.rendered_preview, &preview) {
            (Some(old), Some(new)) => Arc::ptr_eq(old, new),
            (None, None) => true,
            _ => false,
        };
        if owner_changed || (!same_snapshot && self.rendered_preview != preview) {
            let revision = self
                .preview_revision
                .checked_add(1)
                .expect("preview revision exhausted");
            if owner_changed
                || !extends_preview(self.rendered_preview.as_deref(), preview.as_deref())
            {
                self.preview_start_revision = revision;
            }
            self.preview_gesture = owner;
            self.preview_revision = revision;
        }
        self.rendered_preview = preview;
        let key = self.presentation_key();
        self.retry_reclaimed_render();
        while let Some((handle, result)) = self.renderer.as_mut().and_then(Executor::poll) {
            let expected = self
                .pending_frame
                .filter(|(_, pending)| *pending == handle)
                .map(|(key, _)| key);
            if self
                .pending_frame
                .is_some_and(|(_, pending)| pending == handle)
            {
                self.pending_frame = None;
            }
            match result {
                Ok(frame)
                    if expected == Some(frame.key)
                        && self.accept_render_key(frame.key)
                        && frame.device_epoch == self.gpu.as_ref().map(|gpu| gpu.epoch())
                        && (frame.fallback.is_some()
                            || frame
                                .device_epoch
                                .as_ref()
                                .is_none_or(|epoch| epoch.is_current())) =>
                {
                    if let Some(reason) = frame.fallback {
                        // A scaled-path failure need not retire a working native-
                        // resolution backend. A lost device always retires it.
                        if frame.key.4 == 0
                            || frame
                                .device_epoch
                                .as_ref()
                                .is_none_or(|epoch| !epoch.is_current())
                        {
                            self.replace_gpu(None);
                        }
                        self.editor.set_status(reason);
                    }
                    if let Some(texture) = frame.gpu_texture {
                        self.install_gpu_texture(ctx, frame.key, texture);
                    } else {
                        self.install_render(ctx, frame.key, Ok(frame.pixels));
                    }
                }
                Err(error)
                    if expected.is_some_and(|key| self.accept_render_key(key))
                        && error.code != paintcraft_core::types::ErrorCode::OperationCancelled =>
                {
                    self.install_render(ctx, key, Err(error))
                }
                _ => {} // Obsolete document/epoch results cannot replace the current canvas.
            }
        }
        if self.texture_key == Some(key) {
            return;
        }
        let keep_preview = self.pending_frame.is_some_and(|(pending_key, _)| {
            self.accept_render_key(pending_key) && self.rendered_preview.is_some()
        });
        if let Some(renderer) = &mut self.renderer {
            if let Some((pending_key, handle)) = self.pending_frame {
                if pending_key == key || keep_preview {
                    ctx.request_repaint_after(Duration::from_millis(16));
                    return;
                }
                renderer.cancel(handle);
                self.pending_frame = None;
            }
            let doc = self.editor.document_snapshot();
            let preparation = self.editor.capture_preview(self.rendered_preview.clone());
            let selection = self.editor.selection();
            let tiles = doc
                .layers
                .values()
                .map(|layer| layer.raster.resident_tiles() as u64)
                .sum::<u64>();
            let output_size = Self::display_size(key);
            let reservation = u64::from(output_size[0]) * u64::from(output_size[1]) * 16
                + tiles * paintcraft_raster::RasterLayer::TILE_BYTES as u64
                + if preparation.possible_preview() {
                    256 << 20
                } else {
                    0
                }
                + cpu_rendering::ADMISSION_BYTES;
            let reservation = reservation.saturating_add(self.gpu.as_ref().map_or(0, |gpu| {
                gpu.source_cache_admission_bytes()
                    .saturating_add(gpu.output_pool_admission_bytes())
                    .saturating_add(if key.4 != 0 {
                        gpu.scaled_scratch_admission_bytes()
                    } else {
                        0
                    })
            }));
            let native_display = self.gpu_target.is_some();
            let gpu_scaled_display = self.gpu_scaled_display;
            let gpu = self.gpu.clone();
            let cpu = self.cpu_renderer.clone();
            let wake = ctx.clone();
            match renderer.submit(
                reservation,
                move |cancel| {
                    cancel.check()?;
                    let batch = preparation.build(|| cancel.check())?;
                    let is_preview = batch.is_some();
                    let staged;
                    let doc = if let Some(batch) = batch {
                        staged = cpu.stage(&doc, selection, &batch, &|| cancel.check())?.doc;
                        &staged
                    } else {
                        cpu.clear_geometry();
                        doc.as_ref()
                    };
                    cancel.check()?;
                    let device_epoch = gpu.as_ref().map(|gpu| gpu.epoch());
                    let (pixels, gpu_texture, fallback) = if key.4 != 0 && !gpu_scaled_display {
                        (
                            cpu.render_display(doc, key.3, key.4, is_preview, || cancel.check())?,
                            None,
                            None,
                        )
                    } else if let Some(gpu) = gpu {
                        let result = if native_display {
                            let request =
                                paintcraft_render::display::DisplayRequest::new(doc, key.3, key.4)?;
                            gpu.render_display_texture(doc, request, || cancel.check())
                                .map(|texture| (Vec::new(), Some(texture)))
                        } else if key.4 == 0 {
                            gpu.render_region_with_stats(doc, key.3, || cancel.check())
                                .map(|frame| (frame.pixels, None))
                        } else {
                            // A headless readback consumer still requires straight
                            // pixels, not lossy unpremultiplication of display bytes.
                            cpu.render_display(doc, key.3, key.4, is_preview, || cancel.check())
                                .map(|pixels| (pixels, None))
                        };
                        match result {
                            Ok((pixels, texture)) => (pixels, texture, None),
                            Err(error) => {
                                cancel.check()?;
                                (
                                    cpu.render_display(doc, key.3, key.4, is_preview, || {
                                        cancel.check()
                                    })?,
                                    None,
                                    Some(if key.4 == 0 {
                                        format!("Using CPU rendering: {error}")
                                    } else {
                                        format!("Scaled display used CPU: {error}")
                                    }),
                                )
                            }
                        }
                    } else {
                        (
                            cpu.render_display(doc, key.3, key.4, is_preview, || cancel.check())?,
                            None,
                            None,
                        )
                    };
                    Ok(RenderedFrame {
                        key,
                        pixels,
                        gpu_texture,
                        fallback,
                        device_epoch,
                    })
                },
                move || wake.request_repaint(),
            ) {
                Ok(handle) => self.pending_frame = Some((key, handle)),
                Err(error)
                    if error.code == paintcraft_core::types::ErrorCode::ResourceLimit
                        && renderer.request_fits(reservation) => {} // Shared contention: retry, retain current pixels.
                Err(error) if renderer.pending() == 0 => self.install_render(ctx, key, Err(error)),
                Err(_) => {} // Retry after already-admitted work releases its reservation.
            }
            ctx.request_repaint_after(Duration::from_millis(16));
            return;
        }
        let pixels = self
            .editor
            .capture_preview(self.rendered_preview.clone())
            .build(|| Ok(()))
            .and_then(|batch| match batch {
                Some(batch) => self
                    .cpu_renderer
                    .stage(
                        self.editor.document(),
                        self.editor.selection(),
                        &batch,
                        &|| Ok(()),
                    )
                    .and_then(|staged| {
                        self.cpu_renderer
                            .render_display(&staged.doc, key.3, key.4, true, || Ok(()))
                    }),
                None => self.cpu_renderer.render_display(
                    self.editor.document(),
                    key.3,
                    key.4,
                    false,
                    || Ok(()),
                ),
            });
        self.install_render(ctx, key, pixels);
    }

    /// Replacing a backend invalidates outstanding frames even if document and
    /// preview keys are identical. Host pixels/history remain authoritative.
    pub(super) fn replace_gpu(&mut self, gpu: Option<Arc<GpuRenderer>>) {
        if let Some((_, handle)) = self.pending_frame.take()
            && let Some(renderer) = &self.renderer
        {
            renderer.cancel(handle);
        }
        self.gpu_texture = None;
        self.gpu_target = None;
        self.gpu = gpu;
        self.texture_key = None;
    }

    /// In-flight progress may be shown only within the same live gesture. A
    /// cancelled/restarted gesture or a publication retires all older previews.
    pub(super) fn accept_render_key(&self, key: RenderKey) -> bool {
        let current = self.presentation_key();
        key == current
            || (self.rendered_preview.is_some()
                && key.0 == current.0
                && key.1 == current.1
                && key.3 == current.3
                && key.4 == current.4
                && key.2 >= self.preview_start_revision
                && key.2 < current.2)
    }

    pub(super) fn install_render(
        &mut self,
        ctx: &egui::Context,
        key: RenderKey,
        pixels: paintcraft_core::types::Result<Vec<u8>>,
    ) {
        if !self.accept_render_key(key) {
            return;
        }
        self.render_residency_retry = pixels
            .as_ref()
            .err()
            .and_then(|error| error.allocation_denial())
            .filter(|denial| {
                denial.class() == paintcraft_core::types::resources::AllocationClass::DecodedTiles
                    && denial.is_possible()
            })
            .cloned()
            .map(|denial| (key, denial));
        self.update_capacity_interest();
        if let Some(wakes) = &self.budget_wakes {
            wakes.wait_for_release(self.render_residency_retry.is_some());
        }
        if self
            .render_residency_retry
            .as_ref()
            .is_some_and(|(_, denial)| denial.can_retry())
        {
            ctx.request_repaint();
        }
        self.gpu_texture = None;
        let [width, height] = Self::display_size(key);
        match pixels {
            Ok(px) if px.len() as u64 == u64::from(width) * u64::from(height) * 4 => {
                let limit = ctx.input(|input| input.max_texture_side) as u32;
                if width > limit || height > limit {
                    self.texture = None;
                    self.rendered.clear();
                    self.editor
                        .set_status("Cannot render: region exceeds display texture limits");
                    self.texture_key = Some(key);
                    return;
                }
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    &px,
                );
                match &mut self.texture {
                    Some(texture) => texture.set(image, egui::TextureOptions::NEAREST),
                    None => {
                        self.texture =
                            Some(ctx.load_texture("canvas", image, egui::TextureOptions::NEAREST))
                    }
                }
                self.rendered = px;
            }
            Ok(_) => {
                self.texture = None;
                self.rendered.clear();
                self.editor
                    .set_status("Cannot render: worker returned an invalid pixel buffer");
            }
            Err(error) => {
                self.texture = None;
                self.rendered.clear();
                self.editor.set_status(format!("Cannot render: {error}"));
            }
        }
        self.texture_key = Some(key);
    }
}

// Only logical input extensions may reuse an older in-flight preview. Removal,
// replacement, style changes and cancelled/retried plans retire the old result.
fn extends_preview(previous: Option<&Preview>, next: Option<&Preview>) -> bool {
    fn paths(preview: &Preview) -> Option<&[paintcraft_tools::Stroke]> {
        match preview {
            Preview::Path(path) => Some(std::slice::from_ref(path)),
            Preview::Paths(paths) => Some(paths),
            _ => None,
        }
    }
    let (Some(previous), Some(next)) = (previous, next) else {
        return false;
    };
    if let (Some(previous), Some(next)) = (paths(previous), paths(next)) {
        let Some(last) = previous.last() else {
            return false;
        };
        if next.len() < previous.len()
            || previous[..previous.len() - 1] != next[..previous.len() - 1]
        {
            return false;
        }
        let tail = &next[previous.len() - 1];
        return last.layer == tail.layer
            && last.point_scale == tail.point_scale
            && last.color == tail.color
            && last.size == tail.size
            && last.round == tail.round
            && last.mode == tail.mode
            && tail.points.starts_with(&last.points);
    }
    match (previous, next) {
        (
            Preview::Text {
                layer: a,
                style: sa,
                background: ba,
                at: pa,
                text: ta,
                scale: za,
                color: ca,
            },
            Preview::Text {
                layer: b,
                style: sb,
                background: bb,
                at: pb,
                text: tb,
                scale: zb,
                color: cb,
            },
        ) => {
            a == b && sa == sb && ba == bb && pa == pb && za == zb && ca == cb && tb.starts_with(ta)
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "rendering/scheduling_tests.rs"]
mod scheduling_tests;

#[cfg(test)]
#[path = "rendering/device_tests.rs"]
mod device_tests;

#[cfg(test)]
#[path = "rendering/region_tests.rs"]
mod region_tests;

#[cfg(test)]
#[path = "rendering/display_tests.rs"]
mod display_tests;
