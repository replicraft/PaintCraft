//! Shared tool identities, options, editing intentions and preview contracts.
use super::{BlendMode, PointerButton, ShapePreset};
use paintcraft_core::types::{Rgba, text::TextOptions};
use uuid::Uuid;
pub const FINISH_SHAPE_BEFORE_FILE: &str = "Finish the shape with Enter or cancel it with Escape before opening, saving or exporting a file";

/// How a shape is painted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeStyle {
    Outline,
    Filled,
    /// Outline in the primary colour over a fill in the secondary colour.
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Pencil,
    Brush,
    Eraser,
    Fill,
    ColorPicker,
    /// View-only tool. Hosts handle clicks; it never produces document edits.
    Magnifier,
    /// View-only navigation; never produces document commands.
    Pan,
    Line,
    Rect,
    Ellipse,
    Preset(ShapePreset),
    Polygon,
    Curve,
    Select,
    Text,
}

impl Tool {
    /// Stable action identity; preserve the original nine tool IDs.
    pub fn action_id(self) -> u64 {
        match self {
            Self::Pencil => 0,
            Self::Brush => 1,
            Self::Eraser => 2,
            Self::Fill => 3,
            Self::Line => 4,
            Self::Rect => 5,
            Self::Ellipse => 6,
            Self::Select => 7,
            Self::Text => 8,
            Self::Preset(preset) => 9 + preset as u64,
            Self::Polygon => 27,
            Self::Curve => 28,
            Self::ColorPicker => 29,
            Self::Magnifier => 30,
            Self::Pan => 31,
        }
    }
    /// Native Studio color gestures. Other secondary-button tool policies remain
    /// unavailable until their distinct selection/text/eraser semantics exist.
    pub fn supports_button(self, button: PointerButton) -> bool {
        button == PointerButton::Primary
            || matches!(
                self,
                Self::ColorPicker
                    | Self::Magnifier
                    | Self::Pencil
                    | Self::Brush
                    | Self::Fill
                    | Self::Line
                    | Self::Rect
                    | Self::Ellipse
                    | Self::Preset(_)
                    | Self::Polygon
                    | Self::Curve
            )
    }

    pub const ALL: [Tool; 14] = [
        Tool::Pencil,
        Tool::Brush,
        Tool::Eraser,
        Tool::Fill,
        Tool::Line,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Select,
        Tool::Text,
        Tool::Polygon,
        Tool::Curve,
        Tool::ColorPicker,
        Tool::Magnifier,
        Tool::Pan,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Pencil => "Pencil",
            Tool::Brush => "Brush",
            Tool::Eraser => "Eraser",
            Tool::Fill => "Fill",
            Tool::ColorPicker => "Color picker",
            Tool::Magnifier => "Magnifier",
            Tool::Pan => "Pan tool",
            Tool::Line => "Line",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Preset(preset) => preset.name(),
            Tool::Polygon => "Polygon",
            Tool::Curve => "Curve",
            Tool::Select => "Select",
            Tool::Text => "Text",
        }
    }
}

/// Captured selection gesture; changing preferences never changes an active drag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionMode {
    #[default]
    Rectangle,
    FreeForm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolOptions {
    pub selection_mode: SelectionMode,
    /// Ignore the captured secondary color while placing a moved selection.
    pub transparent_selection: bool,
    /// Pencil thickness in pixels, captured at pointer-down (1..=1024).
    pub pencil_size: u32,
    /// Brush, eraser and outline thickness in pixels (1..=1024).
    pub size: u32,
    pub round: bool,
    pub media: paintcraft_core::types::brush::BrushMedia,
    /// Replacement pixel for erasing, captured at pointer-down. Transparent for
    /// layered editors; an opaque canvas color for paint-style frontends.
    pub eraser_color: Rgba,
    /// Capped Brush opacity in percent, 0..=100; captured at pointer-down.
    pub brush_opacity: u8,
    pub style: ShapeStyle,
    /// Fill tolerance, 0..=255 per channel.
    pub tolerance: u8,
    /// Text size as a whole-number multiple of the 8-pixel font (1..=16).
    pub text_scale: u32,
    pub picker_source: paintcraft_core::types::picker::SampleSource,
    pub picker_size: paintcraft_core::types::picker::SampleSize,
    pub picker_after: paintcraft_core::types::picker::PickerAfter,
}

impl Default for ToolOptions {
    fn default() -> Self {
        ToolOptions {
            selection_mode: SelectionMode::Rectangle,
            transparent_selection: false,
            pencil_size: 1,
            size: 4,
            round: true,
            media: Default::default(),
            eraser_color: Rgba::TRANSPARENT,
            brush_opacity: 100,
            style: ShapeStyle::Outline,
            tolerance: 0,
            text_scale: 2,
            picker_source: paintcraft_core::types::picker::SampleSource::Layer,
            picker_size: paintcraft_core::types::picker::SampleSize::One,
            picker_after: paintcraft_core::types::picker::PickerAfter::Stay,
        }
    }
}

