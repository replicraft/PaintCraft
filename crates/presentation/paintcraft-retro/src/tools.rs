//! Sixteen tool choices share existing engine tool identities and captured options.
use crate::{Intent, Workspace, surface::with_style};
use egui::Vec2;
use paintcraft_core::types::brush::BrushMedia;
use paintcraft_tools::{SelectionMode, ShapePreset, ShapeStyle, Tool};
use paintcraft_ui::{
    actions::{Action, Context, ViewAction},
    chrome::Controls,
    icons::Icon,
    t,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolChoice {
    FreeFormSelect,
    Select,
    Eraser,
    Fill,
    ColorPicker,
    Magnifier,
    Pencil,
    Brush,
    Airbrush,
    Text,
    Line,
    Curve,
    Rectangle,
    Polygon,
    Ellipse,
    RoundedRectangle,
}
impl ToolChoice {
    pub const ALL: [Self; 16] = [
        Self::FreeFormSelect,
        Self::Select,
        Self::Eraser,
        Self::Fill,
        Self::ColorPicker,
        Self::Magnifier,
        Self::Pencil,
        Self::Brush,
        Self::Airbrush,
        Self::Text,
        Self::Line,
        Self::Curve,
        Self::Rectangle,
        Self::Polygon,
        Self::Ellipse,
        Self::RoundedRectangle,
    ];
    pub fn tool(self) -> Tool {
        match self {
            Self::FreeFormSelect | Self::Select => Tool::Select,
            Self::Eraser => Tool::Eraser,
            Self::Fill => Tool::Fill,
            Self::ColorPicker => Tool::ColorPicker,
            Self::Magnifier => Tool::Magnifier,
            Self::Pencil => Tool::Pencil,
            Self::Brush | Self::Airbrush => Tool::Brush,
            Self::Text => Tool::Text,
            Self::Line => Tool::Line,
            Self::Curve => Tool::Curve,
            Self::Rectangle => Tool::Rect,
            Self::Polygon => Tool::Polygon,
            Self::Ellipse => Tool::Ellipse,
            Self::RoundedRectangle => Tool::Preset(ShapePreset::RoundedRectangle),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::FreeFormSelect => "Free-form",
            Self::Select => "Select",
            Self::Airbrush => "Airbrush",
            _ => self.tool().name(),
        }
    }
    /// The host calls this only after its real tool selector has accepted the change.
    pub fn apply_to(self, controls: &mut Controls) {
        controls.tool = self.tool();
        match self {
            Self::FreeFormSelect => controls.options.selection_mode = SelectionMode::FreeForm,
            Self::Select => controls.options.selection_mode = SelectionMode::Rectangle,
            Self::Brush => controls.options.media = BrushMedia::Plain,
            Self::Airbrush => {
                controls.options.media = BrushMedia::Airbrush;
                controls.options.round = true;
            }
            _ => {}
        }
    }
    pub fn matches(self, controls: &Controls) -> bool {
        controls.tool == self.tool()
            && match self {
                Self::FreeFormSelect => controls.options.selection_mode == SelectionMode::FreeForm,
                Self::Select => controls.options.selection_mode == SelectionMode::Rectangle,
                Self::Airbrush => {
                    controls.options.media == BrushMedia::Airbrush && controls.options.round
                }
                Self::Brush => {
                    controls.options.media != BrushMedia::Airbrush || !controls.options.round
                }
                _ => true,
            }
    }
    fn icon(self) -> Icon {
        match self {
            Self::FreeFormSelect => Icon::LassoSelect,
            Self::Select => Icon::RectangleSelect,
            Self::Eraser => Icon::Eraser,
            Self::Fill => Icon::Bucket,
            Self::ColorPicker => Icon::Eyedropper,
            Self::Magnifier => Icon::ZoomIn,
            Self::Pencil => Icon::Pencil,
            Self::Brush => Icon::Brush,
            Self::Airbrush => Icon::Spray,
            Self::Text => Icon::Text,
            Self::Line => Icon::Line,
            Self::Curve => Icon::Curve,
            Self::Rectangle => Icon::Rectangle,
            Self::Polygon => Icon::Polygon,
            Self::Ellipse => Icon::Ellipse,
            Self::RoundedRectangle => Icon::RoundedRectangle,
        }
    }
}

pub fn toolbox(
    ui: &mut egui::Ui,
    controls: &mut Controls,
    workspace: &mut Workspace,
    context: Context,
) -> Vec<Intent> {
    let mut intents = Vec::new();
    if !workspace.toolbox {
        return intents;
    }
    with_style(ui, |ui| {
        ui.add_enabled_ui(context.document_open && !context.blocked, |ui| {
            egui::Grid::new("retro-tools")
                .spacing(Vec2::ZERO)
                .show(ui, |ui| {
                    for (index, choice) in ToolChoice::ALL.into_iter().enumerate() {
                        let selected = choice.matches(controls);
                        let image =
                            paintcraft_ui::icons::legacy_image(ui.ctx(), choice.icon(), 16.0);
                        let response = ui.add(
                            egui::Button::image(image)
                                .selected(selected)
                                .min_size(Vec2::splat(25.0)),
                        );
                        let label = t!(ui.ctx(), choice.label());
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::Role::Button,
                                ui.is_enabled(),
                                selected,
                                &label,
                            )
                        });
                        crate::surface::bevel(ui, response.rect.shrink(0.5), selected);
                        if response.on_hover_text(label).clicked() {
                            intents.push(Intent::ChooseTool(choice));
                        }
                        if index % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
            ui.separator();
            options(ui, controls, &mut intents);
        });
    });
    intents
}

fn options(ui: &mut egui::Ui, controls: &mut Controls, intents: &mut Vec<Intent>) {
    if controls.tool == Tool::Magnifier {
        for (label, value) in [
            ("100%", 10000),
            ("200%", 20000),
            ("400%", 40000),
            ("800%", 80000),
        ] {
            if ui.button(label).clicked() {
                intents.push(Intent::Action(Action::View(ViewAction::SetZoom(value))));
            }
        }
        return;
    }
    if controls.tool == Tool::Select {
        ui.checkbox(
            &mut controls.options.transparent_selection,
            t!(ui.ctx(), "Transparent selection"),
        );
    }
    if matches!(
        controls.tool,
        Tool::Pencil
            | Tool::Brush
            | Tool::Eraser
            | Tool::Line
            | Tool::Curve
            | Tool::Rect
            | Tool::Ellipse
            | Tool::Polygon
            | Tool::Preset(_)
    ) {
        let caption = ui.label(t!(ui.ctx(), "Size"));
        let size = if controls.tool == Tool::Pencil {
            &mut controls.options.pencil_size
        } else {
            &mut controls.options.size
        };
        ui.add(egui::DragValue::new(size).range(1..=1024).suffix(" px"))
            .labelled_by(caption.id);
    }
    if controls.tool == Tool::Brush && controls.options.media != BrushMedia::Airbrush {
        ui.radio_value(&mut controls.options.round, true, t!(ui.ctx(), "Round"));
        ui.radio_value(&mut controls.options.round, false, t!(ui.ctx(), "Square"));
    }
    if controls.tool == Tool::Fill {
        let caption = ui.label(t!(ui.ctx(), "Tolerance"));
        ui.add(egui::DragValue::new(&mut controls.options.tolerance).range(0..=255))
            .labelled_by(caption.id);
    }
    if matches!(
        controls.tool,
        Tool::Rect | Tool::Ellipse | Tool::Polygon | Tool::Preset(_)
    ) {
        for (style, label) in [
            (ShapeStyle::Outline, "Outline"),
            (ShapeStyle::Filled, "Filled"),
            (ShapeStyle::Both, "Both"),
        ] {
            ui.radio_value(&mut controls.options.style, style, t!(ui.ctx(), label));
        }
    }
}
