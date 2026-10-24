//! Multi-step polygon and cubic-curve drafts. No document/history publication.
use super::*;
type Point = (i64, i64);
const VERTICES_MAX: usize = 1024;

#[derive(Debug)]
pub(super) struct Construction {
    pub(super) base: Gesture,
    pub(super) pressed: bool,
    cursor: Point,
    closing: bool,
    phase: u8,
    handles: [Point; 2],
}

impl Construction {
    pub(super) fn new(base: Gesture) -> Self {
        let cursor = base.points[0];
        Self {
            base,
            pressed: true,
            cursor,
            closing: false,
            phase: 0,
            handles: [cursor; 2],
        }
    }
    fn endpoint(&self) -> Point {
        let anchor = *self.base.points.last().unwrap();
        geometry::endpoints(
            Tool::Line,
            &[anchor, self.cursor],
            self.base.constrain_shape,
        )
        .1
    }
    pub(super) fn move_to(&mut self, at: Point) -> bool {
        if at == self.cursor {
            return false;
        }
        self.cursor = at;
        if self.pressed && self.base.tool == Tool::Polygon && self.base.points.len() >= 3 {
            self.closing = near(at, self.base.points[0]) || at == *self.base.points.last().unwrap();
        }
        true
    }
    pub(super) fn press(
        &mut self,
        layer: Uuid,
        tool: Tool,
        at: Point,
        button: PointerButton,
    ) -> bool {
        if self.pressed
            || self.base.layer != layer
            || self.base.tool != tool
            || self.base.button != button
        {
            return false;
        }
        self.pressed = true;
        self.cursor = at;
        self.closing = self.base.tool == Tool::Polygon
            && self.base.points.len() >= 3
            && (near(at, self.base.points[0]) || at == *self.base.points.last().unwrap());
        true
    }
    pub(super) fn release(&mut self, button: PointerButton) -> bool {
        if !self.pressed || button != self.base.button {
            return false;
        }
        self.pressed = false;
        if self.base.tool == Tool::Polygon {
            if self.closing {
                return true;
            }
            let end = self.endpoint();
            if self.base.points.last() != Some(&end) {
                self.base.points.push(end);
            }
            self.cursor = end;
            return self.base.points.len() == VERTICES_MAX;
        }
        match self.phase {
            0 => {
                let a = self.base.points[0];
                let b =
                    geometry::endpoints(Tool::Line, &[a, self.cursor], self.base.constrain_shape).1;
                self.base.points.push(b);
                self.handles = [lerp(a, b, 1, 3), lerp(a, b, 2, 3)];
                self.phase = 1;
                self.cursor = self.handles[0];
            }
            1 => {
                self.handles[0] = self.cursor;
                self.phase = 2;
                self.cursor = self.handles[1];
            }
            _ => {
                self.handles[1] = self.cursor;
                return true;
            }
        }
        false
    }
    pub(super) fn remove_vertex(&mut self) -> bool {
        if self.pressed {
            return false;
        }
        if self.base.tool == Tool::Polygon && self.base.points.len() > 1 {
            self.base.points.pop();
            self.cursor = *self.base.points.last().unwrap();
            return true;
        }
        if self.base.tool == Tool::Curve && self.phase > 0 {
            self.phase -= 1;
            if self.phase == 0 {
                self.cursor = self.base.points.pop().unwrap();
            } else {
                self.cursor = self.handles[0];
            }
            return true;
        }
        false
    }
    pub(super) fn view(&self, finalize: bool) -> Option<Contour> {
        let mut points = self.base.points.clone();
        let closed = if self.base.tool == Tool::Polygon {
            if !finalize && !self.closing {
                let end = self.endpoint();
                if points.last() != Some(&end) && points.len() < VERTICES_MAX {
                    points.push(end);
                }
            }
            points.len() >= 3
        } else {
            if self.phase == 0 {
                points.push(
                    geometry::endpoints(
                        Tool::Line,
                        &[points[0], self.cursor],
                        self.base.constrain_shape,
                    )
                    .1,
                );
            } else {
                let mut handles = self.handles;
                if !finalize || self.phase < 2 || !self.pressed {
                    handles[usize::from(self.phase - 1)] = self.cursor;
                }
                points = cubic(points[0], handles[0], handles[1], points[1]);
            }
            false
        };
        points.dedup();
        if points.len() < if closed { 3 } else { 2 } {
            return None;
        }
        let (mut stroke, fill) = ToolState::shape_paint(&self.base);
        if !closed && stroke.is_none() {
            stroke = Some((self.base.color, self.base.options.size));
        }
        Some(Contour {
            layer: self.base.layer,
            points,
            closed,
            stroke,
            fill: if closed { fill } else { None },
        })
    }
    pub(super) fn can_finish(&self) -> bool {
        !self.pressed
            && if self.base.tool == Tool::Polygon {
                self.base.points.len() >= 3
            } else {
                self.phase > 0
            }
    }
    pub(super) fn hint(&self) -> &'static str {
        if self.base.tool == Tool::Polygon {
            "Polygon: click to add vertices; click the start or click the last vertex again to close; Enter finishes, Backspace removes, Escape cancels"
        } else {
            match self.phase {
                0 => "Curve: drag the initial line, then place or drag two control handles",
                1 => {
                    "Curve: place or drag the first control handle; Enter finishes, Escape cancels"
                }
                _ => {
                    "Curve: place or drag the second control handle to finish; Enter finishes, Escape cancels"
                }
            }
        }
    }
}

