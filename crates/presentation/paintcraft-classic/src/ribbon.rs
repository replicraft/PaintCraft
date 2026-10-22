//! Windows 11 ribbon geometry. Unimplemented controls are disabled prototypes.
use super::{Action, Controls, Workspace};
use egui::Vec2;
use paintcraft_tools::Tool;
use paintcraft_ui::{
    actions::Context,
    chrome::tool_button,
    widgets::{Icon, icon_button},
};

pub(super) fn ribbon(
    ui: &mut egui::Ui,
    state: &mut Controls,
    workspace: &mut Workspace,
    context: Context,
    actions: &mut Vec<Action>,
) {
    ui.separator();
    egui::ScrollArea::horizontal()
        .id_salt("windows11-ribbon")
        .max_height(96.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                group(ui, "Selection", |ui| {
                    if tool_button(ui, state, Tool::Select, true) {
                        actions.push(Action::SelectTool(Tool::Select));
                    }
                    super::dropdowns::selection(ui, state, context, actions);
                });
                ui.separator();
                group(ui, "Image", |ui| {
                    egui::Grid::new("image-ribbon")
                        .spacing(Vec2::splat(1.0))
                        .show(ui, |ui| {
                            prototype(ui, "Crop", Icon::Crop);
                            super::image_menus::rotate(ui);
                            if icon_button(ui, "Resize canvas", Icon::Resize, false, false)
                                .clicked()
                            {
                                actions.push(Action::CanvasSize);
                            }
                            ui.end_row();
                            prototype(ui, "Remove background", Icon::RemoveBackground);
                            super::image_menus::flip(ui);
                        });
                });
                ui.separator();
                group(ui, "Tools", |ui| {
                    egui::Grid::new("windows11-tools")
                        .spacing(Vec2::splat(1.0))
                        .show(ui, |ui| {
                            for tool in [Tool::Pencil, Tool::Fill, Tool::Text] {
                                if tool_button(ui, state, tool, false) {
                                    actions.push(Action::SelectTool(tool));
                                }
                            }
                            ui.end_row();
                            if tool_button(ui, state, Tool::Eraser, false) {
                                actions.push(Action::SelectTool(Tool::Eraser));
                            }
                            prototype(ui, "Color picker", Icon::Eyedropper);
                            if icon_button(ui, "Zoom to fit", Icon::FitWindow, false, false)
                                .clicked()
                            {
                                actions.push(Action::View(
                                    paintcraft_ui::actions::ViewAction::FitWindow,
                                ));
                            }
                        });
                });
                ui.separator();
                group(ui, "Brushes", |ui| {
                    if tool_button(ui, state, Tool::Brush, true) {
                        actions.push(Action::SelectTool(Tool::Brush));
                    }
                    super::dropdowns::brushes(ui, state, actions);
                });
                ui.separator();
                group(ui, "Shapes", |ui| {
                    ui.horizontal_top(|ui| {
                        egui::Frame::new()
                            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                            .corner_radius(4)
                            .inner_margin(2)
                            .show(ui, |ui| {
                                ui.spacing_mut().button_padding = Vec2::splat(2.0);
                                egui::ScrollArea::vertical()
                                    .id_salt("classic-shapes-gallery")
                                    .max_height(78.0)
                                    .max_width(132.0)
                                    .show(ui, |ui| {
                                        egui::Grid::new("windows11-shapes")
                                            .min_row_height(24.0)
                                            .min_col_width(24.0)
                                            .spacing(Vec2::ZERO)
                                            .show(ui, |ui| {
                                                for (i, icon) in [
                                                    Icon::Line,
                                                    Icon::Curve,
                                                    Icon::Ellipse,
                                                    Icon::Rectangle,
                                                    Icon::RoundedRectangle,
                                                    Icon::Polygon,
                                                    Icon::Triangle,
                                                    Icon::RightTriangle,
                                                    Icon::Diamond,
                                                    Icon::Pentagon,
                                                    Icon::Hexagon,
                                                    Icon::RightArrow,
                                                    Icon::LeftArrow,
                                                    Icon::UpArrow,
                                                    Icon::DownArrow,
                                                    Icon::FourPointStar,
                                                    Icon::Star,
                                                    Icon::SixPointStar,
                                                    Icon::RectangularCallout,
                                                    Icon::RoundedSpeechBubble,
                                                    Icon::SpeechBubble,
                                                    Icon::CloudCallout,
                                                    Icon::Heart,
                                                    Icon::Lightning,
                                                    Icon::Burst,
                                                ]
                                                .into_iter()
                                                .enumerate()
                                                {
                                                    let tool = match icon {
                                                        Icon::Line => Some(Tool::Line),
                                                        Icon::Ellipse => Some(Tool::Ellipse),
                                                        Icon::Rectangle => Some(Tool::Rect),
                                                        _ => None,
                                                    };
                                                    if let Some(tool) = tool {
                                                        if compact_icon(
                                                            ui,
                                                            tool.name(),
                                                            icon,
                                                            state.tool == tool,
                                                        )
                                                        .clicked()
                                                        {
                                                            state.tool = tool;
                                                            actions.push(Action::SelectTool(tool));
                                                        }
                                                    } else {
                                                        ui.add_enabled_ui(false, |ui| {
                                                            compact_icon(
                                                                ui,
                                                                icon.name(),
                                                                icon,
                                                                false,
                                                            )
                                                        })
                                                        .inner
                                                        .on_disabled_hover_text(
                                                            "UI preview · shape not implemented",
                                                        );
                                                    }
                                                    if i % 5 == 4 {
                                                        ui.end_row();
                                                    }
                                                }
                                            });
                                    });
                            });
                        ui.vertical(|ui| super::dropdowns::shape_paint(ui, state));
                    });
                });
                ui.separator();
                group(ui, "Colors", |ui| {
                    super::palette::colors(ui, state, &workspace.custom_colors, context, actions)
                });
                ui.separator();
                group(ui, "Layers", |ui| {
                    let response = ui
                        .allocate_ui_with_layout(
                            Vec2::new(60.0, 60.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                let response = ui.add(
                                    egui::Button::image(paintcraft_ui::icons::image(
                                        ui.ctx(),
                                        Icon::Layers,
                                        44.0,
                                    ))
                                    .min_size(Vec2::new(60.0, 60.0))
                                    .selected(workspace.layers)
                                    .frame_when_inactive(workspace.layers)
                                    .frame(true),
                                );
                                response.widget_info(|| {
                                    egui::WidgetInfo::selected(
                                        egui::Role::Button,
                                        ui.is_enabled(),
                                        workspace.layers,
                                        "Layers",
                                    )
                                });
                                response.on_hover_text("Layers")
                            },
                        )
                        .inner;
                    if response.clicked() && ui.ctx().current_pass_index() == 0 {
                        workspace.layers = !workspace.layers;
                    }
                });
            });
        });
    ui.separator();
    if state.tool != Tool::Text && !super::tool_controls::supports(state.tool) {
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 40.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_min_height(40.0);
                paintcraft_ui::chrome::options_with_shape_style(ui, state, actions, false)
            },
        );
    }
}

