//! Historical ribbon gallery with row scrolling and an expanded shape chooser.
use super::SHAPES;
use crate::{Action, ClassicVariant, Controls};
use egui::{Color32, Vec2};
use paintcraft_ui::{i18n::tr, icons::Icon};

pub(crate) fn gallery(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    state: &mut Controls,
    actions: &mut Vec<Action>,
) {
    ui.scope(|ui| {
        let ink = if ui.visuals().dark_mode {
            Color32::from_rgb(150, 205, 240)
        } else {
            Color32::from_rgb(30, 93, 130)
        };
        ui.visuals_mut().override_text_color = Some(ink);
        ui.visuals_mut().widgets.active.fg_stroke.color = ink;
        ui.spacing_mut().button_padding = Vec2::splat(2.0);
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        egui::Frame::new()
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .inner_margin(1)
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let pending_id = ui.make_persistent_id("shape-scroll-request");
                    let pending = ui.data_mut(|d| d.remove_temp::<f32>(pending_id));
                    let mut scroll = egui::ScrollArea::vertical()
                        .id_salt("legacy-shapes")
                        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                        .min_scrolled_height(60.0)
                        .max_height(60.0)
                        .max_width(140.0);
                    if let Some(offset) = pending {
                        scroll = scroll.vertical_scroll_offset(offset);
                    }
                    let output = scroll.show(ui, |ui| grid(ui, variant, state, actions, false));
                    let max_offset = (output.content_size.y - output.inner_rect.height()).max(0.0);
                    ui.vertical(|ui| {
                        let up = scroll_button(
                            ui,
                            "Previous shapes",
                            -1.0,
                            output.state.clamped_offset().y > 0.5,
                        );
                        let down = scroll_button(
                            ui,
                            "Next shapes",
                            1.0,
                            output.state.clamped_offset().y < max_offset - 0.5,
                        );
                        let delta = if up.clicked() {
                            -20.0
                        } else if down.clicked() {
                            20.0
                        } else {
                            0.0
                        };
                        if delta != 0.0 {
                            let target =
                                (output.state.clamped_offset().y + delta).clamp(0.0, max_offset);
                            ui.data_mut(|d| d.insert_temp(pending_id, target));
                            ui.ctx().request_repaint();
                        }
                        let (response, _) = egui::containers::menu::MenuButton::from_button(
                            egui::Button::new(" ").min_size(Vec2::new(14.0, 20.0)),
                        )
                        .config(egui::containers::menu::MenuConfig::new().style(
                            |style: &mut egui::Style| {
                                egui::containers::menu::menu_style(style);
                                style.visuals.menu_corner_radius = egui::CornerRadius::ZERO;
                            },
                        ))
                        .ui(ui, |ui| {
                            ui.visuals_mut().override_text_color = Some(ink);
                            ui.visuals_mut().widgets.active.fg_stroke.color = ink;
                            let height = crate::popup_layout::height(ui, 60.0, 180.0);
                            egui::ScrollArea::vertical()
                                .id_salt("expanded-historical-shapes")
                                .max_height(height)
                                .show(ui, |ui| {
                                    grid(ui, variant, state, actions, true);
                                });
                        });
                        let center = response.rect.center();
                        crate::legacy_controls::arrow(ui, center + Vec2::new(0.0, 2.0));
                        ui.painter().line_segment(
                            [
                                center + Vec2::new(-3.0, -3.0),
                                center + Vec2::new(3.0, -3.0),
                            ],
                            ui.visuals().widgets.inactive.fg_stroke,
                        );
                        crate::dropdowns::open_state(ui, &response, "Shapes…");
                        response.on_hover_text(tr(ui.ctx(), "Shapes…"));
                    });
                });
            });
    });
}

fn grid(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    state: &mut Controls,
    actions: &mut Vec<Action>,
    close: bool,
) {
    egui::Grid::new("historical-shape-grid")
        .spacing(Vec2::ZERO)
        .min_col_width(20.0)
        .min_row_height(20.0)
        .show(ui, |ui| {
            for (index, (icon, label, tool)) in SHAPES.into_iter().enumerate() {
                if let Some(tool) = tool
                    && button(ui, variant, label, icon, state.tool == tool).clicked()
                {
                    state.tool = tool;
                    actions.push(Action::SelectTool(tool));
                    if close {
                        ui.close();
                    }
                }
                if index % 7 == 6 {
                    ui.end_row();
                }
            }
        });
}

fn scroll_button(ui: &mut egui::Ui, label: &str, direction: f32, enabled: bool) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(" ").min_size(Vec2::new(14.0, 20.0)),
    );
    let center = response.rect.center();
    let ink = if enabled {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            center + Vec2::new(-2.5, -direction * 1.5),
            center + Vec2::new(2.5, -direction * 1.5),
            center + Vec2::new(0.0, direction * 1.5),
        ],
        ink,
        egui::Stroke::NONE,
    ));
    let label = tr(ui.ctx(), label);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, enabled && ui.is_enabled(), &label)
    });
    response.on_hover_text(label)
}

fn button(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    label: &str,
    icon: Icon,
    selected: bool,
) -> egui::Response {
    let label = tr(ui.ctx(), label);
    let image = paintcraft_ui::icons::classic_shape_image(
        ui.ctx(),
        icon,
        16.0,
        ui.visuals().strong_text_color(),
    );
    let response = crate::legacy_surface::button(
        ui,
        variant,
        egui::Button::new(image).min_size(Vec2::splat(20.0)),
        selected,
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, &label)
    });
    response.on_hover_text(label)
}
