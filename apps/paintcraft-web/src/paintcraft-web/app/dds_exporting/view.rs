use super::*;
const PAUSED: &str =
    "Codec jobs are paused for this tab. Download your project, then reload to try again.";
impl WebApp {
    pub(crate) fn dds_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut review) = self.dds.review.take() else {
            return;
        };
        if !review.current(self) {
            self.dds.review = Some(review);
            self.dds.cancel();
            if self.export_confirm == Some(ImageExport::Dds) {
                self.export_confirm = None;
            }
            return;
        }
        let mut metadata_changed = false;
        let response = egui::Modal::new(egui::Id::unique("browser-dds-export")).show(ctx, |ui| {
            ui.set_max_width((ctx.content_rect().width() - 32.0).clamp(1.0, 440.0));
            ui.heading(t!(ctx, "DDS texture"));
            #[cfg(target_arch = "wasm32")]
            let paused = self.dds.service.as_ref().is_some_and(bridge::quarantined);
            #[cfg(not(target_arch = "wasm32"))]
            let paused = false;
            let changed = egui::ScrollArea::vertical()
                .id_salt("dds-export-body")
                .max_height((ctx.content_rect().height() - 96.0).max(1.0))
                .show(ui, |ui| {
            ui.label(&review.filename);
            ui.label(t!(ctx, "This browser DDS profile allows up to 4,194,304 canvas pixels and 8 layers. Larger or more complex snapshots are refused."));
            let size = review.frozen.document.canvas;
            let mut changed = controls::controls(ui, &mut review.settings, size.0, size.1);
            if review.frozen.document.source_metadata.is_some() {
                let previous = review.omit_source_metadata;
                paintcraft_ui::source_metadata_export::controls(ui, &mut review.omit_source_metadata);
                metadata_changed = previous != review.omit_source_metadata;
                changed |= metadata_changed;
            }
            if let Some(texture) = &review.texture {
                ui.label(t!(ctx, "Base image preview"));
                let image = ui.add(egui::Image::new(texture).alt_text(t!(ctx, "Base image preview")));
                if ui.clip_rect().contains_rect(image.rect) {
                    review.shown.get_or_insert(ctx.cumulative_frame_nr());
                }
            } else if let Some(error) = &review.error {
                let message = if error.code == ErrorCode::ResourceLimit {
                    "Not enough memory to export with these settings. Use a smaller image or fewer layers."
                } else if error.message.contains("nonopaque") {
                    "BC1 requires a picture without transparency. Choose RGBA8, BC2 or BC3."
                } else {
                    "DDS export failed; the current document was kept."
                };
                ui.colored_label(ui.visuals().error_fg_color, t!(ctx, message));
            } else if !review.waiting_metadata_choice() {
                ui.spinner();
                ui.label(t!(ctx, "Preparing DDS texture…"));
            }
            if paused {
                ui.label(t!(ctx, PAUSED));
                if ui.button(t!(ctx, "Download project")).clicked() {
                    self.download(ctx, false);
                }
            }
            if let Some(prepared) = &review.prepared {
                for loss in &prepared.losses {
                    let description = match loss.as_str() {
                        "flatten" => "Visible layers will be flattened into one picture.",
                        "hidden-rgb" => "Invisible color information will not be retained.",
                        dds::BLOCK_COMPRESSION_LOSS => "Block compression can change colors and transparency.",
                        dds::MIP_HIDDEN_RGB_LOSS => "Mip filtering discards invisible colors in smaller images.",
                        paintcraft_formats::export::dds::LINEAR_QUANTIZATION_LOSS => "Converting to linear 8-bit samples loses color precision.",
                        paintcraft_formats::export::metadata_policy::OMITTED_REASON => "Original source metadata is omitted from this exported copy.",
                        _ => loss.as_str(),
                    };
                    ui.label(t!(ctx, description));
                }
            }
            if review.prepared.as_ref().is_some_and(Prepared::requires_consent) {
                ui.checkbox(&mut review.approved, t!(ctx, "I accept these export changes."));
            }
            changed
                }).inner;
            let consent = review.prepared.as_ref().is_some_and(|p| !p.requires_consent() || review.approved);
            let ready = consent && !changed && !paused && review.prepared.is_some()
                && review.shown.is_some_and(|f| f < ctx.cumulative_frame_nr());
            ui.horizontal(|ui| {
                let cancel = ui.button(t!(ctx, "Cancel")).clicked();
                let accept = ui.add_enabled(ready, egui::Button::new(t!(ctx, "Download DDS"))).clicked();
                (changed, cancel, accept)
            }).inner
        });
        if ctx.current_pass_index() != 0 {
            self.dds.review = Some(review);
            return;
        }
        let (changed, cancel, accept) = response.inner;
        if cancel || response.should_close() {
            self.dds.review = Some(review);
            self.dds.cancel();
            self.export_confirm = None;
            return;
        }
        if changed {
            if let Ok(settings) = effective(review.settings, review.frozen.document.canvas) {
                review.settings = settings;
            }
            if metadata_changed {
                review.revoke_metadata_intent();
            } else {
                review.retire_preview();
            }
            ctx.request_repaint();
        }
        self.dds.review = Some(review);
        if accept {
            self.finish_dds_export(ctx);
        }
    }
    fn finish_dds_export(&mut self, ctx: &egui::Context) {
        let result = (|| {
            admit()?;
            let review = self.dds.review.as_mut().ok_or_else(stale)?;
            let prepared = review.prepared.take().ok_or_else(stale)?;
            let expected = wire::encode_settings(options(review.settings), prepared.header.size)?;
            if !review.matches_prepared(&prepared)
                || (prepared.requires_consent() && !review.approved)
                || prepared.header.settings != expected
                || review.shown.is_none_or(|f| f >= ctx.cumulative_frame_nr())
            {
                return Err(stale());
            }
            Ok(prepared)
        })();
        match result {
            Ok(prepared) => {
                if !self.dds.review.as_ref().is_some_and(|r| r.current(self)) {
                    self.dds.cancel();
                    self.export_confirm = None;
                    return;
                }
                let review = self.dds.review.take().unwrap();
                self.export_confirm = None;
                #[cfg(target_arch = "wasm32")]
                {
                    let result = Rc::new(RefCell::new(None));
                    let target = result.clone();
                    self.dds.download = Some((review.owner.clone(), result));
                    let ctx = ctx.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        *target.borrow_mut() =
                            Some(bridge::download(prepared, review.filename).await);
                        ctx.request_repaint();
                    });
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let _ = (prepared, review);
                }
                self.message = t!(ctx, "Requesting DDS download…");
            }
            Err(error) => self.message = error.to_string(),
        }
    }
}
