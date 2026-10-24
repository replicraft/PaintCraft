//! Tool state machines produce intentions; the application publishes them.
pub mod commands;
mod geometry;
mod path_admission;
mod preview_revision;
#[cfg(feature = "text")]
pub mod text;
pub use preview_revision::PreviewRevision;

use paintcraft_core::types::{Rgba, text::TextOptions};
pub use paintcraft_core::types::{policy::BlendMode, shapes::ShapePreset};
pub use paintcraft_input::PointerButton;
use uuid::Uuid;
const POINTS_PER_OP: usize = 65_536;
const OPS_PER_BATCH: usize = 1024;
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
    Line,
    Rect,
    Ellipse,
    Preset(ShapePreset),
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
        }
    }
    /// Native Studio color gestures. Other secondary-button tool policies remain
    /// unavailable until their distinct selection/text/eraser semantics exist.
    pub fn supports_button(self, button: PointerButton) -> bool {
        button == PointerButton::Primary
            || matches!(
                self,
                Self::Pencil
                    | Self::Brush
                    | Self::Fill
                    | Self::Line
                    | Self::Rect
                    | Self::Ellipse
                    | Self::Preset(_)
            )
    }

    pub const ALL: [Tool; 9] = [
        Tool::Pencil,
        Tool::Brush,
        Tool::Eraser,
        Tool::Fill,
        Tool::Line,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Select,
        Tool::Text,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Pencil => "Pencil",
            Tool::Brush => "Brush",
            Tool::Eraser => "Eraser",
            Tool::Fill => "Fill",
            Tool::Line => "Line",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Preset(preset) => preset.name(),
            Tool::Select => "Select",
            Tool::Text => "Text",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolOptions {
    /// Pencil thickness in pixels, captured at pointer-down (1..=1024).
    pub pencil_size: u32,
    /// Brush, eraser and outline thickness in pixels (1..=1024).
    pub size: u32,
    pub round: bool,
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
}

impl Default for ToolOptions {
    fn default() -> Self {
        ToolOptions {
            pencil_size: 1,
            size: 4,
            round: true,
            eraser_color: Rgba::TRANSPARENT,
            brush_opacity: 100,
            style: ShapeStyle::Outline,
            tolerance: 0,
            text_scale: 2,
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
    pub mode: BlendMode,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
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
    ResizeCanvas {
        width: u32,
        height: u32,
    },
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

/// Text being typed; it becomes one transaction when committed.
#[derive(Debug)]
struct TextEdit {
    style: Option<TextOptions>,
    background: Rgba,
    token: u64,
    layer: Uuid,
    at: (i64, i64),
    text: String,
    scale: u32,
    color: Rgba,
}

#[derive(Debug)]
struct Gesture {
    button: PointerButton,
    token: u64,
    layer: Uuid,
    tool: Tool,
    color: Rgba,
    fill: Rgba,
    options: ToolOptions,
    points: Vec<(i64, i64)>,
    point_scale: u32,
    constrain_shape: bool,
}

#[derive(Debug)]
pub struct ToolState {
    pub text_style: Option<TextOptions>,
    pub color: Rgba,
    pub secondary: Rgba,
    pub tool: Tool,
    pub options: ToolOptions,
    gesture: Option<Gesture>,
    text: Option<TextEdit>,
    next_gesture: u64,
    preview_revision: PreviewRevision,
}
impl Default for ToolState {
    fn default() -> Self {
        Self {
            text_style: None,
            color: Rgba([0, 0, 0, 255]),
            secondary: Rgba([255; 4]),
            tool: Tool::Pencil,
            options: ToolOptions::default(),
            gesture: None,
            text: None,
            next_gesture: 1 << 63,
            preview_revision: PreviewRevision::default(),
        }
    }
}
impl ToolState {
    pub fn preview_revision(&self) -> PreviewRevision {
        self.preview_revision
    }
    pub fn set_text_style(&mut self, style: TextOptions) {
        if self.text_style.as_ref() != Some(&style) {
            self.text_style = Some(style.clone());
        }
        if let Some(text) = &mut self.text
            && (text.style.as_ref() != Some(&style)
                || text.color != self.color
                || text.background != self.secondary)
        {
            text.style = Some(style);
            text.color = self.color;
            text.background = self.secondary;
            self.preview_revision.advance();
        }
    }
    pub fn begin_text(&mut self, layer: Uuid, at: (i64, i64)) {
        self.next_gesture += 1;
        self.text = Some(TextEdit {
            style: self.text_style.clone(),
            background: self.secondary,
            token: self.next_gesture,
            layer,
            at,
            text: String::new(),
            scale: self
                .options
                .text_scale
                .clamp(1, paintcraft_core::types::policy::SCALE_MAX as u32),
            color: self.color,
        });
        self.preview_revision.advance();
    }
    pub fn pointer_down(&mut self, layer: Uuid, at: (i64, i64)) {
        self.pointer_down_button(layer, at, PointerButton::Primary);
    }
    pub fn pointer_down_button(&mut self, layer: Uuid, at: (i64, i64), button: PointerButton) {
        if self.gesture.is_some() || !self.tool.supports_button(button) {
            return;
        }
        self.next_gesture += 1;
        let (color, fill) = if button == PointerButton::Primary {
            (self.color, self.secondary)
        } else {
            (self.secondary, self.color)
        };
        self.gesture = Some(Gesture {
            button,
            token: self.next_gesture,
            layer,
            tool: self.tool,
            color,
            fill,
            options: self.options,
            points: vec![at],
            point_scale: 1,
            constrain_shape: false,
        });
        self.preview_revision.advance();
    }
    /// Round brushes retain fixed-point positions; other tools stay pixel exact.
    pub fn pointer_down_subpixel(&mut self, layer: Uuid, at: (i64, i64), button: PointerButton) {
        if self.gesture.is_some() {
            return;
        }
        self.pointer_down_button(layer, (at.0.div_euclid(256), at.1.div_euclid(256)), button);
        if let Some(g) = &mut self.gesture
            && g.tool == Tool::Brush
            && g.options.round
        {
            g.point_scale = 256;
            g.points[0] = at;
        }
    }

    pub fn pointer_move_subpixel(&mut self, at: (i64, i64)) {
        if self.gesture.as_ref().is_some_and(|g| g.point_scale == 256) {
            self.append_point(at);
        } else {
            self.append_point((at.0.div_euclid(256), at.1.div_euclid(256)));
        }
    }

    pub fn preview_token(&self) -> Option<u64> {
        self.gesture
            .as_ref()
            .map(|g| g.token)
            .or_else(|| self.text.as_ref().map(|t| t.token))
    }

    pub fn gesture_token(&self) -> Option<u64> {
        self.gesture.as_ref().map(|g| g.token)
    }

    pub fn gesture_button(&self) -> Option<PointerButton> {
        self.gesture.as_ref().map(|g| g.button)
    }
    pub fn gesture_is_path(&self) -> bool {
        self.gesture
            .as_ref()
            .is_some_and(|g| matches!(g.tool, Tool::Pencil | Tool::Brush | Tool::Eraser))
    }

    pub fn gesture_in_progress(&self) -> bool {
        self.gesture.is_some()
    }

    pub fn text_layer(&self) -> Option<Uuid> {
        self.text.as_ref().map(|text| text.layer)
    }

    pub fn is_editing_text(&self) -> bool {
        self.text.is_some()
    }

    pub fn backspace(&mut self) {
        if let Some(t) = &mut self.text
            && t.text.pop().is_some()
        {
            self.preview_revision.advance();
        }
    }

    pub fn cancel_text(&mut self) {
        if self.text.take().is_some() {
            self.preview_revision.advance();
        }
    }

    pub fn preview(&self) -> Option<Preview> {
        if let Some(t) = &self.text {
            return Some(Preview::Text {
                layer: t.layer,
                style: t.style.clone(),
                background: t.background,
                at: t.at,
                text: t.text.clone(),
                scale: t.scale,
                color: t.color,
            });
        }
        let g = self.gesture.as_ref()?;
        let (first, last) = geometry::endpoints(g.tool, &g.points, g.constrain_shape);
        Some(match g.tool {
            Tool::Pencil | Tool::Brush | Tool::Eraser => {
                let (size, round, color, mode) = Self::brush_of(g);
                Preview::Path(Stroke {
                    layer: g.layer,
                    mode,
                    points: g.points.clone(),
                    point_scale: g.point_scale,
                    size,
                    round,
                    color,
                })
            }
            Tool::Fill | Tool::Text => return None,
            Tool::Select => Preview::Marquee {
                from: first,
                to: last,
            },
            Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Preset(_) => {
                let (stroke, fill) = Self::shape_paint(g);
                Preview::Shape {
                    layer: g.layer,
                    tool: g.tool,
                    from: first,
                    to: last,
                    stroke,
                    fill,
                }
            }
        })
    }

    fn brush_of(g: &Gesture) -> (u32, bool, Rgba, BlendMode) {
        match g.tool {
            Tool::Pencil => (
                g.options.pencil_size.clamp(1, 1024),
                false,
                g.color,
                BlendMode::SourceOver,
            ),
            Tool::Eraser => (
                g.options.size,
                false,
                g.options.eraser_color,
                BlendMode::Replace,
            ),
            _ => {
                let mut color = g.color;
                color.0[3] = ((u16::from(color.a()) * u16::from(g.options.brush_opacity.min(100))
                    + 50)
                    / 100) as u8;
                (
                    g.options.size,
                    g.options.round,
                    color,
                    BlendMode::SourceOver,
                )
            }
        }
    }

    fn shape_paint(g: &Gesture) -> (Option<(Rgba, u32)>, Option<Rgba>) {
        let stroke = (g.options.style != ShapeStyle::Filled || g.tool == Tool::Line)
            .then_some((g.color, g.options.size));
        let fill = match (g.tool, g.options.style) {
            (Tool::Line, _) | (_, ShapeStyle::Outline) => None,
            (_, ShapeStyle::Filled) => Some(g.color),
            (_, ShapeStyle::Both) => Some(g.fill),
        };
        (stroke, fill)
    }

    /// Live modifier: retain raw pointer geometry so releasing Shift restores it.
    pub fn constrain_shape(&mut self, enabled: bool) {
        if let Some(g) = &mut self.gesture
            && g.constrain_shape != enabled
        {
            g.constrain_shape = enabled;
            self.preview_revision.advance();
        }
    }

    pub fn gesture_points(&self) -> &[(i64, i64)] {
        self.gesture.as_ref().map_or(&[], |g| &g.points)
    }

    pub fn pointer_move(&mut self, doc_point: (i64, i64)) {
        let point = if self.gesture.as_ref().is_some_and(|g| g.point_scale == 256) {
            (doc_point.0 * 256 + 128, doc_point.1 * 256 + 128)
        } else {
            doc_point
        };
        self.append_point(point);
    }

    fn append_point(&mut self, doc_point: (i64, i64)) {
        let Some(g) = &mut self.gesture else { return };
        match g.tool {
            Tool::Pencil | Tool::Brush | Tool::Eraser => {
                if g.points.last() != Some(&doc_point)
                    && g.points.len() < POINTS_PER_OP * OPS_PER_BATCH
                {
                    g.points.push(doc_point);
                    self.preview_revision.advance();
                }
            }
            // Fill acts where the button went down; shapes and selections only need both ends.
            Tool::Fill | Tool::Text => {}
            _ => {
                if g.points.last() != Some(&doc_point) {
                    g.points.truncate(1);
                    g.points.push(doc_point);
                    self.preview_revision.advance();
                }
            }
        }
    }

    pub fn cancel_gesture(&mut self) {
        if self.gesture.take().is_some() {
            self.preview_revision.advance();
        }
    }
    pub fn pointer_up(&mut self) -> Option<(u64, Action)> {
        self.pointer_up_button(PointerButton::Primary)
    }
    pub fn pointer_up_button(&mut self, button: PointerButton) -> Option<(u64, Action)> {
        if self.gesture_button() != Some(button) {
            return None;
        }
        let g = self.gesture.take()?;
        self.preview_revision.advance();
        let (first, last) = geometry::endpoints(g.tool, &g.points, g.constrain_shape);
        let action = match g.tool {
            Tool::Pencil | Tool::Brush | Tool::Eraser => {
                let (size, round, color, mode) = Self::brush_of(&g);
                Action::Stroke(Stroke {
                    mode,
                    layer: g.layer,
                    points: g.points,
                    point_scale: g.point_scale,
                    color,
                    size,
                    round,
                })
            }
            Tool::Text => return None,
            Tool::Fill => Action::Fill {
                layer: g.layer,
                at: first,
                color: g.color,
                tolerance: g.options.tolerance,
            },
            Tool::Select => Action::Select {
                corners: Some((first, last)),
            },
            Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Preset(_) => {
                let (stroke, fill) = Self::shape_paint(&g);
                Action::Shape {
                    layer: g.layer,
                    tool: g.tool,
                    from: first,
                    to: last,
                    stroke,
                    fill,
                }
            }
        };
        Some((g.token, action))
    }
    pub fn clear_text(&mut self) {
        if let Some(text) = &mut self.text
            && !text.text.is_empty()
        {
            text.text.clear();
            self.preview_revision.advance();
        }
    }
    /// Borrow the active draft without cloning its text or resolving fonts.
    pub fn text_content(&self) -> Option<&str> {
        self.text.as_ref().map(|text| text.text.as_str())
    }
    /// Freeze the text intention while retaining the editable draft. A host can
    /// prepare/validate/publicize it before dropping the draft on success.
    pub fn pending_text_action(&self) -> Option<Action> {
        let t = self.text.as_ref()?;
        if t.text.is_empty() {
            return None;
        }
        Some(Action::Text {
            style: t.style.clone(),
            background: t.background,
            layer: t.layer,
            at: t.at,
            text: t.text.clone(),
            scale: t.scale,
            color: t.color,
        })
    }
    pub fn commit_text(&mut self) -> Option<(u64, Action)> {
        if let Some(t) = self.text.take() {
            self.preview_revision.advance();
            if t.text.is_empty() {
                return None;
            }
            return Some((
                t.token,
                Action::Text {
                    style: t.style,
                    background: t.background,
                    layer: t.layer,
                    at: t.at,
                    text: t.text,
                    scale: t.scale,
                    color: t.color,
                },
            ));
        }
        None
    }
    pub fn type_text(&mut self, typed: &str) -> usize {
        let Some(t) = &mut self.text else { return 0 };
        let mut skipped = 0;
        let previous_len = t.text.len();
        let mut count = t.text.chars().count();
        for c in typed.chars() {
            if !(if t.style.is_some() {
                c == '\n' || !c.is_control()
            } else {
                paintcraft_raster::font::supported(c)
            }) {
                skipped += 1;
            } else if count < paintcraft_raster::font::TEXT_CHARS_MAX {
                t.text.push(c);
                count += 1;
            }
        }
        if t.text.len() != previous_len {
            self.preview_revision.advance();
        }
        skipped
    }
}
