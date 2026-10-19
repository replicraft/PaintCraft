//! Classic presentation emits actions; editor and platform services own effects.
use egui::{Color32, Vec2};
use paintcraft_core::types::Rgba;
use paintcraft_tools::Tool;
use paintcraft_ui::widgets::{Icon, icon_button};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassicVariant {
    V7,
    V10,
    V11,
}

impl ClassicVariant {
    pub const ALL: [ClassicVariant; 3] =
        [ClassicVariant::V7, ClassicVariant::V10, ClassicVariant::V11];
    /// Selected when the user has not chosen.
    pub const DEFAULT: ClassicVariant = ClassicVariant::V11;

    pub fn id(self) -> &'static str {
        match self {
            ClassicVariant::V7 => "v7",
            ClassicVariant::V10 => "v10",
            ClassicVariant::V11 => "v11",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ClassicVariant::V7 => "Paint v7",
            ClassicVariant::V10 => "Paint v10",
            ClassicVariant::V11 => "Paint v11",
        }
    }

    pub fn from_id(s: &str) -> Option<ClassicVariant> {
        ClassicVariant::ALL.into_iter().find(|v| v.id() == s)
    }
}

pub use paintcraft_ui::chrome::{Action, Controls};
use paintcraft_ui::chrome::{color_edit, options, tool_button};

pub fn chrome(ui: &mut egui::Ui, variant: ClassicVariant, state: &mut Controls) -> Vec<Action> {
    let mut actions = Vec::new();
    ui.horizontal(|ui| {
        ui.menu_button("File", |ui| {
            for (label, action) in [
                ("New", Action::New),
                ("Open…", Action::Open),
                ("Save", Action::Save),
                ("Save as…", Action::SaveAs),
                ("Export…", Action::Export),
            ] {
                if ui.button(label).clicked() {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        ui.menu_button("Edit", |ui| {
            for (label, action) in [
                ("Undo", Action::Undo),
                ("Redo", Action::Redo),
                ("Select all", Action::SelectAll),
                ("Deselect", Action::Deselect),
            ] {
                if ui.button(label).clicked() {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        ui.menu_button("View", |ui| {
            if ui.button("Change Paint version…").clicked() {
                actions.push(Action::Interface);
                ui.close();
            }
        });
        ui.separator();
        for (label, icon, action) in [
            ("New", Icon::New, Action::New),
            ("Open", Icon::Open, Action::Open),
            ("Save", Icon::Save, Action::Save),
            ("Export", Icon::Export, Action::Export),
            ("Undo", Icon::Undo, Action::Undo),
            ("Redo", Icon::Redo, Action::Redo),
        ] {
            if icon_button(ui, label, icon, false, false).clicked() {
                actions.push(action);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Interface").clicked() {
                actions.push(Action::Interface);
            }
        });
    });
    if variant != ClassicVariant::V11 {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Home")
                    .color(Color32::from_rgb(0, 95, 184))
                    .strong(),
            );
            ui.label("PaintCraft");
        });
    }
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
                                Tool::Brush,
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                if tool_button(ui, state, tool, true) {
                                    actions.push(Action::SelectTool(tool));
                                }
                                if i % 3 == 2 {
                                    ui.end_row();
                                }
                            }
                        });
                });
                ui.separator();
                ribbon_group(ui, "Shapes", |ui| {
                    ui.horizontal(|ui| {
                        for tool in [Tool::Line, Tool::Rect, Tool::Ellipse] {
                            if tool_button(ui, state, tool, true) {
                                actions.push(Action::SelectTool(tool));
                            }
                        }
                    });
                });
                ui.separator();
                ribbon_group(ui, "Colors", |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            color_edit(ui, "Pencil color", &mut state.primary);
                            color_edit(ui, "Fill color", &mut state.secondary);
                        });
                        egui::Grid::new("classic-palette")
                            .min_col_width(0.0)
                            .num_columns(14)
                            .spacing(Vec2::splat(3.0))
                            .show(ui, |ui| {
                                for (i, rgb) in PALETTE.into_iter().enumerate() {
                                    let color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                                    let (rect, response) = ui.allocate_exact_size(
                                        Vec2::new(17.0, 19.0),
                                        egui::Sense::click(),
                                    );
                                    if variant == ClassicVariant::V11 {
                                        ui.painter().circle_filled(rect.center(), 8.0, color);
                                    } else {
                                        ui.painter().rect_filled(rect.shrink(1.0), 0.0, color);
                                    }
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
                                            format!(
                                                "Color #{:02X}{:02X}{:02X}",
                                                rgb[0], rgb[1], rgb[2]
                                            ),
                                        )
                                    });
                                    if response.clicked() {
                                        state.primary = Rgba([rgb[0], rgb[1], rgb[2], 255]);
                                    }
                                    if response.secondary_clicked() {
                                        state.secondary = Rgba([rgb[0], rgb[1], rgb[2], 255]);
                                    }
                                    if i % 14 == 13 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });
                });
            });
        });
    ui.separator();
    options(ui, state, &mut actions);
    actions
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
                ui.weak(label);
            },
        );
    });
}
const PALETTE: [[u8; 3]; 28] = [
    [0, 0, 0],
    [127, 127, 127],
    [136, 0, 21],
    [237, 28, 36],
    [255, 127, 39],
    [255, 242, 0],
    [34, 177, 76],
    [0, 162, 232],
    [63, 72, 204],
    [163, 73, 164],
    [255, 255, 255],
    [195, 195, 195],
    [185, 122, 87],
    [255, 174, 201],
    [255, 201, 14],
    [239, 228, 176],
    [181, 230, 29],
    [153, 217, 234],
    [112, 146, 190],
    [200, 191, 231],
    [64, 64, 64],
    [128, 64, 0],
    [0, 128, 128],
    [0, 64, 128],
    [128, 0, 128],
    [255, 224, 192],
    [192, 255, 224],
    [224, 224, 255],
];

/// Initial presentation styles; reference-build qualification remains separate.
pub fn apply_style(ctx: &egui::Context, v: ClassicVariant) {
    let (rounding, panel, accent) = match v {
        ClassicVariant::V7 => (
            0u8,
            Color32::from_rgb(233, 238, 247),
            Color32::from_rgb(63, 119, 186),
        ),
        ClassicVariant::V10 => (
            0,
            Color32::from_rgb(243, 243, 243),
            Color32::from_rgb(0, 120, 215),
        ),
        ClassicVariant::V11 => (
            6,
            Color32::from_rgb(249, 249, 249),
            Color32::from_rgb(0, 95, 184),
        ),
    };
    let mut vis = egui::Visuals::light();
    vis.panel_fill = panel;
    vis.window_fill = panel;
    vis.selection.bg_fill = accent;
    vis.selection.stroke = egui::Stroke::new(1.0, Color32::WHITE);
    let r = egui::CornerRadius::same(rounding);
    for w in [
        &mut vis.widgets.noninteractive,
        &mut vis.widgets.inactive,
        &mut vis.widgets.hovered,
        &mut vis.widgets.active,
        &mut vis.widgets.open,
    ] {
        w.corner_radius = r;
    }
    vis.window_corner_radius = r;
    ctx.set_visuals(vis);
}
