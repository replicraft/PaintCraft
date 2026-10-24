//! Compact square controls used by the historical ribbon, independent of v11 metrics.
use super::{Action, Controls};
use egui::{Rect, Response, Vec2};
use paintcraft_tools::{ShapeStyle, Tool};
use paintcraft_ui::{actions::Context, i18n::tr, icons::Icon};

pub(super) fn icon_button(
    ui: &mut egui::Ui,
    context: Context,
    action: Action,
    icon: Icon,
    size: f32,
) -> Response {
    let label = tr(
        ui.ctx(),
        if icon == Icon::Paste {
            "Paste"
        } else {
            action.descriptor_for(context.frontend).label
        },
    );
    let response = ui.add_enabled(
        action.enabled(context),
        egui::Button::new(paintcraft_ui::icons::legacy_image(ui.ctx(), icon, size))
            .min_size(Vec2::splat(size + 4.0))
            .frame_when_inactive(false),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            action.enabled(context) && ui.is_enabled(),
            &label,
        )
    });
    response.on_hover_text(label)
}

pub(super) fn large_button(
    ui: &mut egui::Ui,
    context: Context,
    action: Action,
    icon: Icon,
    caption: &str,
) -> Response {
    let label = tr(
        ui.ctx(),
        if icon == Icon::Paste {
            "Paste"
        } else {
            action.descriptor_for(context.frontend).label
        },
    );
    let caption = tr(ui.ctx(), caption).replacen(' ', "\n", 1);
    let font = egui::TextStyle::Body.resolve(ui.style());
    let width = ui
        .painter()
        .layout_no_wrap(caption.clone(), font.clone(), ui.visuals().text_color())
        .size()
        .x
        + 8.0;
    let response = ui.add_enabled(
        action.enabled(context),
        egui::Button::new("")
            .min_size(Vec2::new(width.max(50.0), 64.0))
            .frame_when_inactive(false),
    );
    paintcraft_ui::icons::legacy_image(ui.ctx(), icon, 28.0).paint_at(
        ui,
        Rect::from_center_size(
            response.rect.center_top() + Vec2::new(0.0, 19.0),
            Vec2::splat(28.0),
        ),
    );
    ui.painter().text(
        response.rect.center_bottom() - Vec2::new(0.0, 15.0),
        egui::Align2::CENTER_CENTER,
        caption,
        font,
        ui.visuals().text_color(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            action.enabled(context) && ui.is_enabled(),
            &label,
        )
    });
    response.on_hover_text(label)
}

pub(super) fn tools(
    ui: &mut egui::Ui,
    state: &mut Controls,
    context: Context,
    actions: &mut Vec<Action>,
) {
    egui::Grid::new("legacy-tools").spacing(Vec2::new(2.0, 3.0)).show(ui, |ui| {
        for (index, tool) in [Some(Tool::Pencil), Some(Tool::Fill), Some(Tool::Text), Some(Tool::Eraser), Some(Tool::ColorPicker), None].into_iter().enumerate() {
            if let Some(tool) = tool {
                let action = Action::SelectTool(tool);
                let icon = match tool { Tool::Pencil => Icon::Pencil, Tool::Fill => Icon::Bucket, Tool::Text => Icon::Text, Tool::Eraser => Icon::Eraser, _ => Icon::Eyedropper };
                let label = tr(ui.ctx(), tool.name());
                let response = ui.add_enabled(action.enabled(context), egui::Button::new(paintcraft_ui::icons::legacy_image(ui.ctx(), icon, 16.0))
                    .min_size(Vec2::splat(22.0)).selected(state.tool == tool).frame_when_inactive(state.tool == tool));
                response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, action.enabled(context) && ui.is_enabled(), state.tool == tool, &label));
                if response.on_hover_text(label).clicked() { state.tool = tool; actions.push(action); }
            } else {
                let response = ui.add_enabled(false, egui::Button::new(paintcraft_ui::icons::legacy_image(ui.ctx(), Icon::ZoomIn, 16.0)).min_size(Vec2::splat(22.0)).frame_when_inactive(false));
                let label = tr(ui.ctx(), "Magnifier");
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, false, &label));
                response
                    .on_disabled_hover_text(tr(ui.ctx(), "The magnifier tool is not available yet. Use the status bar or the View menu's zoom controls to change magnification."));
            }
            if index % 3 == 2 { ui.end_row(); }
        }
    });
}

