//! Two-row reference palette; choosing a swatch never quantizes the document.
use crate::{
    Intent, Workspace,
    surface::{bevel, with_style},
};
use egui::{Color32, Vec2};
use paintcraft_core::types::Rgba;
use paintcraft_ui::{
    actions::{Action, Context},
    chrome::Controls,
    t,
};
/// Initial 28 opaque colors, in two rows of fourteen.
pub const PALETTE: [[u8; 3]; 28] = [
    [0, 0, 0],
    [128, 128, 128],
    [128, 0, 0],
    [128, 128, 0],
    [0, 128, 0],
    [0, 128, 128],
    [0, 0, 128],
    [128, 0, 128],
    [128, 128, 64],
    [0, 64, 64],
    [0, 128, 255],
    [0, 64, 128],
    [64, 0, 255],
    [128, 64, 0],
    [255, 255, 255],
    [192, 192, 192],
    [255, 0, 0],
    [255, 255, 0],
    [0, 255, 0],
    [0, 255, 255],
    [0, 0, 255],
    [255, 0, 255],
    [255, 255, 128],
    [0, 255, 128],
    [128, 255, 255],
    [128, 128, 255],
    [255, 0, 128],
    [255, 128, 64],
];

const PALETTE_NAMES: [&str; 28] = [
    "Black",
    "Gray",
    "Maroon",
    "Olive",
    "Green",
    "Teal",
    "Navy",
    "Purple",
    "Dark khaki",
    "Dark teal",
    "Azure",
    "Dark azure",
    "Indigo",
    "Brown",
    "White",
    "Silver",
    "Red",
    "Yellow",
    "Lime",
    "Cyan",
    "Blue",
    "Magenta",
    "Pale yellow",
    "Spring green",
    "Pale cyan",
    "Periwinkle",
    "Rose",
    "Light orange",
];

pub fn palette(
    ui: &mut egui::Ui,
    controls: &mut Controls,
    workspace: &mut Workspace,
    context: Context,
) -> Vec<Intent> {
    let mut intents = Vec::new();
    if !workspace.colors {
        return intents;
    }
    with_style(ui, |ui| {
        ui.spacing_mut().interact_size = Vec2::splat(16.0);
        ui.add_enabled_ui(context.document_open && !context.blocked, |ui| {
            ui.horizontal(|ui| {
                current_colors(ui, controls, workspace, &mut intents);
                egui::ScrollArea::horizontal()
                    .id_salt("retro-palette-scroll")
                    .min_scrolled_height(36.0)
                    .max_height(50.0)
                    .show(ui, |ui| {
                        egui::Grid::new("retro-palette")
                            .min_col_width(16.0)
                            .max_col_width(16.0)
                            .min_row_height(16.0)
                            .spacing(Vec2::splat(1.0))
                            .show(ui, |ui| {
                                for (index, [r, g, b]) in PALETTE.into_iter().enumerate() {
                                    let response = ui.add(
                                        egui::Button::new("")
                                            .fill(Color32::from_rgb(r, g, b))
                                            .min_size(Vec2::splat(16.0)),
                                    );
                                    let label = format!(
                                        "{} #{r:02X}{g:02X}{b:02X}",
                                        t!(ui.ctx(), "Palette color")
                                    );
                                    response.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::Role::ColorWell,
                                            ui.is_enabled(),
                                            &label,
                                        )
                                    });
                                    let response = response.on_hover_text(format!(
                                        "{} #{r:02X}{g:02X}{b:02X}",
                                        t!(ui.ctx(), PALETTE_NAMES[index])
                                    ));
                                    bevel(ui, response.rect.shrink(0.5), true);
                                    let secondary =
                                        response.secondary_clicked() || workspace.edit_secondary;
                                    if response.clicked() || response.secondary_clicked() {
                                        if secondary {
                                            controls.secondary = Rgba([r, g, b, 255]);
                                        } else {
                                            controls.primary = Rgba([r, g, b, 255]);
                                        }
                                    }
                                    if response.double_clicked_by(egui::PointerButton::Primary)
                                        || response
                                            .double_clicked_by(egui::PointerButton::Secondary)
                                    {
                                        intents.push(Intent::Action(if secondary {
                                            Action::EditSecondaryColors
                                        } else {
                                            Action::EditColors
                                        }));
                                    }
                                    if index % 14 == 13 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });
            });
        });
    });
    intents
}

fn current_colors(
    ui: &mut egui::Ui,
    controls: &Controls,
    workspace: &mut Workspace,
    intents: &mut Vec<Intent>,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(36.0, 36.0), egui::Sense::hover());
    for (secondary, offset, color, label) in [
        (true, Vec2::new(13.0, 13.0), controls.secondary, "Color 2"),
        (false, Vec2::new(1.0, 1.0), controls.primary, "Color 1"),
    ] {
        let well = egui::Rect::from_min_size(rect.min + offset, Vec2::splat(22.0));
        let response = ui.put(well, egui::Button::new("").fill(Color32::from_gray(192)));
        bevel(ui, well.shrink(0.5), workspace.edit_secondary == secondary);
        let rgba = color.0;
        ui.painter().rect_filled(
            well.shrink(4.0),
            0,
            Color32::from_rgb(rgba[0], rgba[1], rgba[2]),
        );
        let label = t!(ui.ctx(), label);
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::Role::ColorWell,
                ui.is_enabled(),
                workspace.edit_secondary == secondary,
                &label,
            )
        });
        if response.clicked() {
            workspace.edit_secondary = secondary;
        }
        if response.double_clicked() {
            intents.push(Intent::Action(if secondary {
                Action::EditSecondaryColors
            } else {
                Action::EditColors
            }));
        }
        response.on_hover_text(label);
    }
}
