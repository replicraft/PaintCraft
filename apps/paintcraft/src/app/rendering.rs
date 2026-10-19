//! Desktop rendering integration.

use super::*;

impl App {
    /// Connect native services only for a real desktop window; harnesses stay deterministic.
    pub fn with_native_services(mut self, cc: &eframe::CreationContext<'_>) -> Self {
        self.dialogs = Some(NativeDialogs::new(cc));
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
        let key = self.editor.render_key();
        while let Some((handle, result)) = self.renderer.as_mut().and_then(Executor::poll) {
            let expected = self.pending_frame == Some((key, handle));
            if self
                .pending_frame
                .is_some_and(|(_, pending)| pending == handle)
            {
                self.pending_frame = None;
            }
            match result {
                Ok(frame) if expected && frame.key == key => {
                    if let Some(reason) = frame.fallback {
                        self.gpu = None;
                        self.editor.set_status(reason);
                    }
                    self.install_render(ctx, key, Ok(frame.pixels));
                }
                Err(error)
                    if expected
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
        if let Some(renderer) = &mut self.renderer {
            if let Some((pending_key, handle)) = self.pending_frame {
                if pending_key == key {
                    ctx.request_repaint_after(Duration::from_millis(16));
                    return;
                }
                renderer.cancel(handle);
                self.pending_frame = None;
            }
            let doc = self.editor.document_snapshot();
            let tiles = doc
                .layers
                .values()
                .map(|layer| layer.raster.resident_tiles() as u64)
                .sum::<u64>();
            let reservation = u64::from(doc.canvas.0) * u64::from(doc.canvas.1) * 16
                + tiles * paintcraft_raster::RasterLayer::TILE_BYTES as u64;
            let gpu = self.gpu.clone();
            let wake = ctx.clone();
            match renderer.submit(
                reservation,
                move |cancel| {
                    let (pixels, fallback) = if let Some(gpu) = gpu {
                        match gpu.render_cancellable(&doc, || cancel.check()) {
                            Ok(pixels) => (pixels, None),
                            Err(error) => {
                                cancel.check()?;
                                (
                                    paintcraft_render::render_cancellable(&doc, || cancel.check())?,
                                    Some(format!("Using CPU rendering: {error}")),
                                )
                            }
                        }
                    } else {
                        (
                            paintcraft_render::render_cancellable(&doc, || cancel.check())?,
                            None,
                        )
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
        self.install_render(ctx, key, render_flat(self.editor.document()));
    }

    pub(super) fn install_render(
        &mut self,
        ctx: &egui::Context,
        key: (Uuid, u64),
        pixels: paintcraft_core::types::Result<Vec<u8>>,
    ) {
        if key != self.editor.render_key() {
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
