//! Ordered paint-only speculation against a stable published document layout.
use super::*;
use paintcraft_tools::Stroke;

// Logical activation limits. These do not reserve allocator/process memory.
const MAX_STROKES: usize = 32;
const MAX_POINTS: usize = 262_144;
const MAX_POINT_CAPACITY: usize = MAX_POINTS * 2;
#[derive(Default)]
struct Load {
    strokes: usize,
    points: usize,
    capacity: usize,
}
impl Load {
    fn add(&mut self, stroke: &Stroke) {
        self.strokes += 1;
        self.points = self.points.saturating_add(stroke.points.len());
        self.capacity = self.capacity.saturating_add(stroke.points.capacity());
    }
}
impl Editor {
    fn queued_strokes(&self) -> impl Iterator<Item = &Stroke> {
        self.actions
            .iter()
            .filter_map(|action| match action {
                background::Queued::Edit(Action::Stroke(stroke)) => Some(stroke),
                _ => None,
            })
            .chain(self.outbox.pending().filter_map(|action| match action {
                Action::Stroke(stroke) => Some(stroke),
                _ => None,
            }))
    }
    fn paint_load(&self) -> Load {
        let mut load = Load::default();
        if self.has_inflight_edit()
            && let Some(Preview::Path(stroke)) = &self.publication_preview
        {
            load.add(stroke);
        }
        for stroke in self.queued_strokes() {
            load.add(stroke);
        }
        if self.tools.gesture_is_path() {
            load.strokes += 1;
            load.capacity = load
                .capacity
                .saturating_add(self.tools.gesture_point_capacity());
            load.points = load
                .points
                .saturating_add(self.tools.gesture_points().len());
        }
        load
    }
    pub(super) fn admit_paint(&mut self, stroke: &Stroke) -> bool {
        if self.editing.is_none() {
            return true;
        }
        let mut load = self.paint_load();
        load.add(stroke);
        if load.strokes > MAX_STROKES
            || load.points > MAX_POINTS
            || load.capacity > MAX_POINT_CAPACITY
        {
            self.status = "Painting queue is full; wait for earlier strokes to finish".into();
            false
        } else {
            true
        }
    }
    pub(super) fn move_paint_pointer(&mut self, at: (i64, i64), subpixel: bool) {
        if self.tools.gesture_is_lasso() {
            let result = if subpixel {
                self.tools.try_pointer_move_subpixel(at, 65_536, 65_536)
            } else {
                self.tools.try_pointer_move(at, 65_536, 65_536)
            };
            if let Err(error) = result {
                self.status = error.message;
            }
            return;
        }
        if self.editing.is_none() || !self.tools.gesture_is_path() {
            if subpixel {
                self.tools.pointer_move_subpixel(at);
            } else {
                self.tools.pointer_move(at);
            }
            return;
        }
        let load = self.paint_load();
        let points = MAX_POINTS
            .saturating_sub(load.points)
            .saturating_add(self.tools.gesture_points().len());
        let capacity = MAX_POINT_CAPACITY
            .saturating_sub(load.capacity)
            .saturating_add(self.tools.gesture_point_capacity());
        let result = if subpixel {
            self.tools.try_pointer_move_subpixel(at, points, capacity)
        } else {
            self.tools.try_pointer_move(at, points, capacity)
        };
        if let Err(error) = result {
            self.status = error.message;
        }
    }
    /// Later painting can overlap a paint-only prefix. Any operation that could
    /// change layout, target validity, selection or publication identity blocks it.
    pub(crate) fn painting_blocked(&self) -> bool {
        if self.editing.is_none() {
            return false;
        }
        if self.file_barrier.is_some()
            || (self.has_inflight_edit()
                && !matches!(self.publication_preview, Some(Preview::Path(_))))
            || self
                .actions
                .iter()
                .any(|action| !matches!(action, background::Queued::Edit(Action::Stroke(_))))
            || self
                .outbox
                .pending()
                .any(|action| !matches!(action, Action::Stroke(_)))
        {
            return true;
        }
        if self.has_pending_edits()
            && !matches!(self.tool, Tool::Pencil | Tool::Brush | Tool::Eraser)
        {
            return true;
        }
        let load = self.paint_load();
        // An admitted live gesture must still receive release/cancel at its limit.
        !self.tools.gesture_in_progress()
            && (load.strokes >= MAX_STROKES
                || load.points >= MAX_POINTS
                || load.capacity >= MAX_POINT_CAPACITY)
    }
    /// Compose the in-flight stroke, accepted FIFO prefix and live gesture once
    /// each. Stop at a file/structural boundary; its later commands are not yet
    /// part of the visible speculative document.
    pub(super) fn paint_preview(&self) -> Option<Preview> {
        if self.editing.is_none() || self.file_barrier.is_some() {
            return None;
        }
        let mut paths = Vec::new();
        if self.has_inflight_edit() {
            if let Some(Preview::Path(stroke)) = &self.publication_preview {
                paths.push(stroke.clone());
            } else {
                return None;
            }
        }
        let mut boundary = false;
        for action in &self.actions {
            if let background::Queued::Edit(Action::Stroke(stroke)) = action {
                paths.push(stroke.clone());
            } else {
                boundary = true;
                break;
            }
        }
        if !boundary {
            for action in self.outbox.pending() {
                if let Action::Stroke(stroke) = action {
                    paths.push(stroke.clone());
                } else {
                    boundary = true;
                    break;
                }
            }
        }
        if !boundary && let Some(Preview::Path(stroke)) = self.tools.preview() {
            paths.push(stroke);
        }
        match paths.len() {
            0 => None,
            1 => Some(Preview::Path(paths.pop().expect("one path"))),
            _ => Some(Preview::Paths(paths)),
        }
    }
}

#[cfg(test)]
#[path = "paint_queue/tests.rs"]
mod tests;
