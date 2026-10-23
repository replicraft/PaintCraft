//! Paint-style names for the fixed palette; extra swatches have descriptive names.
use super::{Action, Controls, CustomColors, Rgba};
use egui::{Color32, Vec2};
use paintcraft_ui::{actions::Context, icons::Icon, widgets::icon_button};

const NAMES: [&str; 28] = [
    "Black",
    "Gray",
    "Dark red",
    "Red",
    "Orange",
    "Yellow",
    "Green",
    "Turquoise",
    "Indigo",
    "Purple",
    "White",
    "Light gray",
    "Brown",
    "Rose",
    "Gold",
    "Light yellow",
    "Lime",
    "Light turquoise",
    "Blue-gray",
    "Lavender",
    "Dark gray",
    "Dark brown",
    "Teal",
    "Dark blue",
    "Dark purple",
    "Light orange",
    "Mint",
    "Periwinkle",
];
pub(super) fn tooltip(index: usize, rgb: [u8; 3]) -> String {
    format!(
        "{}\n#{:02X}{:02X}{:02X}",
        NAMES[index], rgb[0], rgb[1], rgb[2]
    )
}

pub(super) fn colors(
    ui: &mut egui::Ui,
    state: &mut Controls,
    custom: &CustomColors,
    context: Context,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| {
        wells(ui, state, context, actions);
        swatches(ui, state, custom, context, actions);
        if ui
            .add_enabled_ui(Action::EditColors.enabled(context), edit_button)
            .inner
            .clicked()
        {
            actions.push(Action::EditColors);
        }
    });
}

fn edit_button(ui: &mut egui::Ui) -> egui::Response {
    if !super::ribbon::spacious(ui) {
        return icon_button(ui, "Edit colors…", Icon::EditColors, false, false);
    }
    let response = ui.add(
        egui::Button::new("")
            .min_size(Vec2::splat(40.0))
            .frame_when_inactive(false)
            .frame(true),
    );
    paintcraft_ui::icons::image(ui.ctx(), Icon::EditColors, 32.0).paint_at(
        ui,
        egui::Rect::from_center_size(response.rect.center(), Vec2::splat(32.0)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "Edit colors…")
    });
    response.on_hover_text("Edit colors…")
}

fn wells(ui: &mut egui::Ui, state: &Controls, context: Context, actions: &mut Vec<Action>) {
    ui.allocate_ui_with_layout(
        Vec2::new(40.0, 78.0),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            for (label, color, action, size, inner) in [
                (
                    "Pencil color",
                    state.primary,
                    Action::EditColors,
                    40.0,
                    15.0,
                ),
                (
                    "Fill color",
                    state.secondary,
                    Action::EditSecondaryColors,
                    32.0,
                    13.0,
                ),
            ] {
                let response = ui
                    .add_enabled_ui(action.enabled(context), |ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
                        let accent = action == Action::EditColors
                            || response.hovered()
                            || response.has_focus();
                        let border = if accent {
                            ui.visuals().selection.stroke.color
                        } else {
                            ui.visuals().widgets.noninteractive.fg_stroke.color
                        };
                        ui.painter().circle_stroke(
                            rect.center(),
                            size * 0.5 - 1.0,
                            egui::Stroke::new(1.0, border),
                        );
                        ui.painter().circle_filled(
                            rect.center(),
                            inner,
                            Color32::from_rgb(color.0[0], color.0[1], color.0[2]),
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::ColorWell, ui.is_enabled(), label)
                        });
                        response.on_hover_text(well_tooltip(label, color))
                    })
                    .inner;
                if response.clicked() {
                    actions.push(action);
                }
            }
        },
    );
}

fn well_tooltip(label: &str, color: Rgba) -> String {
    let rgb = [color.0[0], color.0[1], color.0[2]];
    let name = super::PALETTE
        .iter()
        .position(|value| *value == rgb)
        .map_or("Custom color", |index| NAMES[index]);
    format!(
        "{label}\n{name}\n#{:02X}{:02X}{:02X}",
        rgb[0], rgb[1], rgb[2]
    )
}

fn swatches(
    ui: &mut egui::Ui,
    state: &mut super::Controls,
    custom: &super::CustomColors,
    context: paintcraft_ui::actions::Context,
    actions: &mut Vec<super::Action>,
) {
    let spacious = super::ribbon::spacious(ui);
    let cell = if spacious { 24.0 } else { 20.0 };
    let radius = cell * 0.5 - 1.0;
    egui::Grid::new("windows11-palette")
        .num_columns(10)
        .min_col_width(0.0)
        .min_row_height(cell)
        .spacing(egui::vec2(if spacious { 6.0 } else { 4.0 }, 3.0))
        .show(ui, |ui| {
            for index in 0..30 {
                let (color, label, tip) = if index < 20 {
                    let rgb = super::PALETTE[index];
                    (
                        Some(super::Rgba([rgb[0], rgb[1], rgb[2], 255])),
                        format!("Color #{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]),
                        tooltip(index, rgb),
                    )
                } else {
                    let slot = index - 20;
                    let color = custom[slot];
                    let tip = color.map_or_else(
                        || format!("Custom color {} — add a color", slot + 1),
                        |c| {
                            format!(
                                "Custom color {}\n#{:02X}{:02X}{:02X}",
                                slot + 1,
                                c.0[0],
                                c.0[1],
                                c.0[2]
                            )
                        },
                    );
                    (color, format!("Palette custom color {}", slot + 1), tip)
                };
                let (rect, response) =
                    ui.allocate_exact_size(egui::Vec2::splat(cell), egui::Sense::click());
                if let Some(color) = color {
                    ui.painter().circle_filled(
                        rect.center(),
                        radius,
                        egui::Color32::from_rgb(color.0[0], color.0[1], color.0[2]),
                    );
                }
                ui.painter().circle_stroke(
                    rect.center(),
                    radius,
                    egui::Stroke::new(0.8, ui.visuals().widgets.noninteractive.fg_stroke.color),
                );
                if response.hovered() || response.has_focus() {
                    ui.painter().circle_stroke(
                        rect.center(),
                        radius + 0.75,
                        egui::Stroke::new(1.0, ui.visuals().selection.stroke.color),
                    );
                }
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &label)
                });
                if let Some(color) = color {
                    if response.clicked() {
                        state.primary = color;
                    }
                    if response.secondary_clicked() {
                        state.secondary = color;
                    }
                } else if response.clicked() && super::Action::EditColors.enabled(context) {
                    actions.push(super::Action::EditCustomColor((index - 20) as u8));
                }
                response.on_hover_text(tip);
                if index % 10 == 9 {
                    ui.end_row();
                }
            }
        });
}
