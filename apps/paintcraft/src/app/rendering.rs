//! Desktop rendering integration.

use super::*;

impl App {
    pub(super) fn presentation_key(&self) -> RenderKey {
        let (state, epoch) = self.editor.render_key();
        (state, epoch, self.preview_revision)
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
        if let Some(state) = &cc.wgpu_render_state {
            let epoch = paintcraft_render::gpu::DeviceEpoch::default();
            let lost = epoch.clone();
            let wake = cc.egui_ctx.clone();
            state.device.set_device_lost_callback(move |_, _| {
                lost.retire();
                wake.request_repaint();
            });
            match GpuRenderer::with_epoch(state.device.clone(), state.queue.clone(), epoch) {
                Ok(gpu) => {
                    self.replace_gpu(Some(Arc::new(gpu)));
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
                        self.replace_gpu(None);
                        self.editor.set_status(reason);
                    }
                    self.install_render(ctx, frame.key, Ok(frame.pixels));
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
            let reservation = u64::from(doc.canvas.0) * u64::from(doc.canvas.1) * 16
                + tiles * paintcraft_raster::RasterLayer::TILE_BYTES as u64
                + if preparation.possible_preview() {
                    256 << 20
                } else {
                    0
                }
                + cpu_rendering::ADMISSION_BYTES;
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
                        staged = paintcraft_editor::execute::execute(&doc, selection, &batch)?.doc;
                        &staged
                    } else {
                        doc.as_ref()
                    };
                    cancel.check()?;
                    let device_epoch = gpu.as_ref().map(|gpu| gpu.epoch());
                    let (pixels, fallback) = if let Some(gpu) = gpu {
                        match gpu.render_cancellable(doc, || cancel.check()) {
                            Ok(pixels) => (pixels, None),
                            Err(error) => {
                                cancel.check()?;
                                (
                                    cpu.render(doc, is_preview, || cancel.check())?,
                                    Some(format!("Using CPU rendering: {error}")),
                                )
                            }
                        }
                    } else {
                        (cpu.render(doc, is_preview, || cancel.check())?, None)
                    };
                    Ok(RenderedFrame {
                        key,
                        pixels,
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
                Some(batch) => paintcraft_editor::execute::execute(
                    self.editor.document(),
                    self.editor.selection(),
                    &batch,
                )
                .and_then(|staged| self.cpu_renderer.render(&staged.doc, true, || Ok(()))),
                None => self
                    .cpu_renderer
                    .render(self.editor.document(), false, || Ok(())),
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
        self.gpu = gpu;
        self.texture_key = None;
    }

    /// In-flight progress may be shown only within the same live gesture. A
    /// cancelled/restarted gesture or a publication retires all older previews.
    fn accept_render_key(&self, key: RenderKey) -> bool {
        let current = self.presentation_key();
        key == current
            || (self.rendered_preview.is_some()
                && key.0 == current.0
                && key.1 == current.1
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
        let doc = self.editor.document();
        match pixels {
            Ok(px) if px.len() as u64 == u64::from(doc.canvas.0) * u64::from(doc.canvas.1) * 4 => {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [doc.canvas.0 as usize, doc.canvas.1 as usize],
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
mod scheduling_tests {
    use super::*;
    #[test]
    fn moving_brush_preserves_inflight_progress_and_then_renders_the_latest_samples() {
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        app.editor.select_tool(Tool::Brush);
        app.editor.pointer_down((10, 10));
        app.sync_texture(&ctx);
        let old = app.presentation_key();
        let pixels = app.rendered.clone();
        let (send, wait) = std::sync::mpsc::channel();
        let mut worker = Executor::new(WorkerLimits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 512 << 20,
        })
        .unwrap();
        let handle = worker
            .submit(
                1024,
                move |_| {
                    wait.recv().unwrap();
                    Ok(RenderedFrame {
                        key: old,
                        pixels,
                        fallback: None,
                        device_epoch: None,
                    })
                },
                || {},
            )
            .unwrap();
        app.renderer = Some(worker);
        app.pending_frame = Some((old, handle));
        app.texture_key = None;
        for i in 1..40 {
            app.editor.pointer_move((10 + i, 10));
            app.sync_texture(&ctx);
            assert_eq!(app.pending_frame, Some((old, handle)));
        }
        send.send(()).unwrap();
        let current = app.presentation_key();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while app.texture_key != Some(current) {
            assert!(std::time::Instant::now() < deadline);
            app.sync_texture(&ctx);
            std::thread::yield_now();
        }
        let staged = paintcraft_editor::execute::execute(
            app.editor.document(),
            app.editor.selection(),
            &app.editor.preview_batch().unwrap().unwrap(),
        )
        .unwrap();
        assert_eq!(app.rendered, render_flat(&staged.doc).unwrap());
    }
    #[test]
    fn idle_repaints_reuse_the_owned_preview_even_after_an_equal_value_refresh() {
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        app.editor.pointer_down((1, 1));
        app.editor.pointer_move((2, 1));
        app.sync_texture(&ctx);
        let first = app.rendered_preview.clone().unwrap();
        let revision = app.preview_revision;
        app.sync_texture(&ctx);
        assert!(Arc::ptr_eq(&first, app.rendered_preview.as_ref().unwrap()));
        assert_eq!(app.preview_revision, revision);
        // This changes the source stamp, but the gesture captured its own tool.
        app.editor.tool = Tool::Brush;
        app.sync_texture(&ctx);
        let refreshed = app.rendered_preview.clone().unwrap();
        assert_eq!(first, refreshed);
        assert_eq!(app.preview_revision, revision);
        app.sync_texture(&ctx);
        assert!(Arc::ptr_eq(
            &refreshed,
            app.rendered_preview.as_ref().unwrap()
        ));
        app.editor.pointer_move((3, 1));
        app.sync_texture(&ctx);
        assert!(app.preview_revision > revision);
        assert_ne!(first, app.rendered_preview.clone().unwrap());
    }
    #[test]
    fn native_preview_capture_and_caret_lookup_do_not_wait_for_a_busy_font_rasterizer() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        app.editor.select_tool(Tool::Text);
        app.editor.pointer_down((8, 8));
        app.editor.type_text("Hello");
        app.renderer = Some(
            Executor::new(WorkerLimits {
                workers: 1,
                pending_jobs: 2,
                transient_bytes: 1 << 30,
            })
            .unwrap(),
        );
        let fonts = app.text_renderer.clone();
        let (started, wait_started) = mpsc::sync_channel(1);
        let (release, wait_release) = mpsc::channel();
        let forced = Arc::new(AtomicBool::new(false));
        let timed_out = forced.clone();
        let holder = std::thread::spawn(move || {
            let _guard = fonts.lock().unwrap();
            started.send(()).unwrap();
            if wait_release.recv_timeout(Duration::from_secs(8)).is_err() {
                timed_out.store(true, Ordering::SeqCst);
            }
        });
        wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
        // The old UI-side batch builder waited for this same mutex before it
        // could even submit a render job. This call must return with it held.
        app.sync_texture(&ctx);
        assert!(app.pending_frame.is_some());
        assert!(
            !forced.load(Ordering::SeqCst),
            "UI waited for font rasterization"
        );
        assert!(
            app.editor
                .cached_font_text_mask("Hello", &app.settings.text, (8, 8))
                .is_none()
        );
        app.editor.type_text(" Ω");
        app.sync_texture(&ctx);
        release.send(()).unwrap();
        holder.join().unwrap();
        let current = app.presentation_key();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while app.texture_key != Some(current) {
            assert!(std::time::Instant::now() < deadline);
            app.sync_texture(&ctx);
            std::thread::yield_now();
        }
        let staged = paintcraft_editor::execute::execute(
            app.editor.document(),
            app.editor.selection(),
            &app.editor.preview_batch().unwrap().unwrap(),
        )
        .unwrap();
        assert_eq!(app.rendered, render_flat(&staged.doc).unwrap());
        assert_eq!(app.editor.publications(), 0);
    }
    fn delayed_frame(
        app: &mut App,
        pixels: Vec<u8>,
    ) -> (RenderKey, JobHandle, std::sync::mpsc::Sender<()>) {
        let old = app.presentation_key();
        let (release, wait) = std::sync::mpsc::channel();
        let (started, wait_started) = std::sync::mpsc::sync_channel(1);
        let mut renderer = Executor::new(WorkerLimits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 1 << 30,
        })
        .unwrap();
        let handle = renderer
            .submit(
                1024,
                move |_| {
                    started.send(()).unwrap();
                    wait.recv_timeout(Duration::from_secs(10)).unwrap();
                    Ok(RenderedFrame {
                        key: old,
                        pixels,
                        fallback: None,
                        device_epoch: None,
                    })
                },
                || {},
            )
            .unwrap();
        wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
        app.renderer = Some(renderer);
        app.pending_frame = Some((old, handle));
        app.texture_key = None;
        (old, handle, release)
    }
    fn finish_current(app: &mut App, ctx: &egui::Context) {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let current = app.presentation_key();
        while app.texture_key != Some(current) {
            assert!(std::time::Instant::now() < deadline);
            app.sync_texture(ctx);
            std::thread::yield_now();
        }
    }
    #[test]
    fn deleting_text_retires_inflight_longer_text_instead_of_treating_it_as_progress() {
        let ctx = egui::Context::default();
        let mut app = App::new(None);
        app.editor.select_tool(Tool::Text);
        app.editor.pointer_down((8, 8));
        app.editor.type_text("Hello Ω");
        app.sync_texture(&ctx);
        let pixels = app.rendered.clone();
        let (old, handle, release) = delayed_frame(&mut app, pixels);
        app.editor.backspace();
        app.sync_texture(&ctx);
        assert!(!app.accept_render_key(old));
        assert_ne!(app.pending_frame, Some((old, handle)));
        release.send(()).unwrap();
        finish_current(&mut app, &ctx);
        let staged = paintcraft_editor::execute::execute(
            app.editor.document(),
            app.editor.selection(),
            &app.editor.preview_batch().unwrap().unwrap(),
        )
        .unwrap();
        assert_eq!(app.rendered, render_flat(&staged.doc).unwrap());
    }
    #[test]
    fn cancelling_and_retrying_identical_queued_paint_retires_the_old_owner_even_with_equal_pixels()
    {
        let ctx = egui::Context::default();
        let mut app = App::new(None);
        app.initialize_editing(ctx.clone());
        let gate = app.editing.as_ref().unwrap().pause();
        let stroke = paintcraft_tools::Stroke {
            layer: app.editor.target().unwrap(),
            points: vec![(3, 3)],
            point_scale: 1,
            color: Rgba([255, 0, 0, 128]),
            size: 1,
            round: false,
            mode: paintcraft_tools::BlendMode::SourceOver,
        };
        app.editor.request(101, Action::Stroke(stroke.clone()));
        app.editor.run_pending();
        app.sync_texture(&ctx);
        let pixels = app.rendered.clone();
        let (old, handle, release) = delayed_frame(&mut app, pixels);
        app.editor.cancel_pending_edits();
        app.editor.request(102, Action::Stroke(stroke));
        app.editor.run_pending();
        app.sync_texture(&ctx);
        assert!(!app.accept_render_key(old));
        assert_ne!(app.pending_frame, Some((old, handle)));
        release.send(()).unwrap();
        finish_current(&mut app, &ctx);
        assert_eq!(
            &app.rendered[(3 * 1000 + 3) * 4..][..4],
            &[255, 127, 127, 255]
        );
        app.editor.cancel_pending_edits();
        gate.send(()).unwrap();
    }
}

#[cfg(test)]
#[path = "rendering/device_tests.rs"]
mod device_tests;
