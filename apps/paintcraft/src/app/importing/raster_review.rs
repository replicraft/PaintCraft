//! The pending flat raster is shown before its loss consent can publish it.
use super::*;
use paintcraft_core::types::resources::{AllocationClass, BudgetAccount, SharedBudget};
const SIDE: u64 = 512;
pub(super) struct Review {
    texture: egui::TextureHandle,
    size: egui::Vec2,
    shown: Option<u64>,
    consent: bool,
    // Logical RGBA scratch/egui-image bytes, not allocator, GPU or RSS accounting.
    _copy: BudgetAccount,
}
impl Review {
    fn new(ctx: &egui::Context, image: &image::Decoded, budget: SharedBudget) -> Result<Self> {
        let (width, height) = (image.width, image.height);
        if width == 0
            || height == 0
            || u64::from(width)
                .checked_mul(u64::from(height))
                .and_then(|n| n.checked_mul(4))
                != Some(image.rgba.len() as u64)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Raster preview sample extent",
            ));
        }
        let limit = (ctx.input(|input| input.max_texture_side) as u64).clamp(1, SIDE);
        let side = u64::from(width.max(height));
        let tw = (u64::from(width) * limit.min(side) / side).max(1) as usize;
        let th = (u64::from(height) * limit.min(side) / side).max(1) as usize;
        let mut copy = budget.account(AllocationClass::PaintMetadata);
        copy.resize((tw * th * 8) as u64)?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(tw * th * 4)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Raster preview allocation"))?;
        rgba.resize(tw * th * 4, 0);
        for y in 0..th {
            for x in 0..tw {
                let from = ((y as u64 * u64::from(height) / th as u64) * u64::from(width)
                    + x as u64 * u64::from(width) / tw as u64)
                    * 4;
                rgba[(y * tw + x) * 4..(y * tw + x + 1) * 4]
                    .copy_from_slice(&image.rgba[from as usize..from as usize + 4]);
            }
        }
        let texture = ctx.load_texture(
            "native-stock-raster-preview",
            egui::ColorImage::from_rgba_unmultiplied([tw, th], &rgba),
            egui::TextureOptions::NEAREST,
        );
        Ok(Self {
            texture,
            size: egui::vec2(width as f32, height as f32),
            shown: None,
            consent: false,
            _copy: copy,
        })
    }
    pub(super) fn ready(&self, ctx: &egui::Context) -> bool {
        self.consent
            && self
                .shown
                .is_some_and(|frame| frame < ctx.cumulative_frame_nr())
    }
    fn preview(&mut self, ui: &mut egui::Ui) -> egui::Rect {
        let size = paintcraft_ui::import_preview::fitted_size(self.size, ui.available_size());
        let image = ui.add(
            egui::Image::new(&self.texture)
                .maintain_aspect_ratio(false)
                .fit_to_exact_size(size),
        );
        if size.min_elem() >= 1.0
            && ui.is_rect_visible(image.rect)
            && ui.clip_rect().contains_rect(image.rect)
            && self.shown.is_none()
        {
            self.shown = Some(ui.ctx().cumulative_frame_nr());
            ui.ctx().request_repaint();
        }
        image.rect
    }
    fn show(&mut self, ui: &mut egui::Ui, losses: &[String]) -> bool {
        ui.label(t!(ui.ctx(), "Import preview"));
        self.preview(ui);
        for loss in losses {
            if let Some(description) = import_loss_description(loss) {
                ui.label(t!(ui.ctx(), description));
            }
        }
        let visible = self
            .shown
            .is_some_and(|frame| frame < ui.ctx().cumulative_frame_nr());
        ui.add_enabled_ui(visible, |ui| {
            ui.checkbox(&mut self.consent, t!(ui.ctx(), "Accept import limitations"));
        });
        self.ready(ui.ctx())
    }
}
impl App {
    pub(super) fn raster_import_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.pending_import.take() else {
            return;
        };
        if !pending.current(self) {
            self.confirm = None;
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let Candidate::Raster(image) = &pending.candidate else {
            self.pending_import = Some(pending);
            return;
        };
        if pending.review.is_none() {
            let budget = self.source_metadata_budget();
            match Review::new(ctx, image, budget) {
                Ok(review) => pending.review = Some(review),
                Err(error) => {
                    self.confirm = None;
                    self.editor.set_status(error.to_string());
                    return;
                }
            }
        }
        let (mut accept, mut cancel) = (false, false);
        egui::Modal::new(egui::Id::unique("native-import-losses")).show(ctx, |ui| {
            ui.heading(t!(ctx, "Open with these import limitations?"));
            let ready = pending.review.as_mut().unwrap().show(ui, &image.losses);
            ui.horizontal(|ui| {
                accept = ui
                    .add_enabled(ready, egui::Button::new(t!(ctx, "Open image")))
                    .clicked();
                cancel = ui.button(t!(ctx, "Cancel")).clicked();
            });
        });
        self.pending_import = Some(pending);
        if ctx.current_pass_index() == 0
            && (accept || cancel || ctx.input(|input| input.key_pressed(egui::Key::Escape)))
        {
            self.resolve_import(ctx, accept);
        }
    }
}
#[cfg(test)]
#[path = "raster_review/tests.rs"]
pub(crate) mod tests;
