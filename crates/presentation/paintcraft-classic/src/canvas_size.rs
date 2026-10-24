//! Canvas dimension/default presentation; document and preference writes stay in the host.
use egui::{Color32, Vec2};
use paintcraft_ui::t;
mod defaults;
pub use defaults::{CanvasDefaults, DEFAULT_SIZE, SIZE_MAX};
mod fields;
mod legacy;
pub use fields::dimension_fields;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartupSize {
    pub remember_last: bool,
    pub last: Option<(u32, u32)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasSize {
    pub size: (u32, u32),
    /// Present when editing New-document defaults rather than the current image.
    pub startup: Option<StartupSize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasResponse {
    Pending,
    Apply,
    Cancel,
}

pub fn canvas_size_dialog(
    ctx: &egui::Context,
    draft: &mut CanvasSize,
    max_dimension: u32,
) -> CanvasResponse {
    dialog(ctx, draft, max_dimension, None, None)
}

/// Pixel dimensions use the existing canvas operation; unsupported metadata and
/// conversions stay explicit instead of inventing a resolution or color mode.
pub fn image_properties_dialog(
    ctx: &egui::Context,
    size: &mut (u32, u32),
    name: &str,
    max_dimension: u32,
) -> CanvasResponse {
    let mut draft = CanvasSize {
        size: *size,
        startup: None,
    };
    let response = dialog(ctx, &mut draft, max_dimension, Some(name), None);
    *size = draft.size;
    response
}

/// Use the same staged dimension/properties surface with a constrained host's
/// output-pixel admission. Invalid dimensions cannot emit Apply.
pub fn bounded_canvas_size_dialog(
    ctx: &egui::Context,
    draft: &mut CanvasSize,
    max_dimension: u32,
    max_pixels: u64,
    properties: Option<&str>,
) -> CanvasResponse {
    dialog(ctx, draft, max_dimension, properties, Some(max_pixels))
}

/// Skin-specific presentation with optional host pixel admission. Startup policy
/// keeps its dedicated preferences surface; document properties use compact
/// historical controls in the earlier skins.
pub fn canvas_size_dialog_for_skin(
    ctx: &egui::Context,
    variant: crate::ClassicVariant,
    draft: &mut CanvasSize,
    max_dimension: u32,
    max_pixels: Option<u64>,
    properties: Option<&str>,
) -> CanvasResponse {
    if variant != crate::ClassicVariant::V11 && draft.startup.is_none() {
        legacy::show(ctx, draft, max_dimension, max_pixels, properties)
    } else {
        dialog(ctx, draft, max_dimension, properties, max_pixels)
    }
}

fn dialog(
    ctx: &egui::Context,
    draft: &mut CanvasSize,
    max_dimension: u32,
    properties: Option<&str>,
    max_pixels: Option<u64>,
) -> CanvasResponse {
    let defaults = draft.startup.is_some();
    let title = if properties.is_some() {
        "Image properties"
    } else if defaults {
        "Default canvas size"
    } else {
        "Canvas size"
    };
    let width = (ctx.content_rect().width() - 56.0).clamp(240.0, 520.0);
    let height = (ctx.content_rect().height() - 72.0)
        .clamp(180.0, if properties.is_some() { 600.0 } else { 520.0 });
    let mut result = CanvasResponse::Pending;
    let frame = egui::Frame::window(&ctx.style_of(ctx.theme()))
        .inner_margin(16)
        .corner_radius(12);
    let modal =
        egui::Modal::new(egui::Id::unique(title))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.set_max_height(height);
                ui.spacing_mut().interact_size.y = 40.0;
                ui.spacing_mut().icon_width = 22.0;
                for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                    ui.style_mut()
                        .text_styles
                        .insert(style, egui::FontId::proportional(16.0));
                }
                ui.visuals_mut().override_text_color = Some(ui.visuals().strong_text_color());
                ui.visuals_mut().weak_text_color =
                    Some(Color32::from_gray(if ui.visuals().dark_mode {
                        184
                    } else {
                        88
                    }));
                let close_label = t!(ui.ctx(), "Close { $title }", &[("title", t!(ui.ctx(), title).to_lowercase())]);
                let header = ui.horizontal_top(|ui| {
                    let heading_width = width - 40.0 - ui.spacing().item_spacing.x;
                    ui.allocate_ui_with_layout(
                        Vec2::new(heading_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_width(heading_width);
                            ui.label(egui::RichText::new(t!(ui.ctx(), title)).size(24.0).strong());
                        },
                    );
                    if paintcraft_ui::widgets::classic_menu_button(
                        ui,
                        &close_label,
                        paintcraft_ui::icons::Icon::Close,
                        false,
                        &close_label,
                    )
                    .clicked()
                    {
                        result = CanvasResponse::Cancel;
                    }
                });
                ui.add_space(12.0);
                // A visible reserved scrollbar makes lower policy choices discoverable
                // on short displays without covering dimension fields.
                ui.spacing_mut().scroll.floating = false;
                egui::ScrollArea::vertical()
            .id_salt("canvas-size-body")
            .max_height((height - header.response.rect.height() - 84.0).max(48.0))
            .min_scrolled_height(48.0)
                .auto_shrink([false, true])
            .show(ui, |ui| {
                crate::form_controls::style(ui);
                if let Some(name) = properties {
                    properties_info(ui, name);
                    ui.add_space(12.0);
                }
                egui::Frame::group(ui.style())
                    .corner_radius(6)
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width().max(0.0));
                        ui.strong(t!(ui.ctx(), "Dimensions"));
                        ui.add_space(8.0);
                        if properties.is_some() {
                            ui.horizontal_wrapped(|ui| {
                                for label in ["Inches", "Centimeters"] {
                                    ui.add_enabled(false, egui::RadioButton::new(false, t!(ui.ctx(), label)))
                                        .on_disabled_hover_text(t!(ui.ctx(), "Physical units require image resolution metadata, which is not available for this document."));
                                }
                                let _ = ui.radio(true, t!(ui.ctx(), "Pixels"));
                            });
                            ui.add_space(8.0);
                        }
                        dimension_fields(ui, &mut draft.size, max_dimension, ["Width", "Height"]);
                    });
                if let Some(maximum) = max_pixels {
                    ui.add_space(8.0);
                    let too_large = u64::from(draft.size.0) * u64::from(draft.size.1) > maximum;
                    let label = t!(ui.ctx(), "Image limit: { $maximum } pixels", &[("maximum", maximum.to_string())]);
                    if too_large { ui.colored_label(ui.visuals().error_fg_color,label); }
                    else { ui.label(t!(ui.ctx(), label)); }
                }
                if properties.is_some() {
                    ui.add_space(12.0);
                    ui.strong(t!(ui.ctx(), "Colors"));
                    ui.horizontal_wrapped(|ui| {
                        ui.add_enabled(false, egui::RadioButton::new(false, t!(ui.ctx(), "Black and white")))
                            .on_disabled_hover_text(t!(ui.ctx(), "Monochrome conversion is not available yet."));
                        let _ = ui.radio(true, t!(ui.ctx(), "Color"));
                    });
                }
                ui.add_space(12.0);
                if let Some(startup) = &mut draft.startup {
                    ui.strong(t!(ui.ctx(), "When you create a new image"));
                    ui.radio_value(
                        &mut startup.remember_last,
                        true,
                        t!(ui.ctx(), "Remember last canvas size"),
                    );
                    ui.radio_value(
                        &mut startup.remember_last,
                        false,
                        t!(ui.ctx(), "Use configured default size"),
                    );
                    if let Some((w, h)) = startup.last {
                        ui.weak(t!(ui.ctx(), "Last canvas: { $width } × { $height } pixels", &[("width", w.to_string()), ("height", h.to_string())]));
                    }
                    ui.add_space(8.0);
                    ui.label(t!(ui.ctx(), "Applies to new Classic documents. The current image stays as it is."));
                } else {
                    ui.label(t!(ui.ctx(), "Keep the top-left corner. Content outside the canvas is preserved."));
                }
            });
                ui.add_space(12.0);
                ui.separator();
                let button_width = (width - ui.spacing().item_spacing.x) * 0.5;
                ui.horizontal(|ui| {
                    let ink = if ui.visuals().dark_mode {
                        Color32::from_gray(16)
                    } else {
                        Color32::WHITE
                    };
                    let valid = (1..=max_dimension).contains(&draft.size.0)
                        && (1..=max_dimension).contains(&draft.size.1)
                        && max_pixels.is_none_or(|maximum| u64::from(draft.size.0) * u64::from(draft.size.1) <= maximum);
                    let apply = egui::Button::new(egui::RichText::new(t!(ui.ctx(), if properties.is_some() {"OK"} else {"Apply"})).color(ink))
                        .fill(ui.visuals().selection.stroke.color)
                        .min_size(Vec2::new(button_width, 40.0));
                    if ui.add_enabled(valid, apply).clicked() {
                        result = CanvasResponse::Apply;
                    }
                    if ui
                        .add_sized([button_width, 40.0], egui::Button::new(t!(ui.ctx(), "Cancel")))
                        .clicked()
                    {
                        result = CanvasResponse::Cancel;
                    }
                });
            });
    if modal.should_close() {
        result = CanvasResponse::Cancel;
    }
    result
}

fn properties_info(ui: &mut egui::Ui, name: &str) {
    ui.strong(t!(ui.ctx(), "Image"));
    let value_width = (ui.available_width() - 104.0 - ui.spacing().item_spacing.x).max(40.0);
    egui::Grid::new("image-properties-attributes")
        .max_col_width(value_width)
        .show(ui, |ui| {
            ui.label(t!(ui.ctx(), "File name"));
            ui.add(
                egui::Label::new(name)
                    .truncate()
                    .show_tooltip_when_elided(false),
            )
            .on_hover_text(name);
            ui.end_row();
            ui.label(t!(ui.ctx(), "Resolution"));
            ui.label(t!(ui.ctx(), "Not available")).on_hover_text(t!(
                ui.ctx(),
                "No image resolution metadata is stored for this document.",
            ));
            ui.end_row();
        });
}
