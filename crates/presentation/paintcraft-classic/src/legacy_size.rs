//! Historical ribbon thickness presets and explicit custom tool size.
use super::{ClassicVariant, Controls};
use egui::{Rect, Vec2};
use paintcraft_tools::Tool;
use paintcraft_ui::actions::Context;
use paintcraft_ui::t;

pub(super) fn show(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    state: &mut Controls,
    context: Context,
) {
    let enabled =
        super::tool_controls::supports(state.tool) && context.document_open && !context.blocked;
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            let (response, _) = egui::containers::menu::MenuButton::from_button(
                egui::Button::new("")
                    .min_size(Vec2::new(46.0, 66.0))
                    .frame_when_inactive(false),
            )
            .config(super::legacy_surface::menu_config())
            .ui(ui, |ui| {
                choices(ui, variant, state);
            });
            let ink = ui.visuals().text_color();
            for (index, width) in [1.0, 2.0, 4.0, 6.0].into_iter().enumerate() {
                let center = response.rect.center_top() + Vec2::new(0.0, 9.0 + index as f32 * 7.0);
                ui.painter().rect_filled(
                    Rect::from_center_size(center, Vec2::new(32.0, width)),
                    0,
                    ink,
                );
            }
            ui.painter().text(
                response.rect.center_top() + Vec2::new(0.0, 46.0),
                egui::Align2::CENTER_CENTER,
                t!(ui.ctx(), "Size"),
                egui::TextStyle::Body.resolve(ui.style()),
                ink,
            );
            super::legacy_controls::arrow(ui, response.rect.center_bottom() - Vec2::new(0.0, 7.0));
            super::legacy_surface::open_state(ui, &response, "Stroke size");
            response
        })
        .inner;
    response.on_hover_text(t!(ui.ctx(), "Stroke size"));
}

fn choices(ui: &mut egui::Ui, variant: ClassicVariant, state: &mut Controls) {
    let height = super::popup_layout::height(ui, 64.0, 256.0);
    ui.set_width(180.0);
    egui::ScrollArea::vertical()
        .id_salt("historical-tool-sizes")
        .max_height(height)
        .show(ui, |ui| {
            let size = if state.tool == Tool::Pencil {
                &mut state.options.pencil_size
            } else {
                &mut state.options.size
            };
            for value in [1, 3, 5, 8] {
                let selected = *size == value;
                let label = format!("{value} px");
                let response = super::legacy_surface::button(
                    ui,
                    variant,
                    egui::Button::new(())
                        .left_text(&label)
                        .min_size(Vec2::new(180.0, 32.0)),
                    selected,
                );
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::Role::Button,
                        ui.is_enabled(),
                        selected,
                        &label,
                    )
                });
                ui.painter().rect_filled(
                    Rect::from_center_size(
                        response.rect.right_center() - Vec2::new(55.0, 0.0),
                        Vec2::new(90.0, value as f32),
                    ),
                    0,
                    ui.visuals().text_color(),
                );
                if response.clicked() {
                    *size = value;
                    ui.close();
                }
            }
            if ![1, 3, 5, 8].contains(size) {
                ui.label(t!(
                    ui.ctx(),
                    "Current: { $size } px",
                    &[("size", size.to_string())],
                ));
            }
            let caption = ui.label(t!(ui.ctx(), "Size in pixels"));
            ui.add(egui::DragValue::new(size).range(1..=1024))
                .labelled_by(caption.id);
        });
}
