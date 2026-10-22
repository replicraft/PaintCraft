//! Paint-style names for the fixed palette; extra swatches have descriptive names.
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

pub(super) fn swatches(
    ui: &mut egui::Ui,
    state: &mut super::Controls,
    custom: &super::CustomColors,
    context: paintcraft_ui::actions::Context,
    actions: &mut Vec<super::Action>,
) {
    egui::Grid::new("windows11-palette")
        .num_columns(10)
        .min_col_width(0.0)
        .min_row_height(20.0)
        .spacing(egui::vec2(4.0, 3.0))
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
                    ui.allocate_exact_size(egui::Vec2::splat(20.0), egui::Sense::click());
                if let Some(color) = color {
                    ui.painter().circle_filled(
                        rect.center(),
                        9.0,
                        egui::Color32::from_rgb(color.0[0], color.0[1], color.0[2]),
                    );
                }
                ui.painter().circle_stroke(
                    rect.center(),
                    9.0,
                    egui::Stroke::new(0.8, ui.visuals().widgets.noninteractive.fg_stroke.color),
                );
                if response.hovered() || response.has_focus() {
                    ui.painter().circle_stroke(
                        rect.center(),
                        9.75,
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
                    actions.push(super::Action::EditColors);
                }
                response.on_hover_text(tip);
                if index % 10 == 9 {
                    ui.end_row();
                }
            }
        });
}
