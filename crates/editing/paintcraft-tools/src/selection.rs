//! Lasso geometry is selection input, separate from queued painting paths.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result, policy::COORD_ABS_MAX};

pub(super) fn valid_point(point: (i64, i64)) -> bool {
    [point.0, point.1]
        .iter()
        .all(|value| value.unsigned_abs() <= COORD_ABS_MAX as u64)
}

pub(super) fn has_three_vertices(points: &[(i64, i64)]) -> bool {
    let Some(first) = points.first() else {
        return false;
    };
    let Some(second) = points.iter().find(|point| *point != first) else {
        return false;
    };
    points.iter().any(|point| point != first && point != second)
}

impl ToolState {
    /// Both rectangle and lasso motions update only the selection overlay.
    pub fn gesture_is_selection(&self) -> bool {
        self.gesture
            .as_ref()
            .is_some_and(|gesture| gesture.tool == Tool::Select)
    }

    /// Unlike `gesture_is_path`, this never grants queued stroke ownership.
    pub fn gesture_is_lasso(&self) -> bool {
        self.gesture.as_ref().is_some_and(|gesture| {
            gesture.tool == Tool::Select
                && gesture.options.selection_mode == SelectionMode::FreeForm
        })
    }

    pub fn gesture_is_selection_brush(&self) -> bool {
        self.gesture.as_ref().is_some_and(|g| {
            g.tool == Tool::Select && g.options.selection_mode == SelectionMode::Brush
        })
    }

    /// Both retain every sample, but only a lasso closes a polygon.
    pub fn gesture_is_selection_path(&self) -> bool {
        self.gesture_is_lasso() || self.gesture_is_selection_brush()
    }

    /// Admit the final integer document position before closing a gesture. A
    /// mismatched release button cannot append points or finish the active drag.
    pub fn try_pointer_up_at(
        &mut self,
        at: (i64, i64),
        button: PointerButton,
        points: usize,
        capacity: usize,
    ) -> Result<Option<(u64, Action)>> {
        if self.gesture_button() != Some(button) {
            return Ok(None);
        }
        self.try_pointer_move(at, points, capacity)?;
        Ok(self.pointer_up_button(button))
    }

    pub(super) fn append_selection_path_admitted(
        &mut self,
        at: (i64, i64),
        points: usize,
        capacity: usize,
    ) -> Result<()> {
        let result = self.grow_selection_path(at, points, capacity);
        if result.is_err() {
            // A truncated selection path changes coverage. Never publish the retained prefix.
            self.cancel_gesture();
        }
        result
    }

    fn grow_selection_path(
        &mut self,
        at: (i64, i64),
        points: usize,
        capacity: usize,
    ) -> Result<()> {
        if !valid_point(at) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "selection point exceeds coordinate bounds",
            ));
        }
        let Some(gesture) = &mut self.gesture else {
            return Ok(());
        };
        if gesture.points.last() == Some(&at) {
            return Ok(());
        }
        let limit = || {
            Error::new(
                ErrorCode::ResourceLimit,
                "Selection path is too complex; try a shorter path",
            )
        };
        let needed = gesture.points.len().checked_add(1).ok_or_else(limit)?;
        let maximum = if gesture.options.selection_mode == SelectionMode::Brush {
            16384
        } else {
            POINTS_PER_OP
        };
        if needed > points.min(maximum) || needed > capacity.min(maximum) {
            return Err(limit());
        }
        if gesture.points.capacity() < needed {
            let wanted = gesture
                .points
                .capacity()
                .saturating_mul(2)
                .max(4)
                .min(capacity)
                .min(maximum);
            if wanted < needed {
                return Err(limit());
            }
            let mut next = Vec::new();
            next.try_reserve_exact(wanted).map_err(|_| limit())?;
            if next.capacity() > capacity.min(maximum) {
                return Err(limit());
            }
            next.extend_from_slice(&gesture.points);
            gesture.points = next;
        }
        gesture.points.push(at);
        self.preview_revision.advance();
        Ok(())
    }
}

impl ToolState {
    /// Immutable captured recipe for an asynchronous selection-only preview.
    pub fn selection_recipe(
        &self,
    ) -> Option<(
        uuid::Uuid,
        Vec<(i64, i64)>,
        paintcraft_core::types::selection::Settings,
    )> {
        let g = self.gesture.as_ref()?;
        if g.tool != Tool::Select || self.floating_selection().is_some() {
            return None;
        }
        let shape = g.options.selection_mode;
        if matches!(shape, SelectionMode::FreeForm | SelectionMode::Polygon)
            && !has_three_vertices(&g.points)
        {
            return None;
        }
        let mut settings = g.options.selection;
        settings.shape = shape;
        Some((g.layer, g.points.clone(), settings))
    }
}
