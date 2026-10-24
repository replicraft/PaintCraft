//! Shared frontend controls and action intentions; the shell owns execution.

use crate::i18n::tr;
use crate::widgets::{Icon, icon_button};
use egui::Color32;
use paintcraft_core::types::Rgba;
use paintcraft_tools::{ShapeStyle, Tool, ToolOptions};

#[derive(Clone, Copy, Debug)]
pub struct Controls {
    pub tool: Tool,
    pub options: ToolOptions,
    pub primary: Rgba,
    pub secondary: Rgba,
}
pub use crate::actions::Action;

pub fn options(ui: &mut egui::Ui, state: &mut Controls, actions: &mut Vec<Action>) {
    options_with_shape_style(ui, state, actions, true);
}

/// Classic v11 supplies shape styles through its ribbon dropdowns.
pub fn options_with_shape_style(
    ui: &mut egui::Ui,
    state: &mut Controls,
    actions: &mut Vec<Action>,
    show_shape_style: bool,
) {
    ui.horizontal_wrapped(|ui| {
        if matches!(
            state.tool,
            Tool::Pencil
                | Tool::Brush
                | Tool::Eraser
                | Tool::Line
                | Tool::Rect
                | Tool::Ellipse
                | Tool::Preset(_)
                | Tool::Polygon
                | Tool::Curve
        ) {
            let size = if state.tool == Tool::Pencil {
                &mut state.options.pencil_size
            } else {
                &mut state.options.size
            };
            let slider = ui.add(
                egui::Slider::new(size, 1..=64)
                    .text(tr(ui.ctx(), "Size"))
                    .suffix(" px")
                    .show_value(false)
                    .clamping(egui::SliderClamping::Edits),
            );
            let accessible_label = tr(ui.ctx(), "Tool size");
            slider.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Slider, ui.is_enabled(), &accessible_label)
            });
            let value = ui.add(egui::DragValue::new(size).range(1..=1024).suffix(" px"));
            let accessible_label = tr(ui.ctx(), "Size");
            value.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::Role::SpinButton,
                    ui.is_enabled(),
                    &accessible_label,
                )
            });
        }
        if state.tool == Tool::Brush {
            ui.selectable_value(&mut state.options.round, true, tr(ui.ctx(), "Round"));
            ui.selectable_value(&mut state.options.round, false, tr(ui.ctx(), "Square"));
            if state.options.round {
                use paintcraft_core::types::brush::BrushMedia;
                egui::ComboBox::from_label(tr(ui.ctx(), "Brush medium"))
                    .selected_text(tr(ui.ctx(), state.options.media.name()))
                    .show_ui(ui, |ui| {
                        for media in BrushMedia::ALL {
                            ui.selectable_value(
                                &mut state.options.media,
                                media,
                                tr(ui.ctx(), media.name()),
                            );
                        }
                    });
            }
            ui.add(
                egui::Slider::new(&mut state.options.brush_opacity, 0..=100)
                    .text(tr(ui.ctx(), "Brush opacity"))
                    .suffix("%"),
            );
        }
        if show_shape_style
            && matches!(
                state.tool,
                Tool::Rect | Tool::Ellipse | Tool::Preset(_) | Tool::Polygon
            )
        {
            for (style, label) in [
                (ShapeStyle::Outline, "Outline"),
                (ShapeStyle::Filled, "Filled"),
                (ShapeStyle::Both, "Both"),
            ] {
                ui.selectable_value(&mut state.options.style, style, tr(ui.ctx(), label));
            }
        }
        if state.tool == Tool::Fill {
            let label = ui.label(tr(ui.ctx(), "Tolerance"));
            ui.add(egui::DragValue::new(&mut state.options.tolerance).range(0..=255))
                .labelled_by(label.id);
        }
        if state.tool == Tool::Text {
            ui.label(tr(
                ui.ctx(),
                "Command/Ctrl+Enter to place · Escape to cancel",
            ));
        }
        if state.tool == Tool::Pan {
            ui.label(tr(
                ui.ctx(),
                "Drag to pan · Hold Space to pan with another tool",
            ));
        }
        if state.tool == Tool::Magnifier {
            ui.label(tr(
                ui.ctx(),
                "Drag an area to zoom in · Right-click to zoom out",
            ));
        }
        if state.tool == Tool::Select {
            if ui.button(tr(ui.ctx(), "Select all")).clicked() {
                actions.push(Action::SelectAll);
            }
            if ui.button(tr(ui.ctx(), "Deselect")).clicked() {
                actions.push(Action::Deselect);
            }
        }
        if matches!(state.tool, Tool::Pencil) {
            ui.weak(tr(ui.ctx(), "Pencil · hard edge"));
        }
    });
}

