//! Classic presentation emits actions; editor and platform services own effects.
mod brush_gallery;
pub mod canvas;
mod canvas_handles;
mod canvas_scroll;
pub mod canvas_size;
mod ruler_axis;
mod rulers;
pub mod status;
pub mod tool_controls;
pub use rulers::RULER_SIZE;
mod control_surface;
mod dropdowns;
mod image_menus;
pub mod layers;
mod menus;
mod toolbar_visibility;
pub use toolbar_visibility::ToolbarVisibility;
mod palette;
mod quick_access;
mod ribbon;
mod selection_menu;
mod shapes;
pub mod skin_picker;
mod slider_input;
mod slider_thumb;
#[derive(Debug, Default)]
pub struct Workspace {
    pub layers: bool,
    pub toolbar: ToolbarVisibility,
    /// Current persisted custom colors supplied by the desktop host.
    pub custom_colors: CustomColors,
}
mod text;
pub use text::{TextAlignment, TextOptions, text_options};
mod colors;
pub use colors::{
    CUSTOM_COLOR_COUNT, ColorEditor, ColorResponse, CustomColors, edit_colors_dialog,
};
mod zoom;
pub use zoom::zoom_controls;

use egui::{Color32, Vec2};
use paintcraft_core::types::Rgba;
use paintcraft_tools::Tool;
use paintcraft_ui::widgets::Icon;

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
use paintcraft_ui::chrome::{options, tool_button};

pub fn chrome(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: paintcraft_ui::actions::Context,
) -> Vec<Action> {
    let mut actions = Vec::new();
    if variant == ClassicVariant::V11 {
        // Scoped to Classic chrome: Studio and historical skins keep their own
        // metrics, while installed Selawik supplies the Windows-style text.
        for text in [egui::TextStyle::Body, egui::TextStyle::Button] {
            ui.style_mut()
                .text_styles
                .insert(text, egui::FontId::proportional(15.0));
        }
        ui.style_mut()
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
        ui.spacing_mut().interact_size.y = 32.0;
    }
    let compact = variant == ClassicVariant::V11 && ui.available_width() < 720.0;
    let menu = quick_access::menu_bar(ui, variant).ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            menu_layout(ui, variant);
            if variant == ClassicVariant::V11 {
                menus::file(ui, context, &mut actions);
                return;
            }
            for &action in paintcraft_ui::actions::FILE {
                if matches!(action, Action::Save | Action::Close) {
                    ui.separator();
                }
                if paintcraft_ui::actions::menu_item(ui, action, context) {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        if variant == ClassicVariant::V11 {
            ui.add_space(12.0);
        }
        ui.menu_button("Edit", |ui| {
            menu_layout(ui, variant);
            if variant == ClassicVariant::V11 {
                menus::edit(ui, context, &mut actions);
                return;
            }
            for &action in paintcraft_ui::actions::EDIT {
                if action == Action::SelectAll {
                    ui.separator();
                }
                if paintcraft_ui::actions::menu_item(ui, action, context) {
                    actions.push(action);
                    ui.close();
                }
            }
        });
        if variant == ClassicVariant::V11 {
            ui.add_space(12.0);
        }
        ui.menu_button("View", |ui| {
            menu_layout(ui, variant);
            menus::view(ui, variant, context, &mut actions);
        });
        if !compact {
            ui.separator();
            quick_access::show(ui, variant, context, &mut actions);
        }
        if variant != ClassicVariant::V11
            && ui.button("Layers").clicked()
            && ui.ctx().current_pass_index() == 0
        {
            workspace.layers = !workspace.layers;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let settings = if variant == ClassicVariant::V11 {
                paintcraft_ui::widgets::classic_menu_button(
                    ui,
                    "Settings…",
                    Icon::Settings,
                    false,
                    "Settings…",
                )
            } else {
                paintcraft_ui::widgets::icon_button(ui, "Settings…", Icon::Settings, false, false)
            };
            if settings.clicked() {
                actions.push(Action::Settings);
            }
            actions.extend(paintcraft_ui::chrome::interface_changer(ui, context));
        });
    });
    let mut menu_bounds = menu.response.rect;
    if compact {
        // Quick actions stay visible with the menus even when the ribbon hides.
        // Keep Settings and the changer together without overlapping this row.
        let quick = quick_access::menu_bar(ui, variant).ui(ui, |ui| {
            quick_access::show(ui, variant, context, &mut actions);
        });
        menu_bounds = menu_bounds.union(quick.response.rect);
    }
    if !workspace
        .toolbar
        .update(ui, menu_bounds, context.auto_hide_toolbar)
    {
        ui.separator();
        return actions;
    }
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
    if variant == ClassicVariant::V11 {
        ribbon::ribbon(ui, state, workspace, context, &mut actions);
        return actions;
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
                    dropdowns::selection(ui, state, context, &mut actions);
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
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::Role::ColorWell,
                                        action.enabled(context) && ui.is_enabled(),
                                        label,
                                    )
                                });
                                if response.clicked() {
                                    actions.push(action);
                                }
                            }
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
                                    let response = response.on_hover_text(palette::tooltip(i, rgb));
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
                    if paintcraft_ui::actions::menu_item(ui, Action::EditColors, context) {
                        actions.push(Action::EditColors);
                    }
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
    paintcraft_ui::theme::apply(ctx, egui::Theme::Light);
    apply_style_for_theme(ctx, v, egui::Theme::Light);
}

pub fn apply_style_for_theme(ctx: &egui::Context, v: ClassicVariant, theme: egui::Theme) {
    ctx.style_mut_of(theme, |style| {
        let radius = if v == ClassicVariant::V11 { 5 } else { 0 };
        style.spacing.item_spacing = egui::vec2(6.0, 5.0);
        style.spacing.button_padding = egui::vec2(6.0, 4.0);
        for widget in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.corner_radius = egui::CornerRadius::same(radius);
        }
        if v == ClassicVariant::V7 && theme == egui::Theme::Light {
            style.visuals.panel_fill = Color32::from_rgb(233, 238, 247);
        }
    });
}

fn menu_layout(ui: &mut egui::Ui, variant: ClassicVariant) {
    if variant == ClassicVariant::V11 {
        ui.set_min_width(260.0);
        ui.spacing_mut().interact_size.y = 32.0;
    }
}