fn near(a: Point, b: Point) -> bool {
    let x = i128::from(a.0) - i128::from(b.0);
    let y = i128::from(a.1) - i128::from(b.1);
    x.abs() <= 4 && y.abs() <= 4
}
fn lerp(a: Point, b: Point, t: i128, n: i128) -> Point {
    let coordinate = |a, b| ((i128::from(a) * (n - t) + i128::from(b) * t) / n) as i64;
    (coordinate(a.0, b.0), coordinate(a.1, b.1))
}
fn cubic(a: Point, b: Point, c: Point, d: Point) -> Vec<Point> {
    // Fixed-point samples are stable across hosts. At most 257 points; path
    // length/stroke work and coordinate admission still happen in the engine.
    let mut points = Vec::with_capacity(257);
    for t in 0..=256_i128 {
        let u = 256 - t;
        let coordinate = |a, b, c, d| {
            ((u * u * u * i128::from(a)
                + 3 * u * u * t * i128::from(b)
                + 3 * u * t * t * i128::from(c)
                + t * t * t * i128::from(d))
                / (256 * 256 * 256)) as i64
        };
        points.push((
            coordinate(a.0, b.0, c.0, d.0),
            coordinate(a.1, b.1, c.1, d.1),
        ));
    }
    points
}

impl ToolState {
    pub fn is_constructing_shape(&self) -> bool {
        self.construction.is_some()
    }
    pub fn shape_hint(&self) -> Option<&'static str> {
        self.construction.as_ref().map(Construction::hint)
    }
    pub fn remove_shape_vertex(&mut self) {
        if self
            .construction
            .as_mut()
            .is_some_and(Construction::remove_vertex)
        {
            self.preview_revision.advance();
        }
    }
    pub fn finish_shape(&mut self) -> Option<(u64, Action)> {
        let draft = self.construction.as_ref()?;
        if !draft.can_finish() {
            return None;
        }
        let action = Action::Contour(draft.view(true)?);
        let token = draft.base.token;
        self.construction = None;
        self.preview_revision.advance();
        Some((token, action))
    }
    pub(super) fn release_shape(&mut self, button: PointerButton) -> Option<(u64, Action)> {
        let draft = self.construction.as_mut()?;
        if !draft.pressed || button != draft.base.button {
            return None;
        }
        let finished = draft.release(button);
        self.preview_revision.advance();
        if finished { self.finish_shape() } else { None }
    }
}