pub(super) fn selection(
    ui: &mut egui::Ui,
    state: &mut Controls,
    context: Context,
    actions: &mut Vec<Action>,
) {
    ui.vertical(|ui| {
        if large_button(
            ui,
            context,
            Action::SelectTool(Tool::Select),
            Icon::RectangleSelect,
            "Select",
        )
        .clicked()
        {
            state.tool = Tool::Select;
            actions.push(Action::SelectTool(Tool::Select));
        }
        ui.spacing_mut().interact_size.y = 12.0;
        let (response, _) = egui::containers::menu::MenuButton::from_button(
            egui::Button::new(" ")
                .min_size(Vec2::new(50.0, 12.0))
                .frame_when_inactive(false),
        )
        .ui(ui, |ui| {
            super::selection_menu::show(ui, state, context, actions)
        });
        arrow(ui, response.rect.center());
        super::dropdowns::open_state(ui, &response, "Selection options");
    });
}

pub(super) fn shape_styles(ui: &mut egui::Ui, state: &mut Controls) {
    let enabled = matches!(
        state.tool,
        Tool::Rect | Tool::Ellipse | Tool::Preset(_) | Tool::Polygon
    );
    for (outline, title, icon) in [
        (true, "Outline", Icon::Pencil),
        (false, "Fill", Icon::Bucket),
    ] {
        ui.add_enabled_ui(enabled, |ui| {
            ui.menu_button(
                (
                    paintcraft_ui::icons::legacy_image(ui.ctx(), icon, 14.0),
                    tr(ui.ctx(), title),
                ),
                |ui| {
                    for (solid, label) in [
                        (false, if outline { "No outline" } else { "No fill" }),
                        (
                            true,
                            if outline {
                                "Solid outline"
                            } else {
                                "Solid fill"
                            },
                        ),
                    ] {
                        let selected = if outline {
                            state.options.style != ShapeStyle::Filled
                        } else {
                            state.options.style != ShapeStyle::Outline
                        };
                        if ui
                            .selectable_label(selected == solid, tr(ui.ctx(), label))
                            .clicked()
                        {
                            state.options.style = if outline {
                                if !solid {
                                    ShapeStyle::Filled
                                } else if state.options.style == ShapeStyle::Filled {
                                    ShapeStyle::Both
                                } else {
                                    state.options.style
                                }
                            } else if !solid {
                                ShapeStyle::Outline
                            } else if state.options.style == ShapeStyle::Outline {
                                ShapeStyle::Both
                            } else {
                                state.options.style
                            };
                            ui.close();
                        }
                    }
                },
            );
        });
    }
}

pub(super) fn size(ui: &mut egui::Ui, state: &mut Controls, context: Context) {
    let enabled =
        super::tool_controls::supports(state.tool) && context.document_open && !context.blocked;
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            let (response, _) = egui::containers::menu::MenuButton::from_button(
                egui::Button::new("")
                    .min_size(Vec2::new(46.0, 66.0))
                    .frame_when_inactive(false),
            )
            .ui(ui, |ui| {
                let size = if state.tool == Tool::Pencil {
                    &mut state.options.pencil_size
                } else {
                    &mut state.options.size
                };
                for value in [1, 3, 5, 8] {
                    let response = ui.add(
                        egui::Button::new(format!("{value} px"))
                            .selected(*size == value)
                            .min_size(Vec2::new(180.0, 32.0)),
                    );
                    ui.painter().rect_filled(
                        Rect::from_center_size(
                            response.rect.center() + Vec2::new(25.0, 0.0),
                            Vec2::new(90.0, value as f32),
                        ),
                        0,
                        ui.visuals().text_color(),
                    );
                    if response.clicked() {
                        *size = value;
                        ui.close();
                    }
                }
                let caption = ui.label(tr(ui.ctx(), "Size in pixels"));
                ui.add(egui::DragValue::new(size).range(1..=1024))
                    .labelled_by(caption.id);
            });
            let ink = ui.visuals().text_color();
            for (index, width) in [1.0, 2.0, 4.0, 6.0].into_iter().enumerate() {
                let center = response.rect.center_top() + Vec2::new(0.0, 9.0 + index as f32 * 7.0);
                ui.painter().rect_filled(
                    Rect::from_center_size(center, Vec2::new(32.0, width)),
                    0,
                    ink,
                );
            }
            ui.painter().text(
                response.rect.center_top() + Vec2::new(0.0, 46.0),
                egui::Align2::CENTER_CENTER,
                tr(ui.ctx(), "Size"),
                egui::TextStyle::Body.resolve(ui.style()),
                ink,
            );
            arrow(ui, response.rect.center_bottom() - Vec2::new(0.0, 7.0));
            super::dropdowns::open_state(ui, &response, "Stroke size");
            response
        })
        .inner;
    response.on_hover_text(tr(ui.ctx(), "Stroke size"));
}

/// Tiny filled disclosure triangle, independent of font glyph coverage.
pub(super) fn arrow(ui: &egui::Ui, center: egui::Pos2) {
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            center + Vec2::new(-2.5, -1.5),
            center + Vec2::new(2.5, -1.5),
            center + Vec2::new(0.0, 1.5),
        ],
        ui.visuals().text_color(),
        egui::Stroke::NONE,
    ));
}
