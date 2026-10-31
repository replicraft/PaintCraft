//! A retained, admitted picture preview binds loss consent to one worker result.
use super::*;
use paintcraft_core::types::resources::{AllocationClass, BudgetAccount, SharedBudget};

pub(super) struct Review {
    texture: egui::TextureHandle,
    _copy: BudgetAccount,
    size: egui::Vec2,
    shown: Option<u64>,
    consent: bool,
    pub(super) float_digest: Option<[u8; 32]>,
    pub(super) jxl_digest: Option<[u8; 32]>,
}
impl Review {
    pub(super) fn new(
        ctx: &egui::Context,
        prepared: &PreparedImport,
        budget: SharedBudget,
    ) -> Result<Self> {
        let mut review = Self::from_document(ctx, prepared.document(), budget)?;
        review.float_digest = prepared.float_assessment().map(|value| value.digest());
        if prepared.jxl_integer_assessment().is_some() {
            review.jxl_digest = Some(prepared.jxl_integer_assessment_digest()?);
        }
        Ok(review)
    }
    fn from_document(
        ctx: &egui::Context,
        doc: &paintcraft_document::Document,
        budget: SharedBudget,
    ) -> Result<Self> {
        let (width, height) = doc.canvas;
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "Codec preview extent overflow"))?;
        if pixels == 0 || pixels > wire::PIXELS_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "Codec preview exceeds pixel allowance",
            ));
        }
        let limit = ctx.input(|input| input.max_texture_side).clamp(1, 1024) as u32;
        let (tw, th) = thumbnail_extent(width, height, limit);
        let thumbnail_bytes = u64::from(tw) * u64::from(th) * 4;
        // Declare full RGBA, bounded thumbnail, and ColorImage copy before any
        // allocation. Retain through review; driver memory/hard RSS are excluded.
        let mut copy = budget.account(AllocationClass::PaintMetadata);
        copy.resize(pixels * 4 + thumbnail_bytes * 2)?;
        let rgba =
            crate::spatial_workspace::with_job(crate::spatial_workspace::needed(doc), || {
                paintcraft_render::render_flat(doc)
            })?;
        let mut thumbnail = vec![0; thumbnail_bytes as usize];
        for y in 0..th {
            for x in 0..tw {
                let sx = u64::from(x) * u64::from(width) / u64::from(tw);
                let sy = u64::from(y) * u64::from(height) / u64::from(th);
                let src = ((sy * u64::from(width) + sx) * 4) as usize;
                let dst = ((u64::from(y) * u64::from(tw) + u64::from(x)) * 4) as usize;
                thumbnail[dst..dst + 4].copy_from_slice(&rgba[src..src + 4]);
            }
        }
        let texture = ctx.load_texture(
            "native-codec-preview",
            egui::ColorImage::from_rgba_unmultiplied([tw as usize, th as usize], &thumbnail),
            egui::TextureOptions::NEAREST,
        );
        Ok(Self {
            texture,
            _copy: copy,
            size: egui::vec2(width as f32, height as f32),
            shown: None,
            consent: false,
            float_digest: None,
            jxl_digest: None,
        })
    }
    pub(super) fn show(&mut self, ui: &mut egui::Ui, prepared: &PreparedImport) -> bool {
        let ctx = ui.ctx().clone();
        let visible_before = self.preview(ui);
        if let Some(assessment) = prepared.jxr_assessment() {
            for (label, resolution) in [
                ("Horizontal", assessment.resolution_x()),
                ("Vertical", assessment.resolution_y()),
            ] {
                ui.horizontal(|ui| {
                    ui.label(t!(&ctx, label));
                    if resolution.inferred_96 {
                        ui.label(t!(&ctx, "Default (96 dpi)"));
                    } else {
                        ui.label(format!("{} dpi", f32::from_bits(resolution.effective_bits)));
                    }
                });
            }
        }
        let avif_names = prepared
            .avif_assessment()
            .map(|value| (value.primary_name(), value.alpha_name()))
            .or_else(|| {
                prepared
                    .avif_sdr_assessment()
                    .map(|value| (value.primary_name(), value.alpha_name()))
            });
        if let Some((primary, alpha)) = avif_names {
            for (label, name) in [("Image name", Some(primary)), ("Transparency name", alpha)] {
                if let Some(name) = name.filter(|name| !name.is_empty()) {
                    ui.horizontal(|ui| {
                        ui.label(t!(&ctx, label));
                        ui.add(egui::Label::new(name).truncate());
                    });
                }
            }
        }
        for loss in prepared.losses() {
            if let Some(description) = import_loss_description(loss) {
                ui.label(t!(&ctx, description));
            }
        }

        ui.add_enabled_ui(visible_before, |ui| {
            ui.checkbox(&mut self.consent, t!(&ctx, "Accept import limitations"));
        });
        visible_before && self.consent
    }
    fn preview(&mut self, ui: &mut egui::Ui) -> bool {
        let ctx = ui.ctx().clone();
        ui.label(t!(&ctx, "Import preview"));
        let size = self.size * (200.0 / self.size.x.max(self.size.y)).min(1.0);
        let response = ui.add(egui::Image::new(&self.texture).fit_to_exact_size(size));
        let frame = ctx.cumulative_frame_nr();
        if ui.is_rect_visible(response.rect) {
            self.shown.get_or_insert(frame);
        }
        self.shown.is_some_and(|shown| shown < frame)
    }
}