pub fn tool_button(ui: &mut egui::Ui, state: &mut Controls, tool: Tool, large: bool) -> bool {
    let icon = match tool {
        Tool::Pencil => Icon::Pencil,
        Tool::Brush => Icon::Brush,
        Tool::Eraser => Icon::Eraser,
        Tool::Fill => Icon::Fill,
        Tool::ColorPicker => Icon::Eyedropper,
        Tool::Magnifier => Icon::ZoomIn,
        Tool::Pan => Icon::Pan,
        Tool::Line => Icon::Line,
        Tool::Rect => Icon::Rectangle,
        Tool::Ellipse => Icon::Ellipse,
        Tool::Polygon => Icon::Polygon,
        Tool::Curve => Icon::Curve,
        Tool::Preset(preset) => match preset {
            paintcraft_tools::ShapePreset::RoundedRectangle => Icon::RoundedRectangle,
            paintcraft_tools::ShapePreset::Triangle => Icon::Triangle,
            paintcraft_tools::ShapePreset::RightTriangle => Icon::RightTriangle,
            paintcraft_tools::ShapePreset::Diamond => Icon::Diamond,
            paintcraft_tools::ShapePreset::Pentagon => Icon::Pentagon,
            paintcraft_tools::ShapePreset::Hexagon => Icon::Hexagon,
            paintcraft_tools::ShapePreset::RightArrow => Icon::RightArrow,
            paintcraft_tools::ShapePreset::LeftArrow => Icon::LeftArrow,
            paintcraft_tools::ShapePreset::UpArrow => Icon::UpArrow,
            paintcraft_tools::ShapePreset::DownArrow => Icon::DownArrow,
            paintcraft_tools::ShapePreset::FourPointStar => Icon::FourPointStar,
            paintcraft_tools::ShapePreset::FivePointStar => Icon::Star,
            paintcraft_tools::ShapePreset::SixPointStar => Icon::SixPointStar,
            paintcraft_tools::ShapePreset::RectangularCallout => Icon::RoundedSpeechBubble,
            paintcraft_tools::ShapePreset::OvalCallout => Icon::SpeechBubble,
            paintcraft_tools::ShapePreset::CloudCallout => Icon::CloudCallout,
            paintcraft_tools::ShapePreset::Heart => Icon::Heart,
            paintcraft_tools::ShapePreset::Lightning => Icon::Lightning,
        },
        Tool::Select => Icon::Select,
        Tool::Text => Icon::Text,
    };
    let label = if tool == Tool::Magnifier {
        "Zoom tool"
    } else {
        tool.name()
    };
    if icon_button(ui, label, icon, state.tool == tool, large).clicked() {
        state.tool = tool;
        true
    } else {
        false
    }
}
pub fn color_edit(ui: &mut egui::Ui, label: &str, value: &mut Rgba) {
    let mut color = Color32::from_rgba_unmultiplied(value.0[0], value.0[1], value.0[2], value.0[3]);
    let response = ui.color_edit_button_srgba(&mut color);
    let label = tr(ui.ctx(), label);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ColorWell, ui.is_enabled(), &label));
    if response.changed() {
        *value = Rgba(color.to_srgba_unmultiplied());
    }
}

/// Kept next to Settings in either frontend; switching policy belongs to the host.
pub fn interface_changer(ui: &mut egui::Ui, context: crate::actions::Context) -> Vec<Action> {
    let mut actions = Vec::new();
    ui.menu_button(tr(ui.ctx(), "Interface"), |ui| {
        for frontend in [
            crate::actions::Frontend::Classic,
            crate::actions::Frontend::Studio,
        ] {
            let action = Action::ChooseFrontend(frontend);
            if crate::actions::menu_item(ui, action, context) {
                actions.push(action);
                ui.close();
            }
        }
        if context.frontend == crate::actions::Frontend::Classic {
            ui.separator();
            if ui.button(tr(ui.ctx(), "Change Classic skin…")).clicked() {
                actions.push(Action::Interface);
                ui.close();
            }
        }
    });
    actions
}

/// Studio exposes the distinct sampling choices; Classic has a one-shot image picker.
pub fn picker_options(ui: &mut egui::Ui, state: &mut Controls) {
    use paintcraft_core::types::picker::{PickerAfter, SampleSize, SampleSource};
    if state.tool != Tool::ColorPicker {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        let label = ui.label(tr(ui.ctx(), "Sampling source"));
        egui::ComboBox::from_id_salt("picker-source")
            .selected_text(tr(
                ui.ctx(),
                match state.options.picker_source {
                    SampleSource::Image => "Image",
                    SampleSource::Layer => "Layer",
                },
            ))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut state.options.picker_source,
                    SampleSource::Layer,
                    tr(ui.ctx(), "Layer"),
                );
                ui.selectable_value(
                    &mut state.options.picker_source,
                    SampleSource::Image,
                    tr(ui.ctx(), "Image"),
                );
            })
            .response
            .labelled_by(label.id);
        let label = ui.label(tr(ui.ctx(), "Sample size"));
        egui::ComboBox::from_id_salt("picker-size")
            .selected_text(format!(
                "{} × {}",
                state.options.picker_size.side(),
                state.options.picker_size.side()
            ))
            .show_ui(ui, |ui| {
                for size in SampleSize::ALL {
                    ui.selectable_value(
                        &mut state.options.picker_size,
                        size,
                        format!("{} × {}", size.side(), size.side()),
                    );
                }
            })
            .response
            .labelled_by(label.id);
        let label = ui.label(tr(ui.ctx(), "After picking"));
        egui::ComboBox::from_id_salt("picker-after")
            .selected_text(tr(
                ui.ctx(),
                match state.options.picker_after {
                    PickerAfter::Stay => "Keep color picker",
                    PickerAfter::Previous => "Previous tool",
                    PickerAfter::Pencil => "Switch to Pencil",
                },
            ))
            .show_ui(ui, |ui| {
                for (value, name) in [
                    (PickerAfter::Stay, "Keep color picker"),
                    (PickerAfter::Previous, "Previous tool"),
                    (PickerAfter::Pencil, "Switch to Pencil"),
                ] {
                    ui.selectable_value(&mut state.options.picker_after, value, tr(ui.ctx(), name));
                }
            })
            .response
            .labelled_by(label.id);
        ui.label(tr(ui.ctx(), "Ctrl-click samples the image"));
    });
}
