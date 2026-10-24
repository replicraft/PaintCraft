//! Studio presentation. Actions are executed by the application shell.
pub mod canvas;
mod canvas_size;
pub use canvas_size::{CanvasResponse, CanvasSize, canvas_size_dialog};
mod colors;
mod history;
mod properties;
pub use properties::{LayerProperties, PropertiesResponse, layer_properties_dialog};
mod image_menu;
mod shapes;
mod tabs;
mod toolbar;
mod windows;
pub use tabs::{DocumentTab, TabAction, document_tabs};

pub use history::{HistoryRow, history_window};
pub use toolbar::toolbar;
pub use windows::{floating_controls, layers_window};

#[derive(Clone, Debug)]
pub struct Workspace {
    pub canvas_top: f32,
    pub tools: bool,
    pub colors: bool,
    pub layers: bool,
    pub history: bool,
    pub edit_secondary: bool,
    pub more_colors: bool,
    pub zoom: paintcraft_tools::navigation::ZoomGesture<egui::Id>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            canvas_top: 150.0,
            tools: true,
            colors: true,
            layers: true,
            history: true,
            edit_secondary: false,
            more_colors: false,
            zoom: Default::default(),
        }
    }
}

pub fn apply_style(ctx: &egui::Context) {
    paintcraft_ui::theme::apply(ctx, egui::Theme::Light);
    apply_style_for_theme(ctx, egui::Theme::Light);
}

pub fn apply_style_for_theme(ctx: &egui::Context, theme: egui::Theme) {
    ctx.style_mut_of(theme, |style| {
        style.spacing.item_spacing = egui::vec2(5.0, 4.0);
        style.spacing.button_padding = egui::vec2(5.0, 3.0);
        if theme == egui::Theme::Light {
            style.visuals.panel_fill = egui::Color32::from_rgb(242, 242, 242);
            style.visuals.window_fill = egui::Color32::from_rgb(250, 250, 250);
        }
        style.visuals.window_corner_radius = egui::CornerRadius::same(3);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(0, 95, 184);
        style.visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    });
}
