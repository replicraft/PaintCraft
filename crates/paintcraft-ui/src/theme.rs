//! Shared Windows-style surface colors; frontend layout remains separate.
use egui::{Color32, Theme};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    pub fn id(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
    pub fn parse(id: &str) -> Option<Self> {
        match id {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
    pub fn resolve(self, system: Option<Theme>) -> Theme {
        match self {
            Self::System => system.unwrap_or(Theme::Light),
            Self::Light => Theme::Light,
            Self::Dark => Theme::Dark,
        }
    }
}

pub fn workspace(theme: Theme) -> Color32 {
    match theme {
        Theme::Dark => Color32::from_rgb(30, 30, 32),
        Theme::Light => Color32::from_rgb(222, 222, 222),
    }
}

pub fn apply(ctx: &egui::Context, theme: Theme) {
    let dark = theme == Theme::Dark;
    let mut style = egui::Style {
        visuals: if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        },
        ..Default::default()
    };
    style.visuals.panel_fill = if dark {
        Color32::from_rgb(39, 39, 39)
    } else {
        Color32::from_rgb(249, 249, 249)
    };
    style.visuals.window_fill = if dark {
        Color32::from_rgb(43, 43, 43)
    } else {
        Color32::from_rgb(249, 249, 249)
    };
    style.visuals.extreme_bg_color = if dark {
        Color32::from_rgb(30, 30, 30)
    } else {
        Color32::WHITE
    };
    style.visuals.selection.bg_fill = if dark {
        Color32::from_rgb(30, 67, 85)
    } else {
        Color32::from_rgb(224, 239, 249)
    };
    style.visuals.selection.stroke = egui::Stroke::new(
        1.0,
        if dark {
            Color32::from_rgb(76, 194, 255)
        } else {
            Color32::from_rgb(0, 103, 192)
        },
    );
    style.visuals.window_corner_radius = egui::CornerRadius::same(8);
    style.visuals.widgets.inactive.weak_bg_fill = if dark {
        Color32::from_rgb(51, 51, 51)
    } else {
        Color32::from_rgb(240, 240, 240)
    };
    style.visuals.widgets.inactive.bg_fill = style.visuals.widgets.inactive.weak_bg_fill;
    style.visuals.widgets.hovered.weak_bg_fill = if dark {
        Color32::from_rgb(61, 61, 61)
    } else {
        Color32::from_rgb(229, 229, 229)
    };
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(5);
    }
    ctx.set_style_of(theme, style);
    ctx.set_theme(theme);
}