fn thumbnail_extent(width: u32, height: u32, limit: u32) -> (u32, u32) {
    let side = width.max(height);
    if side <= limit {
        return (width, height);
    }
    (
        (u64::from(width) * u64::from(limit) / u64::from(side)).max(1) as u32,
        (u64::from(height) * u64::from(limit) / u64::from(side)).max(1) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_admission_is_retained_until_drop_and_denied_before_copy() {
        let ctx = egui::Context::default();
        let editor = Editor::new_document(4, 3).unwrap();
        let small = SharedBudget::new(143);
        assert!(Review::from_document(&ctx, editor.document(), small.clone()).is_err());
        assert_eq!(small.usage().reserved, 0);
        let budget = SharedBudget::new(144);
        let review = Review::from_document(&ctx, editor.document(), budget.clone()).unwrap();
        assert_eq!(budget.usage().reserved, 144);
        drop(review);
        assert_eq!(budget.usage().reserved, 0);
    }
    #[test]
    fn thin_wide_document_uses_bounded_texture_without_changing_document() {
        let ctx = egui::Context::default();
        let editor = Editor::new_document(65536, 64).unwrap();
        let before = editor.document_snapshot();
        let budget = SharedBudget::new(32 << 20);
        let preview = Review::from_document(&ctx, editor.document(), budget.clone()).unwrap();
        assert_eq!(preview.texture.size(), [1024, 1]);
        assert_eq!(editor.document().canvas, (65536, 64));
        assert!(Arc::ptr_eq(&before, &editor.document_snapshot()));
        assert_eq!(budget.usage().reserved, 65536 * 64 * 4 + 1024 * 8);
        drop(preview);
        assert_eq!(budget.usage().reserved, 0);
    }
    #[test]
    fn preview_needs_an_actually_visible_previous_frame() {
        let ctx = egui::Context::default();
        let editor = Editor::new_document(4, 3).unwrap();
        let mut review =
            Review::from_document(&ctx, editor.document(), SharedBudget::new(144)).unwrap();
        let mut ready = false;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.set_clip_rect(egui::Rect::NOTHING);
            ready = review.preview(ui);
        });
        output.textures_delta.clear();
        assert!(!ready);
        assert_eq!(review.shown, None);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ready = review.preview(ui);
        });
        output.textures_delta.clear();
        assert!(!ready);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ready = review.preview(ui);
        });
        output.textures_delta.clear();
        assert!(ready);
    }
}
