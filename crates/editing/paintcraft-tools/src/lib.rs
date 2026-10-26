//! Tool state machines produce intentions; the application publishes them.
mod image_placement;
pub use image_placement::ImagePlacement;
pub mod commands;
mod construction;
mod contracts;
mod floating;
pub mod sampled_brush;
pub use floating::{FloatingOptions, FloatingSelection};
pub mod navigation;
pub use contracts::*;
mod geometry;
mod path_admission;
mod picker;
mod preview_revision;
mod selection;
#[cfg(feature = "text")]
pub mod text;
pub use preview_revision::PreviewRevision;

use paintcraft_core::types::{Rgba, text::TextOptions};
pub use paintcraft_core::types::{policy::BlendMode, shapes::ShapePreset};
pub use paintcraft_input::PointerButton;
use uuid::Uuid;
const POINTS_PER_OP: usize = 65_536;
const OPS_PER_BATCH: usize = 1024;
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
    return_tool: Tool,
}

#[derive(Debug)]
pub struct ToolState {
    pub text_style: Option<TextOptions>,
    pub color: Rgba,
    pub secondary: Rgba,
    pub tool: Tool,
    pub options: ToolOptions,
    previous_tool: Tool,
    gesture: Option<Gesture>,
    floating: Option<FloatingSelection>,
    construction: Option<construction::Construction>,
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
            previous_tool: Tool::Pencil,
            gesture: None,
            floating: None,
            construction: None,
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
        if self.floating.is_some() {
            self.cancel_gesture();
        }
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
        if self.floating.is_some() && self.tool != Tool::Select {
            self.cancel_gesture();
        }
        if matches!(self.tool, Tool::Magnifier | Tool::Pan) {
            return;
        }
        if let Some(draft) = &mut self.construction {
            if draft.press(layer, self.tool, at, button) {
                self.preview_revision.advance();
            }
            return;
        }
        if self.tool == Tool::Select
            && self.options.selection_mode == SelectionMode::FreeForm
            && !selection::valid_point(at)
        {
            return;
        }
        if self.floating_selection().is_some()
            || self.gesture.is_some()
            || !self.tool.supports_button(button)
        {
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
            return_tool: self.previous_tool,
        });
        if matches!(self.tool, Tool::Polygon | Tool::Curve) {
            self.construction = Some(construction::Construction::new(
                self.gesture.take().unwrap(),
            ));
        }
        self.preview_revision.advance();
    }
    /// Round brushes retain fixed-point positions; other tools stay pixel exact.
    pub fn pointer_down_subpixel(&mut self, layer: Uuid, at: (i64, i64), button: PointerButton) {
        if self.pointer_is_down() {
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
        self.floating_selection()
            .map(|f| f.token())
            .or_else(|| self.gesture.as_ref().map(|g| g.token))
            .or_else(|| self.construction.as_ref().map(|c| c.base.token))
            .or_else(|| self.text.as_ref().map(|t| t.token))
    }

    pub fn gesture_token(&self) -> Option<u64> {
        self.floating_selection()
            .map(|f| f.token())
            .or_else(|| self.gesture.as_ref().map(|g| g.token))
            .or_else(|| self.construction.as_ref().map(|c| c.base.token))
    }

    pub fn gesture_button(&self) -> Option<PointerButton> {
        self.floating_selection()
            .filter(|floating| floating.pointer_is_down())
            .map(|_| PointerButton::Primary)
            .or_else(|| self.gesture.as_ref().map(|g| g.button))
            .or_else(|| {
                self.construction
                    .as_ref()
                    .filter(|c| c.pressed)
                    .map(|c| c.base.button)
            })
    }
    pub fn gesture_is_path(&self) -> bool {
        self.gesture
            .as_ref()
            .is_some_and(|g| matches!(g.tool, Tool::Pencil | Tool::Brush | Tool::Eraser))
    }

    pub fn gesture_in_progress(&self) -> bool {
        self.floating_selection().is_some() || self.gesture.is_some() || self.construction.is_some()
    }
    pub fn pointer_is_down(&self) -> bool {
        self.gesture_button().is_some()
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

    /// Current marquee dimensions, clipped exactly like selection publication.
    /// This reads endpoints directly, without cloning stroke or text buffers.
    pub fn selection_preview_size(
        &self,
        canvas: paintcraft_core::types::Rect,
    ) -> Option<(i64, i64)> {
        let gesture = self
            .gesture
            .as_ref()
            .filter(|gesture| gesture.tool == Tool::Select)?;
        if gesture.options.selection_mode == SelectionMode::FreeForm {
            return None; // A lasso is not a rectangular selection preview.
        }
        let (a, b) = geometry::endpoints(gesture.tool, &gesture.points, false);
        let size = paintcraft_core::types::Rect::new(
            a.0.min(b.0),
            a.1.min(b.1),
            a.0.abs_diff(b.0).saturating_add(1).min(i64::MAX as u64) as i64,
            a.1.abs_diff(b.1).saturating_add(1).min(i64::MAX as u64) as i64,
        )
        .and_then(|rect| rect.intersect(&canvas));
        Some(size.map_or((0, 0), |rect| (rect.width(), rect.height())))
    }

    pub fn preview(&self) -> Option<Preview> {
        if let Some(floating) = self.floating_selection() {
            return floating.preview();
        }
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
        if let Some(draft) = &self.construction {
            return draft.view(false).map(Preview::Contour);
        }
        let g = self.gesture.as_ref()?;
        let (first, last) = geometry::endpoints(g.tool, &g.points, g.constrain_shape);
        Some(match g.tool {
            Tool::Pencil | Tool::Brush | Tool::Eraser => {
                let (size, round, color, mode) = Self::brush_of(g);
                Preview::Path(Stroke {
                    layer: g.layer,
                    media: if g.tool == Tool::Brush {
                        g.options.media
                    } else {
                        Default::default()
                    },
                    mode,
                    points: g.points.clone(),
                    point_scale: g.point_scale,
                    size,
                    round,
                    color,
                })
            }
            Tool::Pan
            | Tool::Magnifier
            | Tool::ColorPicker
            | Tool::Fill
            | Tool::Text
            | Tool::Polygon
            | Tool::Curve => {
                return None;
            }
            Tool::Select if g.options.selection_mode == SelectionMode::FreeForm => {
                Preview::SelectionPath {
                    points: g.points.clone(),
                }
            }
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
        if let Some(draft) = &mut self.construction
            && draft.base.constrain_shape != enabled
        {
            draft.base.constrain_shape = enabled;
            self.preview_revision.advance();
        }
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
        if self.update_floating(doc_point).is_some() {
            return;
        }
        if self.gesture_is_lasso() {
            // Infallible legacy hosts still discard an oversized path completely.
            let _ = self.append_lasso_admitted(doc_point, POINTS_PER_OP, POINTS_PER_OP);
            return;
        }
        if let Some(draft) = &mut self.construction {
            if draft.move_to(doc_point) {
                self.preview_revision.advance();
            }
            return;
        }
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
        let floating = self.floating.take().is_some();
        let gesture = self.gesture.take().is_some();
        let construction = self.construction.take().is_some();
        if floating || gesture || construction {
            self.preview_revision.advance();
        }
    }
    pub fn pointer_up(&mut self) -> Option<(u64, Action)> {
        self.pointer_up_button(PointerButton::Primary)
    }
    pub fn pointer_up_button(&mut self, button: PointerButton) -> Option<(u64, Action)> {
        if let Some(result) = self.release_floating(button) {
            return result;
        }
        if self.construction.is_some() {
            return self.release_shape(button);
        }
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
                    media: if g.tool == Tool::Brush {
                        g.options.media
                    } else {
                        Default::default()
                    },
                    mode,
                    layer: g.layer,
                    points: g.points,
                    point_scale: g.point_scale,
                    color,
                    size,
                    round,
                })
            }
            Tool::Pan | Tool::Magnifier | Tool::Text | Tool::Polygon | Tool::Curve => return None,
            Tool::ColorPicker => Action::PickColor(self.color_pick(&g, last)),
            Tool::Fill => Action::Fill {
                layer: g.layer,
                at: first,
                color: g.color,
                tolerance: g.options.tolerance,
            },
            Tool::Select if g.options.selection_mode == SelectionMode::FreeForm => {
                if !selection::has_three_vertices(&g.points) {
                    return None;
                }
                Action::SelectPolygon { points: g.points }
            }
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
