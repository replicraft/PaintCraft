//! Studio presentation. Actions are executed by the application shell.
mod history;
mod properties;
pub use properties::{LayerProperties, PropertiesResponse, layer_properties_dialog};
mod tabs;
mod toolbar;
mod windows;
pub use tabs::{DocumentTab, TabAction, document_tabs};

pub use history::{HistoryRow, history_window};
pub use toolbar::toolbar;
pub use windows::{floating_controls, layers_window};

#[derive(Clone, Debug)]
pub struct Workspace {
    pub tools: bool,
    pub colors: bool,
    pub layers: bool,
    pub history: bool,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            tools: true,
            colors: true,
            layers: true,
            history: true,
        }
    }
}

pub fn apply_style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Light);
    ctx.style_mut_of(egui::Theme::Light, |style| {
        style.spacing.item_spacing = egui::vec2(5.0, 4.0);
        style.spacing.button_padding = egui::vec2(5.0, 3.0);
        style.visuals.panel_fill = egui::Color32::from_rgb(242, 242, 242);
        style.visuals.window_fill = egui::Color32::from_rgb(250, 250, 250);
        style.visuals.window_corner_radius = egui::CornerRadius::same(3);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(0, 95, 184);
        style.visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    });
}
