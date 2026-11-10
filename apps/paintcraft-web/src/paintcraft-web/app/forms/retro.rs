//! Retro modal contents stage existing host drafts; only the parent adopts them.
use super::*;
impl WebApp {
    pub(super) fn retro_canvas_form(
        ctx: &egui::Context,
        draft: &mut CanvasDraft,
    ) -> paintcraft_classic::canvas_size::CanvasResponse {
        use paintcraft_classic::canvas_size::CanvasResponse;
        let mut result = CanvasResponse::Pending;
        let modal = egui::Modal::new(egui::Id::unique("web-retro-canvas-form"))
            .frame(paintcraft_retro::frame())
            .show(ctx, |ui| {
                paintcraft_retro::with_style(ui, |ui| {
                    ui.heading(t!(ui.ctx(), "Canvas size"));
                    if let Some(name) = &draft.properties {
                        ui.label(name);
                    }
                    ui.horizontal(|ui| {
                        let width = ui.label(t!(ui.ctx(), "Width"));
                        ui.add(
                            egui::DragValue::new(&mut draft.view.size.0)
                                .range(1..=paintcraft_document::CANVAS_SIDE_MAX),
                        )
                        .labelled_by(width.id);
                        let height = ui.label(t!(ui.ctx(), "Height"));
                        ui.add(
                            egui::DragValue::new(&mut draft.view.size.1)
                                .range(1..=paintcraft_document::CANVAS_SIDE_MAX),
                        )
                        .labelled_by(height.id);
                    });
                    let valid = u64::from(draft.view.size.0) * u64::from(draft.view.size.1)
                        <= crate::editor::MAX_PIXELS;
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(valid, egui::Button::new(t!(ui.ctx(), "OK")))
                            .clicked()
                        {
                            result = CanvasResponse::Apply;
                        }
                        if ui.button(t!(ui.ctx(), "Cancel")).clicked() {
                            result = CanvasResponse::Cancel;
                        }
                    });
                })
            });
        if modal.should_close() && result == CanvasResponse::Pending {
            result = CanvasResponse::Cancel;
        }
        result
    }
    pub(super) fn retro_color_form(ctx: &egui::Context, editor: &mut ColorEditor) -> ColorResponse {
        let mut result = ColorResponse::Pending;
        let modal = egui::Modal::new(egui::Id::unique("web-retro-edit-colors"))
            .frame(paintcraft_retro::frame())
            .show(ctx, |ui| {
                paintcraft_retro::with_style(ui, |ui| {
                    ui.heading(t!(ui.ctx(), "Edit colors"));
                    for (value, label) in editor
                        .color
                        .0
                        .iter_mut()
                        .zip(["Red", "Green", "Blue", "Alpha"])
                    {
                        ui.horizontal(|ui| {
                            let caption = ui.label(t!(ui.ctx(), label));
                            ui.add(egui::DragValue::new(value).range(0..=255))
                                .labelled_by(caption.id);
                        });
                    }
                    ui.horizontal(|ui| {
                        if ui.button(t!(ui.ctx(), "OK")).clicked() {
                            result = ColorResponse::Accept;
                        }
                        if ui.button(t!(ui.ctx(), "Cancel")).clicked() {
                            result = ColorResponse::Cancel;
                        }
                    });
                })
            });
        if modal.should_close() && result == ColorResponse::Pending {
            result = ColorResponse::Cancel;
        }
        result
    }
}
impl WebApp {
    pub(super) fn retro_resize_skew_form(
        ctx: &egui::Context,
        draft: &mut paintcraft_classic::resize_skew::Draft,
    ) -> paintcraft_classic::resize_skew::Response {
        use paintcraft_classic::resize_skew::Response;
        let mut response = Response::Pending;
        let modal = egui::Modal::new(egui::Id::unique("web-retro-resize-skew"))
            .frame(paintcraft_retro::frame())
            .show(ctx, |ui| {
                response = paintcraft_retro::with_style(ui, |ui| {
                    paintcraft_classic::resize_skew::contents(
                        ui,
                        draft,
                        paintcraft_document::CANVAS_SIDE_MAX,
                    )
                });
            });
        if modal.should_close() && response == Response::Pending {
            Response::Cancel
        } else {
            response
        }
    }
}

#[cfg(test)]
#[path = "retro/tests.rs"]
mod tests;
