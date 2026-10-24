//! Compact historical brush-media tiles. Samples are UI illustrations, not strokes.
use super::{Action, ClassicVariant, Controls};
use egui::{Rect, Vec2};
use paintcraft_core::types::brush::BrushMedia;
use paintcraft_tools::Tool;
use paintcraft_ui::t;
pub(super) fn gallery(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    state: &mut Controls,
    actions: &mut Vec<Action>,
    opening: bool,
) {
    let height = super::popup_layout::height(ui, 64.0, 260.0);
    ui.set_width(240.0);
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    egui::ScrollArea::vertical()
        .id_salt("historical-brush-media")
        .max_height(height)
        .show(ui, |ui| {
            let mut entries = Vec::with_capacity(10);
            egui::Grid::new("historical-brush-tiles")
                .spacing(Vec2::ZERO)
                .show(ui, |ui| {
                    for (index, media) in BrushMedia::ALL.into_iter().enumerate() {
                        let selected = state.tool == Tool::Brush
                            && state.options.round
                            && state.options.media == media;
                        let label = t!(
                            ui.ctx(),
                            if index == 0 {
                                "Round brush"
                            } else {
                                media.name()
                            },
                        );
                        let response = super::legacy_surface::button(
                            ui,
                            variant,
                            egui::Button::new(" ").min_size(Vec2::new(80.0, 64.0)),
                            selected,
                        );
                        entries.push(response.id);
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::Button,
                                ui.is_enabled(),
                                selected,
                                &label,
                            )
                        });
                        let sample =
                            Rect::from_center_size(response.rect.center(), Vec2::new(60.0, 32.0));
                        super::brush_gallery::draw_sample(
                            ui.painter(),
                            sample,
                            index,
                            ui.visuals().text_color(),
                        );
                        if (opening && selected) || response.gained_focus() {
                            response.scroll_to_me(Some(egui::Align::Center));
                        }
                        if response.on_hover_text(label).clicked() {
                            state.options.media = media;
                            super::brush_gallery::choose(state, actions, true);
                            ui.close();
                        }
                        if index % 3 == 2 {
                            ui.end_row();
                        }
                    }
                });
            ui.separator();
            let selected = state.tool == Tool::Brush && !state.options.round;
            let label = t!(ui.ctx(), "Square brush");
            let square = super::legacy_surface::button(
                ui,
                variant,
                egui::Button::new(&label).min_size(Vec2::new(240.0, 28.0)),
                selected,
            );
            entries.push(square.id);
            square.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, &label)
            });
            if (opening && selected) || square.gained_focus() {
                square.scroll_to_me(Some(egui::Align::Center));
            }
            super::legacy_surface::grid_navigation(ui, &entries, 3);
            if square.clicked() {
                super::brush_gallery::choose(state, actions, false);
                ui.close();
            }
        });
}
