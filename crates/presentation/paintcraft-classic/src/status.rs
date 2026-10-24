//! Classic status presentation; the host supplies document facts and applies actions.
use egui::{Align, Layout, Vec2};
use paintcraft_ui::t;
use paintcraft_ui::{
    actions::{Action, Context},
    icons::Icon,
};

mod legacy;

/// Reserve chrome before canvas layout; paint it afterward with current values.
pub fn height(ui: &egui::Ui, document_open: bool) -> f32 {
    if document_open && ui.available_width() < 860.0 {
        60.0 + ui.spacing().item_spacing.y
    } else {
        30.0
    }
}

/// Historical skins retain the slim status strip shown in their references.
pub fn height_for_skin(ui: &egui::Ui, variant: super::ClassicVariant, document_open: bool) -> f32 {
    if variant == super::ClassicVariant::V11 {
        height(ui, document_open)
    } else if document_open && ui.available_width() < 860.0 {
        44.0
    } else {
        22.0
    }
}

pub fn contents_for_skin(
    ui: &mut egui::Ui,
    variant: super::ClassicVariant,
    status: Status<'_>,
    context: Context,
    id: egui::Id,
) -> Vec<Action> {
    if variant == super::ClassicVariant::V11 {
        contents(ui, status, context, id)
    } else {
        legacy::contents(ui, variant, status, context, id)
    }
}

#[derive(Debug)]
pub struct Status<'a> {
    pub message: &'a str,
    pub recovery: Option<&'a str>,
    pub canvas: Option<(u32, u32)>,
    pub pointer: Option<(i64, i64)>,
    pub selection: Option<(i64, i64)>,
}

pub fn contents(
    ui: &mut egui::Ui,
    status: Status<'_>,
    context: Context,
    id: egui::Id,
) -> Vec<Action> {
    ui.style_mut()
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
    let width = ui.available_width();
    let mut actions = Vec::new();
    if status.canvas.is_none() {
        message(ui, &status, width, 30.0);
        return actions;
    }
    let narrow = width < 860.0;
    let metrics_width = if narrow { width * 0.65 } else { 416.0 };
    let gap = ui.spacing().item_spacing.x;
    let message_width = (width
        - metrics_width
        - if narrow {
            gap
        } else {
            super::zoom::width(ui) + gap * 2.0
        })
    .max(0.0);
    ui.allocate_ui_with_layout(
        Vec2::new(width, 30.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_size(Vec2::new(width, 30.0));
            metrics(ui, &status, metrics_width, 30.0, context.selection_active);
            message(ui, &status, message_width, 30.0);
            if !narrow {
                actions.extend(super::zoom_controls(ui, context, id));
            }
        },
    );
    if narrow {
        ui.allocate_ui_with_layout(
            Vec2::new(width, 30.0),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.set_min_size(Vec2::new(width, 30.0));
                actions.extend(super::zoom_controls(ui, context, id));
            },
        );
    }
    actions
}

fn metrics(
    ui: &mut egui::Ui,
    status: &Status<'_>,
    width: f32,
    row_height: f32,
    selection_active: bool,
) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, row_height),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_size(Vec2::new(width, row_height));
            ui.spacing_mut().item_spacing.x = 0.0;
            let pointer = status.pointer.map(|(x, y)| format!("{x}, {y} px"));
            let selection = status.selection.map(|(w, h)| format!("{w} × {h} px"));
            let canvas = status.canvas.map(|(w, h)| format!("{w} × {h} px"));
            for (index, (label, value, empty, icon, fraction)) in [
                (
                    "Cursor position",
                    pointer,
                    "Outside canvas",
                    Icon::Pointer,
                    0.28,
                ),
                (
                    "Selection size",
                    selection,
                    if selection_active {
                        "Selection bounds unavailable"
                    } else {
                        "No selection"
                    },
                    Icon::RectangleSelect,
                    0.28,
                ),
                (
                    "Canvas size",
                    canvas,
                    "No document open",
                    Icon::CanvasSize,
                    0.44,
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let field = ui.allocate_ui_with_layout(
                    Vec2::new(width * fraction, row_height),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.set_min_size(Vec2::new(width * fraction, row_height));
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.add_space(4.0);
                        ui.add(paintcraft_ui::icons::classic_status_image(
                            ui.ctx(),
                            icon,
                            16.0,
                            ui.visuals().dark_mode,
                        ));
                        let text = value.as_deref().unwrap_or("");
                        ui.add(
                            egui::Label::new(text)
                                .truncate()
                                .show_tooltip_when_elided(false),
                        );
                    },
                );
                if index < 2 {
                    let rect = field.response.rect;
                    ui.painter().line_segment(
                        [
                            rect.right_top() + Vec2::new(0.0, 4.0),
                            rect.right_bottom() - Vec2::new(0.0, 4.0),
                        ],
                        ui.visuals().widgets.noninteractive.bg_stroke,
                    );
                }
                let tip = format!(
                    "{}: {}",
                    t!(ui.ctx(), label),
                    value.clone().unwrap_or_else(|| t!(ui.ctx(), empty))
                );
                field.response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Label, ui.is_enabled(), &tip)
                });
                field.response.on_hover_text(tip);
            }
        },
    );
}

fn message(ui: &mut egui::Ui, status: &Status<'_>, width: f32, row_height: f32) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, row_height),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_size(Vec2::new(width, row_height));
            let mut tip = status.message.to_owned();
            if let Some(recovery) = status.recovery {
                tip.push('\n');
                tip.push_str(recovery);
            }
            ui.add(
                egui::Label::new(status.message)
                    .truncate()
                    .show_tooltip_when_elided(false),
            )
            .on_hover_text(tip);
        },
    );
}
