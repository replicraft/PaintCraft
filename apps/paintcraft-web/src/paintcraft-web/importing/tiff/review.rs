//! Actual selected-page preview precedes report-bound consent and adoption.
use super::*;
use eframe::egui;
pub(super) struct Review {
    pub owner: Owner,
    held: Rc<Held>,
    prepared: Prepared,
    digest: [u8; 32],
    texture: egui::TextureHandle,
    size: egui::Vec2,
    _copy: BudgetAccount,
    shown: Option<u64>,
    consent: bool,
}
impl std::fmt::Debug for Review {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TiffReview")
            .field("digest", &self.digest)
            .finish_non_exhaustive()
    }
}
impl Review {
    pub fn new(
        ctx: &egui::Context,
        held: Rc<Held>,
        owner: Owner,
        prepared: Prepared,
    ) -> Result<Self> {
        let digest = prepared.report.digest();
        let (width, height) = prepared.document.canvas;
        if prepared.report.losses().is_empty()
            || width != prepared.report.width()
            || height != prepared.report.height()
        {
            return Err(Error::new(
                ErrorCode::CodecFailed,
                "TIFF preview extent/assessment changed",
            ));
        }
        let limit = ctx.input(|input| input.max_texture_side).clamp(1, 1024) as u64;
        let side = u64::from(width.max(height));
        let tw = (u64::from(width) * limit.min(side) / side).max(1) as usize;
        let th = (u64::from(height) * limit.min(side) / side).max(1) as usize;
        let mut copy = paintcraft_raster::Admission::new(0)
            .residency_budget()
            .account(AllocationClass::PaintMetadata);
        copy.resize(u64::from(width) * u64::from(height) * 4 + (tw * th * 8) as u64)?;
        let rgba = paintcraft_render::render_flat(&prepared.document)?;
        let mut thumbnail = vec![0; tw * th * 4];
        for y in 0..th {
            for x in 0..tw {
                let sx = x as u64 * u64::from(width) / tw as u64;
                let sy = y as u64 * u64::from(height) / th as u64;
                let from = ((sy * u64::from(width) + sx) * 4) as usize;
                let to = (y * tw + x) * 4;
                thumbnail[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
            }
        }
        let texture = ctx.load_texture(
            "web-tiff-page-preview",
            egui::ColorImage::from_rgba_unmultiplied([tw, th], &thumbnail),
            egui::TextureOptions::NEAREST,
        );
        Ok(Self {
            owner,
            held,
            prepared,
            digest,
            texture,
            size: egui::vec2(width as f32, height as f32),
            _copy: copy,
            shown: None,
            consent: false,
        })
    }
    fn preview(&mut self, ui: &mut egui::Ui) -> egui::Rect {
        let size = paintcraft_ui::import_preview::fitted_size(self.size, ui.available_size());
        let image = egui::Image::new(&self.texture)
            .maintain_aspect_ratio(false)
            .fit_to_exact_size(size);
        let image = ui.add(image);
        if size.min_elem() >= 1.0
            && ui.is_rect_visible(image.rect)
            && ui.clip_rect().contains_rect(image.rect)
        {
            self.shown.get_or_insert(ui.ctx().cumulative_frame_nr());
        }
        image.rect
    }
    fn show(&mut self, ui: &mut egui::Ui) -> bool {
        ui.label(t!(ui.ctx(), "Import preview"));
        self.preview(ui);
        let frame = ui.ctx().cumulative_frame_nr();
        let visible = self.shown.is_some_and(|shown| shown < frame);
        for loss in self.prepared.report.losses() {
            if let Some(description) = paintcraft_ui::i18n::import_loss_description(loss) {
                ui.label(t!(ui.ctx(), description));
            }
        }
        ui.add_enabled_ui(visible, |ui| {
            ui.checkbox(&mut self.consent, t!(ui.ctx(), "Accept import limitations"));
        });
        visible && self.consent
    }
}
impl WebApp {
    pub(super) fn install_tiff(
        &mut self,
        held: Rc<Held>,
        owner: Owner,
        prepared: Prepared,
        consent: Option<[u8; 32]>,
        ctx: &egui::Context,
    ) {
        let selected = prepared.selection;
        if !owner.current(self)
            || prepared.document.canvas != (prepared.report.width(), prepared.report.height())
            || prepared.report.source_bytes() != held.source.length
            || prepared.report.source_sha256() != selected.source_sha256
            || prepared.report.page_index() != selected.page.index
            || prepared.report.page_count() != selected.page_count
            || prepared.report.source_dimensions() != (selected.page.width, selected.page.height)
            || prepared.report.descriptor().orientation != selected.page.orientation
            || prepared.report.descriptor().ifd_offset != selected.page.ifd_offset
            || !match consent {
                Some(digest) => digest == prepared.report.digest(),
                None => prepared.report.losses().is_empty(),
            }
        {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        let Prepared {
            document,
            report: _,
            selection: _,
            #[cfg(target_arch = "wasm32")]
            mut lease,
        } = prepared;
        let editor = crate::editor::Editor::from_document(document);
        #[cfg(target_arch = "wasm32")]
        let released = lease.release();
        #[cfg(not(target_arch = "wasm32"))]
        let released = Ok::<_, Error>(());
        match (editor, released) {
            (Ok(mut editor), Ok(())) if owner.current(self) => {
                editor.session.mark_unsaved();
                self.install_import_editor(editor, held.source.name.clone());
                self.message.clear();
            }
            _ => self.message = t!(ctx, FAILED),
        }
    }
    pub(super) fn tiff_review_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut review) = self.tiff.review.take() else {
            return;
        };
        if !review.owner.current(self) {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        let mut accept = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::unique("web-tiff-review")).show(ctx, |ui| {
            ui.heading(t!(ui.ctx(), "Import image"));
            let ready = review.show(ui);
            ui.horizontal(|ui| {
                accept = ui
                    .add_enabled(ready, egui::Button::new(t!(ui.ctx(), "Open")))
                    .clicked();
                cancel = ui.button(t!(ui.ctx(), "Cancel")).clicked();
            });
        });
        if cancel || ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.message = t!(ctx, "Import cancelled.");
        } else if accept {
            self.install_tiff(
                review.held,
                review.owner,
                review.prepared,
                Some(review.digest),
                ctx,
            );
        } else {
            self.tiff.review = Some(review);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
