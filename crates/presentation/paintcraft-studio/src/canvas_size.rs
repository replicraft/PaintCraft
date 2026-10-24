//! Studio canvas placement; the host captures ownership and publishes atomically.
use paintcraft_core::types::canvas::Anchor;
use paintcraft_ui::i18n::tr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasSize {
    pub size: (u32, u32),
    pub anchor: Anchor,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasResponse {
    Pending,
    Apply,
    Cancel,
}

pub fn canvas_size_dialog(
    ctx: &egui::Context,
    values: &mut CanvasSize,
    max_dimension: u32,
) -> CanvasResponse {
    let mut response = CanvasResponse::Pending;
    let width = (ctx.content_rect().width() - 48.0).clamp(180.0, 460.0);
    let height = (ctx.content_rect().height() - 64.0).clamp(120.0, 460.0);
    let valid = (1..=max_dimension).contains(&values.size.0)
        && (1..=max_dimension).contains(&values.size.1);
    let modal = egui::Modal::new(egui::Id::unique("studio-canvas-size")).show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().interact_size.y = 32.0;
        ui.heading(tr(ui.ctx(), "Canvas size"));
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .id_salt("studio-canvas-size-body")
            .max_height((height - 90.0).max(40.0))
            .show(ui, |ui| {
                ui.label(tr(
                    ui.ctx(),
                    "Your artwork stays at its original size. Content outside the canvas is kept.",
                ));
                egui::Grid::new("studio-canvas-dimensions")
                    .num_columns(2)
                    .spacing(egui::vec2(12.0, 6.0))
                    .show(ui, |ui| {
                        for (name, value) in [
                            ("Width", &mut values.size.0),
                            ("Height", &mut values.size.1),
                        ] {
                            let label = ui.label(tr(ui.ctx(), name));
                            ui.add(
                                egui::DragValue::new(value)
                                    .range(1..=max_dimension)
                                    .min_size(egui::vec2(120.0, 32.0))
                                    .suffix(" px"),
                            )
                            .labelled_by(label.id);
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
                ui.label(tr(ui.ctx(), "Anchor"));
                egui::Grid::new("studio-canvas-anchor-grid")
                    .spacing(egui::vec2(4.0, 4.0))
                    .show(ui, |ui| {
                        for (index, anchor) in Anchor::ALL.into_iter().enumerate() {
                            let selected = values.anchor == anchor;
                            let label = format!(
                                "{}: {}",
                                tr(ui.ctx(), "Anchor"),
                                tr(ui.ctx(), anchor.name())
                            );
                            let glyph = ["↖", "↑", "↗", "←", "●", "→", "↙", "↓", "↘"][index];
                            let button = ui.add(
                                egui::Button::new(glyph)
                                    .selected(selected)
                                    .min_size(egui::vec2(32.0, 32.0)),
                            );
                            button.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::Role::Button,
                                    ui.is_enabled(),
                                    selected,
                                    &label,
                                )
                            });
                            if button.clicked() {
                                values.anchor = anchor;
                            }
                            button.on_hover_text(label);
                            if index % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
            });
        ui.add_space(6.0);
        ui.separator();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(valid, egui::Button::new(tr(ui.ctx(), "Apply")))
                .clicked()
            {
                response = CanvasResponse::Apply;
            }
            if ui.button(tr(ui.ctx(), "Cancel")).clicked() {
                response = CanvasResponse::Cancel;
            }
        });
    });
    if modal.should_close() {
        response = CanvasResponse::Cancel;
    }
    response
}
