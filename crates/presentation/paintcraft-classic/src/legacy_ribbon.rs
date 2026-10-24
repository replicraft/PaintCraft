//! Historical ribbons retain compact colorful tools and their own image controls.
use super::{Action, Controls, PALETTE, Rgba, Workspace, dropdowns, palette, shapes};
use egui::{Color32, Vec2};
use paintcraft_tools::Tool;
use paintcraft_ui::i18n::tr;
use paintcraft_ui::{
    actions::Context,
    chrome::{options, tool_button},
};

pub(super) fn show(
    ui: &mut egui::Ui,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: Context,
    actions: &mut Vec<Action>,
) {
    ui.separator();
    egui::ScrollArea::horizontal()
        .id_salt("classic-ribbon-scroll")
        .max_height(142.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ribbon_group(ui, "Selection", |ui| {
                    if tool_button(ui, state, Tool::Select, true) {
                        actions.push(Action::SelectTool(Tool::Select));
                    }
                    dropdowns::selection(ui, state, context, actions);
                });
                ui.separator();
                ribbon_group(ui, "Image", |ui| {
                    super::image_menus::legacy_controls(ui, context, actions);
                });
                ui.separator();
                ribbon_group(ui, "Tools", |ui| {
                    egui::Grid::new("classic-tools")
                        .num_columns(3)
                        .spacing(Vec2::new(2.0, 2.0))
                        .show(ui, |ui| {
                            for (i, tool) in [
                                Tool::Pencil,
                                Tool::Fill,
                                Tool::Text,
                                Tool::Eraser,
                                Tool::ColorPicker,
                                Tool::Brush,
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                if tool_button(ui, state, tool, false) {
                                    actions.push(Action::SelectTool(tool));
                                }
                                if i % 3 == 2 {
                                    ui.end_row();
                                }
                            }
                        });
                });
                ui.separator();
                ribbon_group(ui, "Brushes", |ui| {
                    let id = ui.make_persistent_id("legacy-brush-gallery-open");
                    let was_open = ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
                    let (button, popup) = egui::containers::menu::MenuButton::from_button(
                        egui::Button::image_and_text(
                            paintcraft_ui::icons::image(
                                ui.ctx(),
                                paintcraft_ui::icons::Icon::Brush,
                                28.0,
                            ),
                            tr(ui.ctx(), "Brushes"),
                        )
                        .min_size(Vec2::new(80.0, 54.0)),
                    )
                    .ui(ui, |ui| {
                        super::brush_gallery::gallery(ui, state, actions, !was_open);
                    });
                    ui.data_mut(|data| data.insert_temp(id, popup.is_some()));
                    button.on_hover_text(tr(ui.ctx(), "Brushes"));
                });
                ui.separator();
                ribbon_group(ui, "Shapes", |ui| {
                    ui.horizontal_top(|ui| {
                        shapes::gallery(ui, state, actions);
                        ui.vertical(|ui| {
                            dropdowns::shape_paint(ui, state);
                        });
                    });
                });
                ui.separator();
                ribbon_group(ui, "Size", |ui| {
                    ui.add_space(22.0);
                    super::stroke_size::show(ui, state, context);
                });
                ui.separator();
                ribbon_group(ui, "Colors", |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            for (label, color, action) in [
                                ("Pencil color", state.primary, Action::EditColors),
                                ("Fill color", state.secondary, Action::EditSecondaryColors),
                            ] {
                                let response = ui.add_enabled(
                                    action.enabled(context),
                                    egui::Button::new("")
                                        .fill(Color32::from_rgb(color.0[0], color.0[1], color.0[2]))
                                        .min_size(Vec2::new(36.0, 22.0)),
                                );
                                let accessible_label = tr(ui.ctx(), label);
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::Role::ColorWell,
                                        action.enabled(context) && ui.is_enabled(),
                                        &accessible_label,
                                    )
                                });
                                if response.clicked() {
                                    actions.push(action);
                                }
                            }
                        });
                        egui::Grid::new("classic-palette")
                            .min_col_width(0.0)
                            .num_columns(10)
                            .spacing(Vec2::splat(3.0))
                            .show(ui, |ui| {
                                for (i, default) in PALETTE[..20]
                                    .iter()
                                    .copied()
                                    .map(Some)
                                    .chain(std::iter::repeat_n(None, 10))
                                    .enumerate()
                                {
                                    let custom = (i >= 20).then(|| workspace.custom_colors[i - 20]);
                                    let rgb = if let Some(rgb) = default {
                                        rgb
                                    } else {
                                        custom
                                            .flatten()
                                            .map_or([255; 3], |c| [c.0[0], c.0[1], c.0[2]])
                                    };
                                    let color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                                    let (rect, response) = ui.allocate_exact_size(
                                        Vec2::new(17.0, 19.0),
                                        egui::Sense::click(),
                                    );
                                    ui.painter().rect_filled(rect.shrink(1.0), 0.0, color);
                                    ui.painter().rect_stroke(
                                        rect.shrink(0.5),
                                        0.0,
                                        egui::Stroke::new(
                                            0.8,
                                            ui.visuals().widgets.noninteractive.bg_stroke.color,
                                        ),
                                        egui::StrokeKind::Inside,
                                    );
                                    if response.hovered() || response.has_focus() {
                                        ui.painter().rect_stroke(
                                            rect,
                                            2.0,
                                            egui::Stroke::new(1.0, Color32::from_rgb(0, 95, 184)),
                                            egui::StrokeKind::Inside,
                                        );
                                    }
                                    response.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::Role::Button,
                                            ui.is_enabled(),
                                            if i < 20 {
                                                format!(
                                                    "Color #{:02X}{:02X}{:02X}",
                                                    rgb[0], rgb[1], rgb[2]
                                                )
                                            } else {
                                                format!("Palette custom color {}", i - 19)
                                            },
                                        )
                                    });
                                    let tip = if i < 20 {
                                        palette::tooltip(ui.ctx(), i, rgb)
                                    } else {
                                        format!("Custom color {}", i - 19)
                                    };
                                    let response = response.on_hover_text(tip);
                                    if response.clicked() {
                                        if custom == Some(None) {
                                            actions.push(Action::EditCustomColor((i - 20) as u8));
                                        } else {
                                            state.primary = Rgba([rgb[0], rgb[1], rgb[2], 255]);
                                        }
                                    }
                                    if response.secondary_clicked() && custom != Some(None) {
                                        state.secondary = Rgba([rgb[0], rgb[1], rgb[2], 255]);
                                    }
                                    if i % 10 == 9 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });
                    if paintcraft_ui::actions::menu_item(ui, Action::EditColors, context) {
                        actions.push(Action::EditColors);
                    }
                });
            });
        });
    ui.separator();
    options(ui, state, actions);
}
fn ribbon_group(ui: &mut egui::Ui, label: &str, contents: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        let top = ui.cursor().min.y;
        ui.set_min_height(126.0);
        contents(ui);
        ui.add_space((top + 108.0 - ui.cursor().min.y).max(0.0));
        let width = ui.min_rect().width();
        ui.allocate_ui_with_layout(
            Vec2::new(width, 16.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.weak(tr(ui.ctx(), label));
            },
        );
    });
}
