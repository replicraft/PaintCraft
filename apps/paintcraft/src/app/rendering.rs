//! Desktop rendering integration.

use super::*;

impl App {
    pub(super) fn presentation_key(&self) -> RenderKey {
        let (state, epoch) = self.editor.render_key();
        (state, epoch, self.preview_revision)
    }

    /// Connect native services only for a real desktop window; harnesses stay deterministic.
    pub fn with_native_services(mut self, cc: &eframe::CreationContext<'_>) -> Self {
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
            match GpuRenderer::new(state.device.clone(), state.queue.clone()) {
                Ok(gpu) => {
                    self.gpu = Some(Arc::new(gpu));
                    self.texture_key = None;
                }
                Err(e) => self.editor.set_status(format!("Using CPU rendering: {e}")),
            }
        }
        match Executor::new(WorkerLimits {
            workers: 1,
            pending_jobs: 2,
            transient_bytes: 1 << 30,
        }) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => self
                .editor
                .set_status(format!("Background renderer unavailable: {error}")),
        }
        self
    }

    pub(super) fn sync_texture(&mut self, ctx: &egui::Context) {
        let owner = self.editor.preview_token();
        if owner != self.preview_gesture {
            self.preview_start_revision = self.preview_revision + 1;
            self.preview_gesture = owner;
        }
        let preview = self.editor.preview().filter(|p| {
            matches!(
                p,
                Preview::Path(_) | Preview::Paths(_) | Preview::Text { style: Some(_), .. }
            )
        });
        if self.rendered_preview.as_deref() != preview.as_ref() {
            self.rendered_preview = preview.map(Arc::new);
            self.preview_revision = self
                .preview_revision
                .checked_add(1)
                .expect("preview revision exhausted");
        }
        let key = self.presentation_key();
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
                Ok(frame) if expected == Some(frame.key) && self.accept_render_key(frame.key) => {
                    if let Some(reason) = frame.fallback {
                        self.gpu = None;
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
                    })
                },
                move || wake.request_repaint(),
            ) {
                Ok(handle) => self.pending_frame = Some((key, handle)),
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
}
