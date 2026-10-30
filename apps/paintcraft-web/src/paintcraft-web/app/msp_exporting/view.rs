use super::*;
const PAUSED: &str =
    "Codec jobs are paused for this tab. Download your project, then reload to try again.";
impl WebApp {
    pub(crate) fn msp_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut review) = self.msp.review.take() else {
            return;
        };
        if !review.current(self) {
            self.msp.review = Some(review);
            self.retire_msp_export();
            return;
        }
        let mut metadata_changed = false;
        let response = egui::Modal::new(egui::Id::unique("browser-msp-export")).show(ctx, |ui| {
            ui.set_max_width((ctx.content_rect().width() - 32.0).clamp(1.0, 440.0));
            ui.heading(t!(ctx, "MSP picture"));
            #[cfg(target_arch = "wasm32")]
            let paused = self.msp.service.as_ref().is_some_and(bridge::quarantined);
            #[cfg(not(target_arch = "wasm32"))]
            let paused = false;
            let changed = egui::ScrollArea::vertical()
                .id_salt("msp-export-body")
                .max_height((ctx.content_rect().height() - 96.0).max(1.0))
                .show(ui, |ui| {
            ui.label(&review.filename);
            ui.label(t!(ctx, "This browser MSP2 profile allows up to 4,194,304 canvas pixels and 8 layers. Larger or more complex snapshots are refused."));
            let mut changed = controls::controls(ui, &mut review.version);
            if review.frozen.document.source_metadata.is_some() {
                let previous = review.omit_source_metadata;
                paintcraft_ui::source_metadata_export::controls(ui, &mut review.omit_source_metadata);
                metadata_changed = previous != review.omit_source_metadata;
                changed |= metadata_changed;
            }
            if review.frozen.has_filters {
                let previous = review.bake_filters;
                paintcraft_ui::filter_bake_export::controls(ui, &mut review.bake_filters);
                metadata_changed |= previous != review.bake_filters;
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
                } else if error.code == ErrorCode::CapabilityUnsupported {
                    "MSP requires opaque black or white pixels. No automatic conversion is applied."
                } else {
                    "MSP export failed; the current document was kept."
                };
                ui.colored_label(ui.visuals().error_fg_color, t!(ctx, message));
            } else if !review.waiting_metadata_choice() {
                ui.spinner();
                ui.label(t!(ctx, "Preparing MSP picture…"));
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
                        paintcraft_formats::export::metadata_policy::OMITTED_REASON => "Original source metadata is omitted from this exported copy.",
                        paintcraft_formats::export::filter_bake::BAKED_REASON => "Editable filters are baked into this exported copy.",
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
                let accept = ui.add_enabled(ready, egui::Button::new(t!(ctx, "Download MSP"))).clicked();
                (changed, cancel, accept)
            }).inner
        });
        if ctx.current_pass_index() != 0 {
            self.msp.review = Some(review);
            return;
        }
        let (changed, cancel, accept) = response.inner;
        if cancel || response.should_close() {
            self.msp.review = Some(review);
            self.retire_msp_export();
            return;
        }
        if changed {
            self.msp_version = review.version;
            if metadata_changed {
                review.revoke_metadata_intent();
            } else {
                review.retire_preview();
            }
            if review.version != Version::RowRleV2 {
                self.msp.review = Some(review);
                self.msp.cancel();
                // A version change returns to the legacy MSP1 dialog. Metadata
                // intent changes keep this review and its exact frozen copy.
                ctx.request_repaint();
                return;
            }
            ctx.request_repaint();
        }
        self.msp.review = Some(review);
        if accept {
            self.finish_msp_export(ctx);
        }
    }
    pub(super) fn finish_msp_export(&mut self, ctx: &egui::Context) {
        if !self.msp.review.as_ref().is_some_and(|r| r.current(self)) {
            self.retire_msp_export();
            return;
        }
        let result = (|| {
            admit()?;
            let review = self.msp.review.as_mut().ok_or_else(stale)?;
            if self.msp_version != Version::RowRleV2 {
                return Err(stale());
            }
            review.take_ready(ctx.cumulative_frame_nr())
        })();
        match result {
            Ok(prepared) => {
                if !self.msp.review.as_ref().is_some_and(|r| r.current(self)) {
                    self.retire_msp_export();
                    return;
                }
                let review = self.msp.review.take().unwrap();
                self.export_confirm = None;
                #[cfg(target_arch = "wasm32")]
                {
                    let result = Rc::new(RefCell::new(None));
                    let target = result.clone();
                    self.msp.download = Some((review.owner.clone(), result));
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
                self.message = t!(ctx, "Requesting MSP download…");
            }
            Err(error) => self.message = error.to_string(),
        }
    }
}
