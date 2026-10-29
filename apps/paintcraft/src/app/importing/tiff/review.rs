//! Consent is tied to a retained selected-page preview, never a page number alone.
use super::*;
pub(super) struct Review {
    texture: egui::TextureHandle,
    size: egui::Vec2,
    shown: Option<u64>,
    consent: bool,
    _copy: BudgetAccount,
}
impl Review {
    pub(super) fn new(ctx: &egui::Context, prepared: &Prepared) -> Result<Self> {
        if prepared
            .thumbnail_size
            .iter()
            .any(|side| *side > ctx.input(|input| input.max_texture_side))
        {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "TIFF preview texture allowance",
            ));
        }
        let mut copy = prepared
            .preview_budget
            .account(paintcraft_core::types::resources::AllocationClass::PaintMetadata);
        copy.resize(prepared.thumbnail.len() as u64)?;
        let texture = ctx.load_texture(
            "tiff-selected-page-preview",
            egui::ColorImage::from_rgba_unmultiplied(prepared.thumbnail_size, &prepared.thumbnail),
            egui::TextureOptions::NEAREST,
        );
        Ok(Self {
            texture,
            size: egui::vec2(
                prepared.document.canvas.0 as f32,
                prepared.document.canvas.1 as f32,
            ),
            shown: None,
            consent: false,
            _copy: copy,
        })
    }
    #[cfg(test)]
    pub(super) fn consent_for_test(&mut self) {
        self.consent = true;
    }
    pub(super) fn ready(&self, ctx: &egui::Context) -> bool {
        self.consent
            && self
                .shown
                .is_some_and(|shown| shown < ctx.cumulative_frame_nr())
    }
    pub(super) fn preview(&mut self, ui: &mut egui::Ui) -> egui::Rect {
        let size = paintcraft_ui::import_preview::fitted_size(self.size, ui.available_size());
        let response = ui.add(
            egui::Image::new(&self.texture)
                .maintain_aspect_ratio(false)
                .fit_to_exact_size(size),
        );
        if response.rect.width() >= 1.0
            && response.rect.height() >= 1.0
            && ui.is_rect_visible(response.rect)
            && ui.clip_rect().contains_rect(response.rect)
        {
            self.shown.get_or_insert(ui.ctx().cumulative_frame_nr());
        }
        response.rect
    }
    pub(super) fn show(&mut self, ui: &mut egui::Ui, prepared: &Prepared) -> bool {
        let ctx = ui.ctx().clone();
        ui.label(t!(
            &ctx,
            "Page { $number }: { $width } × { $height } px",
            number = u32::from(prepared.assessment.page_index()) + 1,
            width = prepared.assessment.width(),
            height = prepared.assessment.height()
        ));
        ui.label(t!(&ctx, "Import preview"));
        self.preview(ui);
        let visible = self
            .shown
            .is_some_and(|shown| shown < ctx.cumulative_frame_nr());
        for loss in &prepared.losses {
            if let Some(description) = import_loss_description(loss) {
                ui.label(t!(&ctx, description));
            }
        }
        ui.add_enabled_ui(visible, |ui| {
            ui.checkbox(&mut self.consent, t!(&ctx, "Accept import limitations"));
        });
        self.ready(&ctx)
    }
}
