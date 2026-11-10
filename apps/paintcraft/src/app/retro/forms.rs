//! Retro modal surfaces retain the existing captured host drafts and transactions.
use eframe::egui;
use paintcraft_classic::{
    ColorEditor, ColorResponse,
    canvas_size::{CanvasResponse, CanvasSize},
};
use paintcraft_ui::t;

pub(crate) fn colors(ctx: &egui::Context, editor: &mut ColorEditor) -> ColorResponse {
    let mut response = ColorResponse::Pending;
    let modal = modal(ctx, "retro-edit-colors").show(ctx, |ui| {
        bound_content(ui);
        paintcraft_retro::with_style(ui, |ui| {
            ui.heading(t!(ui.ctx(), "Edit colors"));
            for (index, label) in ["Red", "Green", "Blue", "Alpha"].into_iter().enumerate() {
                ui.horizontal(|ui| {
                    let caption = ui.label(t!(ui.ctx(), label));
                    ui.add(egui::DragValue::new(&mut editor.color.0[index]).range(0..=255))
                        .labelled_by(caption.id);
                });
            }
            ui.horizontal(|ui| {
                if ui.button(t!(ui.ctx(), "OK")).clicked() {
                    response = ColorResponse::Accept;
                }
                if ui.button(t!(ui.ctx(), "Cancel")).clicked() {
                    response = ColorResponse::Cancel;
                }
            });
        })
    });
    if modal.should_close() {
        ColorResponse::Cancel
    } else {
        response
    }
}

pub(crate) fn canvas(
    ctx: &egui::Context,
    view: &mut CanvasSize,
    properties: Option<&str>,
) -> CanvasResponse {
    let mut response = CanvasResponse::Pending;
    let modal = modal(ctx, "retro-canvas-size").show(ctx, |ui| {
        bound_content(ui);
        paintcraft_retro::with_style(ui, |ui| {
            ui.heading(t!(
                ui.ctx(),
                if properties.is_some() {
                    "Image properties"
                } else {
                    "Canvas size"
                }
            ));
            if let Some(name) = properties {
                ui.label(name);
            }
            for (label, value) in [("Width", &mut view.size.0), ("Height", &mut view.size.1)] {
                ui.horizontal(|ui| {
                    let caption = ui.label(t!(ui.ctx(), label));
                    ui.add(egui::DragValue::new(value).range(1..=crate::settings::SIZE_MAX))
                        .labelled_by(caption.id);
                });
            }
            if let Some(startup) = &mut view.startup {
                ui.checkbox(
                    &mut startup.remember_last,
                    t!(ui.ctx(), "Remember last canvas size"),
                );
            }
            ui.horizontal(|ui| {
                if ui.button(t!(ui.ctx(), "OK")).clicked() {
                    response = CanvasResponse::Apply;
                }
                if ui.button(t!(ui.ctx(), "Cancel")).clicked() {
                    response = CanvasResponse::Cancel;
                }
            });
        })
    });
    if modal.should_close() {
        CanvasResponse::Cancel
    } else {
        response
    }
}

pub(crate) fn text(
    ui: &mut egui::Ui,
    options: &mut paintcraft_classic::TextOptions,
    families: &[String],
) {
    paintcraft_retro::with_style(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            let family = ui.label(t!(ui.ctx(), "Default font"));
            egui::ComboBox::from_id_salt("retro-text-family")
                .selected_text(&options.family)
                .show_ui(ui, |ui| {
                    for name in families {
                        ui.selectable_value(&mut options.family, name.clone(), name);
                    }
                })
                .response
                .labelled_by(family.id);
            let size = ui.label(t!(ui.ctx(), "Font size"));
            ui.add(egui::DragValue::new(&mut options.points).range(1..=512))
                .labelled_by(size.id);
            for (value, label) in [
                (&mut options.bold, "Bold"),
                (&mut options.italic, "Italic"),
                (&mut options.underline, "Underline"),
                (&mut options.background_fill, "Opaque background"),
            ] {
                ui.checkbox(value, t!(ui.ctx(), label));
            }
        });
    });
}

// Set the Area's initial width AND bound the actual contents each frame. The
// latter also repairs an existing modal after the viewport becomes narrower.
pub(crate) fn modal(ctx: &egui::Context, name: &str) -> egui::Modal {
    let id = egui::Id::unique(name);
    egui::Modal::new(id)
        .area(
            egui::Modal::default_area(id)
                .default_width(content_width(ctx) + 6.0)
                .constrain_to(ctx.content_rect().shrink(8.0)),
        )
        .frame(paintcraft_retro::frame())
}
fn content_width(ctx: &egui::Context) -> f32 {
    (ctx.content_rect().width() - 24.0).clamp(1.0, 300.0)
}
pub(crate) fn bound_content(ui: &mut egui::Ui) {
    ui.set_width(content_width(ui.ctx()));
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
}
