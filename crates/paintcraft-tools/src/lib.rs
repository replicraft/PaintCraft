//! Tool state machines produce intentions; the application publishes them.
use paintcraft_core::types::Rgba;
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
    Select,
    Text,
}

impl Tool {
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
            Tool::Select => "Select",
            Tool::Text => "Text",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolOptions {
    /// Brush, eraser and outline thickness in pixels (1..=1024). The pencil is always 1.
    pub size: u32,
    pub round: bool,
    pub style: ShapeStyle,
    /// Fill tolerance, 0..=255 per channel.
    pub tolerance: u8,
    /// Text size as a whole-number multiple of the 8-pixel font (1..=16).
    pub text_scale: u32,
}

impl Default for ToolOptions {
    fn default() -> Self {
        ToolOptions {
            size: 4,
            round: true,
            style: ShapeStyle::Outline,
            tolerance: 0,
            text_scale: 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Stroke {
        layer: Uuid,
        points: Vec<(i64, i64)>,
        color: Rgba,
        size: u32,
        round: bool,
    },
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
    AddLayer,
    DeleteLayer {
        layer: Uuid,
    },
    /// Move within the stack; index 0 is the bottom layer.
    MoveLayer {
        layer: Uuid,
        to_index: usize,
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
    Save,
}

/// What the UI should draw for an unpublished gesture.
#[derive(Clone, Debug, PartialEq)]
pub enum Preview {
    Path {
        points: Vec<(i64, i64)>,
        size: u32,
        round: bool,
        color: Rgba,
    },
    Shape {
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
        at: (i64, i64),
        text: String,
        scale: u32,
        color: Rgba,
    },
}

/// Text being typed; it becomes one transaction when committed.
#[derive(Debug)]
struct TextEdit {
    token: u64,
    layer: Uuid,
    at: (i64, i64),
    text: String,
    scale: u32,
    color: Rgba,
}

#[derive(Debug)]
struct Gesture {
    token: u64,
    layer: Uuid,
    tool: Tool,
    color: Rgba,
    fill: Rgba,
    options: ToolOptions,
    points: Vec<(i64, i64)>,
}

#[derive(Debug)]
pub struct ToolState {
    pub color: Rgba,
    pub secondary: Rgba,
    pub tool: Tool,
    pub options: ToolOptions,
    gesture: Option<Gesture>,
    text: Option<TextEdit>,
    next_gesture: u64,
}
impl Default for ToolState {
    fn default() -> Self {
        Self {
            color: Rgba([0, 0, 0, 255]),
            secondary: Rgba([255; 4]),
            tool: Tool::Pencil,
            options: ToolOptions::default(),
            gesture: None,
            text: None,
            next_gesture: 1 << 63,
        }
    }
}
impl ToolState {
    pub fn begin_text(&mut self, layer: Uuid, at: (i64, i64)) {
        self.next_gesture += 1;
        self.text = Some(TextEdit {
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
    }
    pub fn pointer_down(&mut self, layer: Uuid, at: (i64, i64)) {
        if self.gesture.is_some() {
            return;
        }
        self.next_gesture += 1;
        self.gesture = Some(Gesture {
            token: self.next_gesture,
            layer,
            tool: self.tool,
            color: self.color,
            fill: self.secondary,
            options: self.options,
            points: vec![at],
        });
    }
    pub fn gesture_in_progress(&self) -> bool {
        self.gesture.is_some()
    }

    pub fn is_editing_text(&self) -> bool {
        self.text.is_some()
    }

    pub fn backspace(&mut self) {
        if let Some(t) = &mut self.text {
            t.text.pop();
        }
    }

    pub fn cancel_text(&mut self) {
        self.text = None;
    }

    pub fn preview(&self) -> Option<Preview> {
        if let Some(t) = &self.text {
            return Some(Preview::Text {
                at: t.at,
                text: t.text.clone(),
                scale: t.scale,
                color: t.color,
            });
        }
        let g = self.gesture.as_ref()?;
        let (first, last) = (
            g.points[0],
            *g.points.last().expect("a gesture has a point"),
        );
        Some(match g.tool {
            Tool::Pencil | Tool::Brush | Tool::Eraser => {
                let (size, round, color) = Self::brush_of(g);
                Preview::Path {
                    points: g.points.clone(),
                    size,
                    round,
                    color,
                }
            }
            Tool::Fill | Tool::Text => return None,
            Tool::Select => Preview::Marquee {
                from: first,
                to: last,
            },
            Tool::Line | Tool::Rect | Tool::Ellipse => {
                let (stroke, fill) = Self::shape_paint(g);
                Preview::Shape {
                    tool: g.tool,
                    from: first,
                    to: last,
                    stroke,
                    fill,
                }
            }
        })
    }

    fn brush_of(g: &Gesture) -> (u32, bool, Rgba) {
        match g.tool {
            Tool::Pencil => (1, false, g.color),
            Tool::Eraser => (g.options.size, false, Rgba::TRANSPARENT),
            _ => (g.options.size, g.options.round, g.color),
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

    pub fn gesture_points(&self) -> &[(i64, i64)] {
        self.gesture.as_ref().map_or(&[], |g| &g.points)
    }

    pub fn pointer_move(&mut self, doc_point: (i64, i64)) {
        let Some(g) = &mut self.gesture else { return };
        match g.tool {
            Tool::Pencil | Tool::Brush | Tool::Eraser => {
                if g.points.last() != Some(&doc_point)
                    && g.points.len() < POINTS_PER_OP * OPS_PER_BATCH
                {
                    g.points.push(doc_point);
                }
            }
            // Fill acts where the button went down; shapes and selections only need both ends.
            Tool::Fill | Tool::Text => {}
            _ => {
                g.points.truncate(1);
                g.points.push(doc_point);
            }
        }
    }

    pub fn cancel_gesture(&mut self) {
        self.gesture = None;
    }
    pub fn pointer_up(&mut self) -> Option<(u64, Action)> {
        let Some(g) = self.gesture.take() else {
            return None;
        };
        let (first, last) = (
            g.points[0],
            *g.points.last().expect("a gesture has a point"),
        );
        let action = match g.tool {
            Tool::Pencil | Tool::Brush | Tool::Eraser => {
                let (size, round, color) = Self::brush_of(&g);
                Action::Stroke {
                    layer: g.layer,
                    points: g.points,
                    color,
                    size,
                    round,
                }
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
            Tool::Line | Tool::Rect | Tool::Ellipse => {
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
    pub fn commit_text(&mut self) -> Option<(u64, Action)> {
        if let Some(t) = self.text.take()
            && !t.text.is_empty()
        {
            return Some((
                t.token,
                Action::Text {
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
        for c in typed.chars() {
            if !paintcraft_raster::font::supported(c) {
                skipped += 1;
            } else if t.text.chars().count() < paintcraft_raster::font::TEXT_CHARS_MAX {
                t.text.push(c);
            }
        }
        skipped
    }
}
