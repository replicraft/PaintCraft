//! Classic presentation emits actions; editor and platform services own effects.
mod brush_gallery;
pub mod canvas;
mod canvas_handles;
mod canvas_scroll;
pub mod canvas_size;
mod ruler_axis;
mod rulers;
pub mod status;
pub mod thumbnail;
pub mod tool_controls;
pub use rulers::RULER_SIZE;
mod control_surface;
pub use control_surface::button as selected_button;
mod dropdowns;
mod form_controls;
pub use form_controls::style as form_control_style;
mod image_menus;
pub mod layers;
mod menus;
pub mod resize_skew;
mod toolbar_visibility;
pub use toolbar_visibility::ToolbarVisibility;
mod legacy_brushes;
mod legacy_chrome;
mod legacy_controls;
mod legacy_file;
mod legacy_palette;
pub use legacy_palette::PaletteTarget;
mod legacy_ribbon;
mod legacy_size;
mod legacy_surface;
mod palette;
mod popup_layout;
mod quick_access;
mod ribbon;
mod selection_menu;
mod shapes;
pub mod skin_picker;
mod slider_input;
mod slider_thumb;
mod stroke_size;
#[derive(Debug, Default)]
pub struct Workspace {
    pub layers: bool,
    pub thumbnail: bool,
    /// Presentation-only Home/View tab for the historical ribbons.
    pub legacy_view: bool,
    pub toolbar: ToolbarVisibility,
    /// Contextual Text tab is independent of the Home/View choice.
    pub legacy_text: bool,
    /// Active color well in the historical palette.
    pub palette_target: PaletteTarget,
    #[doc(hidden)]
    pub legacy_tool: Option<paintcraft_tools::Tool>,
    /// Current persisted custom colors supplied by the desktop host.
    pub custom_colors: CustomColors,
}
mod text;
pub use text::{TextAlignment, TextControls, TextOptions, font_family, text_options};
mod colors;
pub use colors::{
    CUSTOM_COLOR_COUNT, ColorEditor, ColorResponse, CustomColors, edit_colors_dialog,
};
mod zoom;
pub use zoom::zoom_controls;

use egui::Color32;
use paintcraft_core::types::Rgba;
use paintcraft_ui::i18n::tr;
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

pub fn chrome(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: paintcraft_ui::actions::Context,
) -> Vec<Action> {
    chrome_with_text(ui, variant, state, workspace, context, None)
}

pub fn chrome_with_text(
    ui: &mut egui::Ui,
    variant: ClassicVariant,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: paintcraft_ui::actions::Context,
    text: Option<TextControls<'_>>,
) -> Vec<Action> {
    if variant != ClassicVariant::V11 {
        return legacy_chrome::show(ui, variant, state, workspace, context, text);
    }
    let mut actions = Vec::new();
    for text in [egui::TextStyle::Body, egui::TextStyle::Button] {
        ui.style_mut()
            .text_styles
            .insert(text, egui::FontId::proportional(15.0));
    }
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    ui.spacing_mut().interact_size.y = 32.0;
    let compact = ui.available_width() < 720.0;
    let menu = quick_access::menu_bar(ui, variant).ui(ui, |ui| {
        ui.menu_button(tr(ui.ctx(), "File"), |ui| {
            menu_layout(ui, variant);
            menus::file(ui, context, &mut actions);
        });
        ui.add_space(6.0);
        ui.menu_button(tr(ui.ctx(), "Edit"), |ui| {
            menu_layout(ui, variant);
            menus::edit(ui, context, &mut actions);
        });
        ui.add_space(6.0);
        ui.menu_button(tr(ui.ctx(), "View"), |ui| {
            menu_layout(ui, variant);
            menus::view(ui, variant, context, &mut actions);
        });
        if !compact {
            ui.separator();
            quick_access::show(ui, variant, context, &mut actions);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if paintcraft_ui::widgets::classic_menu_button(
                ui,
                "Settings…",
                Icon::Settings,
                false,
                "Settings…",
            )
            .clicked()
            {
                actions.push(Action::Settings);
            }
            actions.extend(paintcraft_ui::chrome::interface_changer(ui, context));
        });
    });
    let mut menu_bounds = menu.response.rect;
    if compact {
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
    ribbon::ribbon(ui, state, workspace, context, &mut actions);
    actions
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
        if v == ClassicVariant::V10 && theme == egui::Theme::Light {
            style.visuals.panel_fill = Color32::from_rgb(245, 246, 247);
            style.visuals.selection.bg_fill = Color32::from_rgb(204, 232, 255);
            style.visuals.selection.stroke = egui::Stroke::new(1.0, Color32::from_rgb(0, 120, 215));
        }
        if v == ClassicVariant::V7 && theme == egui::Theme::Light {
            style.visuals.panel_fill = Color32::from_rgb(233, 238, 247);
            style.visuals.selection.bg_fill = Color32::from_rgb(255, 232, 166);
            style.visuals.selection.stroke =
                egui::Stroke::new(1.0, Color32::from_rgb(194, 140, 43));
            style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(255, 242, 203);
            style.visuals.widgets.hovered.bg_stroke =
                egui::Stroke::new(1.0, Color32::from_rgb(226, 187, 92));
        }
    });
}

fn menu_layout(ui: &mut egui::Ui, variant: ClassicVariant) {
    if variant == ClassicVariant::V11 {
        ui.set_min_width(260.0);
        ui.spacing_mut().interact_size.y = 32.0;
    }
}

/// Windows 7's blue-gray workspace surrounds the centered document.
pub fn workspace_color(variant: ClassicVariant, theme: egui::Theme) -> Color32 {
    if variant == ClassicVariant::V7 && theme == egui::Theme::Light {
        Color32::from_rgb(190, 205, 222)
    } else {
        paintcraft_ui::theme::workspace(theme)
    }
}
