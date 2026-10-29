//! Retain the isolated result and its lease until visible precision consent.
use super::*;
use eframe::egui;
use paintcraft_core::types::resources::{AllocationClass, BudgetAccount};

pub(super) struct Review {
    pub owner: Owner,
    prepared: Prepared,
    digest: [u8; 32],
    texture: egui::TextureHandle,
    _copy: BudgetAccount,
    size: egui::Vec2,
    shown: Option<u64>,
    consent: bool,
}
impl std::fmt::Debug for Review {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JxlReview")
            .field("digest", &self.digest)
            .finish_non_exhaustive()
    }
}
impl Prepared {
    fn assessment_digest(&self) -> Result<[u8; 32]> {
        Ok(Sha256::digest(self.report.encode()?).into())
    }
}
impl Review {
    pub fn new(ctx: &egui::Context, owner: Owner, prepared: Prepared) -> Result<Self> {
        let report = prepared.report;
        let digest = prepared.assessment_digest()?;
        if report.source_depth != 16 || prepared.document.canvas != (report.width, report.height) {
            return Err(Error::new(
                ErrorCode::CodecFailed,
                "Integer JXL review contract changed",
            ));
        }
        let (width, height) = prepared.document.canvas;
        let limit = ctx.input(|input| input.max_texture_side).clamp(1, 1024) as u64;
        let side = u64::from(width.max(height));
        let tw = (u64::from(width) * limit.min(side) / side).max(1) as usize;
        let th = (u64::from(height) * limit.min(side) / side).max(1) as usize;
        let thumbnail_bytes = tw * th * 4;
        // Retained logical allocation declaration, not GPU memory/RSS proof.
        let budget = paintcraft_raster::Admission::new(0).residency_budget();
        let mut copy = budget.account(AllocationClass::PaintMetadata);
        copy.resize(u64::from(width) * u64::from(height) * 4 + thumbnail_bytes as u64 * 2)?;
        let rgba = paintcraft_render::render_flat(&prepared.document)?;
        let mut thumbnail = Vec::new();
        thumbnail
            .try_reserve_exact(thumbnail_bytes)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "JXL preview allocation"))?;
        thumbnail.resize(thumbnail_bytes, 0);
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
            "web-jxl-preview",
            egui::ColorImage::from_rgba_unmultiplied([tw, th], &thumbnail),
            egui::TextureOptions::NEAREST,
        );
        Ok(Self {
            owner,
            prepared,
            digest,
            texture,
            _copy: copy,
            size: egui::vec2(width as f32, height as f32),
            shown: None,
            consent: false,
        })
    }
    fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let ctx = ui.ctx().clone();
        ui.label(t!(&ctx, "Import preview"));
        let size = self.size * (200.0 / self.size.x.max(self.size.y)).min(1.0);
        let response = ui.add(egui::Image::new(&self.texture).fit_to_exact_size(size));
        let frame = ctx.cumulative_frame_nr();
        if ui.is_rect_visible(response.rect) {
            self.shown.get_or_insert(frame);
        }
        let visible_before = self.shown.is_some_and(|shown| shown < frame);
        for loss in self.prepared.report.losses() {
            if let Some(description) = paintcraft_ui::i18n::import_loss_description(loss) {
                ui.label(t!(&ctx, description));
            }
        }
        ui.add_enabled_ui(visible_before, |ui| {
            ui.checkbox(&mut self.consent, t!(&ctx, "Accept import limitations"));
        });
        visible_before && self.consent
    }
}
impl WebApp {
    pub(super) fn install_jxl(
        &mut self,
        owner: Owner,
        prepared: Prepared,
        consent: Option<[u8; 32]>,
        ctx: &egui::Context,
    ) {
        if !owner.current(self) {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        let Ok(digest) = prepared.assessment_digest() else {
            self.message = t!(ctx, FAILED);
            return;
        };
        let consent_matches = match consent {
            Some(accepted) => accepted == digest,
            None => prepared.report.source_depth == 8,
        };
        if prepared.document.canvas != (prepared.report.width, prepared.report.height)
            || !consent_matches
        {
            self.message = t!(ctx, FAILED);
            return;
        }
        // Resident tiles are admitted before retiring the JS private-result lease.
        let Prepared {
            document,
            name,
            report: _,
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
                self.install_import_editor(editor, name);
                self.message = t!(ctx, "JPEG XL imported as an editable project copy.");
            }
            _ => self.message = t!(ctx, FAILED),
        }
    }
    pub(super) fn jxl_review_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut review) = self.jxl.review.take() else {
            return;
        };
        if !review.owner.current(self) {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        let mut cancel = false;
        let mut accept = false;
        egui::Modal::new(egui::Id::unique("web-jxl-review")).show(ctx, |ui| {
            ui.heading(t!(ui.ctx(), "Importing JPEG XL"));
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
            self.install_jxl(review.owner, review.prepared, Some(review.digest), ctx);
        } else {
            self.jxl.review = Some(review);
        }
    }
}

#[cfg(test)]
mod tests;