/// Captured stroke geometry and explicit paint mode, shared by preview and release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stroke {
    pub layer: Uuid,
    pub points: Vec<(i64, i64)>,
    pub point_scale: u32,
    pub color: Rgba,
    pub size: u32,
    pub round: bool,
    pub media: paintcraft_core::types::brush::BrushMedia,
    pub mode: BlendMode,
}

/// A closed polygon or a flattened open curve, captured independently of UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contour {
    pub layer: Uuid,
    pub points: Vec<(i64, i64)>,
    pub closed: bool,
    pub stroke: Option<(Rgba, u32)>,
    pub fill: Option<Rgba>,
}

/// Captured sampling intention: no document mutation or recorded history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorPick {
    pub original: Rgba,
    pub revision: super::PreviewRevision,
    pub layer: Uuid,
    pub at: (i64, i64),
    pub button: PointerButton,
    pub source: paintcraft_core::types::picker::SampleSource,
    pub size: paintcraft_core::types::picker::SampleSize,
    pub return_to: Option<Tool>,
    pub return_resource_shape: Option<crate::resource_shape::Reference>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    ToolFailure(&'static str),
    SampledStroke(crate::sampled_brush::SampledStroke),
    PenStroke(crate::pen_stroke::PenStroke),
    ResourceShape(crate::resource_shape::Placement),
    MoveSelection {
        layer: Uuid,
        offset: (i64, i64),
        clear: Rgba,
        transparent_key: Option<Rgba>,
    },
    SelectPolygon {
        points: Vec<(i64, i64)>,
    },
    InvertSelection,
    PickColor(ColorPick),
    CropToSelection,
    EraseSelection {
        color: Rgba,
    },
    TransformImage {
        transform: paintcraft_core::types::orthogonal::OrthogonalTransform,
        clear: Rgba,
    },
    Contour(Contour),
    Stroke(Stroke),
    Fill {
        layer: Uuid,
        at: (i64, i64),
        color: Rgba,
        tolerance: u8,
    },
    Shape {
        layer: Uuid,
        tool: Tool,
        from: (i64, i64),
        to: (i64, i64),
        stroke: Option<(Rgba, u32)>,
        fill: Option<Rgba>,
    },
    Text {
        style: Option<TextOptions>,
        background: Rgba,
        layer: Uuid,
        at: (i64, i64),
        text: String,
        scale: u32,
        color: Rgba,
    },
    /// Rectangle in document pixels (inclusive corners), or `None` to deselect.
    Select {
        corners: Option<((i64, i64), (i64, i64))>,
    },
    /// Replace all layers with one blank background, independent of selection.
    ClearCanvas {
        color: Rgba,
    },
    /// Preserve layer-local pixels/bounds and position them with a canvas anchor.
    ResizeCanvasAnchored {
        width: u32,
        height: u32,
        anchor: paintcraft_core::types::canvas::Anchor,
    },
    ResizeCanvas {
        width: u32,
        height: u32,
    },
    /// Replace the visible composition with one layer; retain recorded Undo.
    FlattenImage,
    AddLayer,
    DuplicateLayer {
        layer: Uuid,
    },
    DeleteLayer {
        layer: Uuid,
    },
    /// Move within the stack; index 0 is the bottom layer.
    MoveLayer {
        layer: Uuid,
        to_index: usize,
    },
    SetLayerProperties {
        layer: Uuid,
        name: String,
        visible: bool,
        opacity: u8,
    },
    Rename {
        layer: Uuid,
        name: String,
    },
    SetOpacity {
        layer: Uuid,
        opacity: u8,
    },
    SetVisible {
        layer: Uuid,
        visible: bool,
    },
    Undo,
    Redo,
    NavigateHistory {
        owner: Uuid,
        state_id: Uuid,
        epoch: u64,
        history_id: Option<Uuid>,
    },
    Save,
}

/// What the UI should draw for an unpublished gesture.
#[derive(Clone, Debug, PartialEq)]
pub enum Preview {
    SampledStroke(crate::sampled_brush::SampledStroke),
    PenStroke(crate::pen_stroke::PenStroke),
    ResourceShape(crate::resource_shape::Placement),
    MoveSelection {
        layer: Uuid,
        offset: (i64, i64),
        clear: Rgba,
        transparent_key: Option<Rgba>,
    },
    /// Overlay only: polygon closure does not mutate the raster or selection.
    SelectionPath {
        points: Vec<(i64, i64)>,
    },
    Contour(Contour),
    Path(Stroke),
    /// Ordered unpublished strokes against one committed base.
    Paths(Vec<Stroke>),
    Shape {
        layer: Uuid,
        tool: Tool,
        from: (i64, i64),
        to: (i64, i64),
        stroke: Option<(Rgba, u32)>,
        fill: Option<Rgba>,
    },
    Marquee {
        from: (i64, i64),
        to: (i64, i64),
    },
    Text {
        layer: Uuid,
        style: Option<TextOptions>,
        background: Rgba,
        at: (i64, i64),
        text: String,
        scale: u32,
        color: Rgba,
    },
}
