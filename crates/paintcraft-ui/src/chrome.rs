//! Shared frontend controls and action intentions; the shell owns execution.

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
    ui.horizontal_wrapped(|ui| {
        if matches!(
            state.tool,
            Tool::Brush | Tool::Eraser | Tool::Line | Tool::Rect | Tool::Ellipse
        ) {
            let label = ui.label("Size");
            ui.add(
                egui::DragValue::new(&mut state.options.size)
                    .range(1..=1024)
                    .suffix(" px"),
            )
            .labelled_by(label.id);
        }
        if state.tool == Tool::Brush {
            ui.selectable_value(&mut state.options.round, true, "Round");
            ui.selectable_value(&mut state.options.round, false, "Square");
        }
        if matches!(state.tool, Tool::Rect | Tool::Ellipse) {
            for (style, label) in [
                (ShapeStyle::Outline, "Outline"),
                (ShapeStyle::Filled, "Filled"),
                (ShapeStyle::Both, "Both"),
            ] {
                ui.selectable_value(&mut state.options.style, style, label);
            }
        }
        if state.tool == Tool::Fill {
            let label = ui.label("Tolerance");
            ui.add(egui::DragValue::new(&mut state.options.tolerance).range(0..=255))
                .labelled_by(label.id);
        }
        if state.tool == Tool::Text {
            let label = ui.label("Text size");
            ui.add(
                egui::DragValue::new(&mut state.options.text_scale)
                    .range(1..=16)
                    .suffix("×"),
            )
            .labelled_by(label.id);
            ui.label("Ctrl+Enter to place · Escape to cancel");
        }
        if state.tool == Tool::Select {
            if ui.button("Select all").clicked() {
                actions.push(Action::SelectAll);
            }
            if ui.button("Deselect").clicked() {
                actions.push(Action::Deselect);
            }
        }
        if matches!(state.tool, Tool::Pencil) {
            ui.weak("Pencil · 1 pixel");
        }
    });
}

pub fn tool_button(ui: &mut egui::Ui, state: &mut Controls, tool: Tool, large: bool) -> bool {
    let icon = match tool {
        Tool::Pencil => Icon::Pencil,
        Tool::Brush => Icon::Brush,
        Tool::Eraser => Icon::Eraser,
        Tool::Fill => Icon::Fill,
        Tool::Line => Icon::Line,
        Tool::Rect => Icon::Rectangle,
        Tool::Ellipse => Icon::Ellipse,
        Tool::Select => Icon::Select,
        Tool::Text => Icon::Text,
    };
    if icon_button(ui, tool.name(), icon, state.tool == tool, large)
        .on_hover_text(tool.name())
        .clicked()
    {
        state.tool = tool;
        true
    } else {
        false
    }
}
pub fn color_edit(ui: &mut egui::Ui, label: &str, value: &mut Rgba) {
    let mut color = Color32::from_rgba_unmultiplied(value.0[0], value.0[1], value.0[2], value.0[3]);
    let response = ui.color_edit_button_srgba(&mut color);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ColorWell, ui.is_enabled(), label));
    if response.changed() {
        *value = Rgba(color.to_srgba_unmultiplied());
    }
}
