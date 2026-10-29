//! Bounded preview of the exact pending flat document, before visible consent.
use eframe::egui;
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{AllocationClass, BudgetAccount, SharedBudget},
};
use paintcraft_document::Document;
use paintcraft_ui::{i18n::import_loss_description, t};
const SIDE: u64 = 512;

pub(super) struct Review {
    texture: egui::TextureHandle,
    size: egui::Vec2,
    shown: Option<u64>,
    consent: bool,
    // Our handle drops before this logical RGBA scratch/egui-image declaration.
    // The framework may retain other copies; this is not a GPU or RSS cap.
    _copy: BudgetAccount,
}
impl std::fmt::Debug for Review {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RasterReview")
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}
impl Review {
    pub(super) fn new(ctx: &egui::Context, document: &Document) -> Result<Self> {
        let (width, height) = document.canvas;
        let invalid = || {
            Error::new(
                ErrorCode::InputSchema,
                "Raster preview requires a flat full-canvas picture",
            )
        };
        if width == 0 || height == 0 || document.roots.len() != 1 || document.layers.len() != 1 {
            return Err(invalid());
        }
        let layer = document
            .layers
            .get(&document.roots[0])
            .ok_or_else(invalid)?;
        if layer.origin != (0, 0)
            || !layer.visible
            || layer.opacity != 255
            || layer.raster.width() != width
            || layer.raster.height() != height
        {
            return Err(invalid());
        }
        let limit = (ctx.input(|input| input.max_texture_side) as u64).clamp(1, SIDE);
        let side = u64::from(width.max(height));
        let tw = (u64::from(width) * limit.min(side) / side).max(1) as usize;
        let th = (u64::from(height) * limit.min(side) / side).max(1) as usize;
        let mut copy = SharedBudget::new(SIDE * SIDE * 8).account(AllocationClass::PaintMetadata);
        copy.resize((tw * th * 8) as u64)?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(tw * th * 4)
            .map_err(|_| Error::new(ErrorCode::ResourceLimit, "Raster preview allocation"))?;
        rgba.resize(tw * th * 4, 0);
        for y in 0..th {
            for x in 0..tw {
                let sx = (x as u64 * u64::from(width) / tw as u64) as i64;
                let sy = (y as u64 * u64::from(height) / th as u64) as i64;
                rgba[(y * tw + x) * 4..(y * tw + x + 1) * 4]
                    .copy_from_slice(&layer.raster.get(sx, sy).0);
            }
        }
        let texture = ctx.load_texture(
            "web-stock-raster-preview",
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
    pub(super) fn show(&mut self, ui: &mut egui::Ui, losses: &[String]) -> bool {
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
#[cfg(test)]
pub(crate) mod tests;