fn group(ui: &mut egui::Ui, label: &str, contents: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        let top = ui.cursor().min.y;
        contents(ui);
        ui.add_space((top + 96.0 - ui.cursor().min.y).max(0.0));
        let width = ui.min_rect().width();
        ui.allocate_ui_with_layout(
            Vec2::new(width, 15.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.label(
                    egui::RichText::new(label)
                        .size(12.0)
                        .color(ui.visuals().weak_text_color()),
                );
            },
        );
    });
}

fn prototype(ui: &mut egui::Ui, label: &str, icon: Icon) {
    ui.add_enabled_ui(false, |ui| {
        paintcraft_ui::widgets::icon_button_with_tooltip(
            ui,
            label,
            icon,
            false,
            false,
            format!("UI preview · {label} is not implemented"),
        )
    });
}

fn compact_icon(ui: &mut egui::Ui, label: &str, icon: Icon, selected: bool) -> egui::Response {
    let response = ui
        .scope(|ui| {
            // Compact shape cells keep three visible rows even when the main
            // toolbar uses larger Windows-style interaction targets.
            ui.spacing_mut().interact_size = Vec2::splat(24.0);
            ui.add(
                egui::Button::new(paintcraft_ui::icons::image(ui.ctx(), icon, 19.0))
                    .selected(selected)
                    .min_size(Vec2::splat(24.0))
                    .frame(selected),
            )
        })
        .inner;
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, label)
    });
    response.on_hover_text(label)
}
