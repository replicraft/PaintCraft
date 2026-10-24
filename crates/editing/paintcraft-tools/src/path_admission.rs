//! Fallible path growth with caller-owned logical point/storage allowances.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
impl ToolState {
    pub fn gesture_point_capacity(&self) -> usize {
        self.gesture
            .as_ref()
            .map_or(0, |gesture| gesture.points.capacity())
    }
    pub fn try_pointer_move(
        &mut self,
        at: (i64, i64),
        points: usize,
        capacity: usize,
    ) -> Result<()> {
        let at = if self
            .gesture
            .as_ref()
            .is_some_and(|gesture| gesture.point_scale == 256)
        {
            let scale = |value: i64| {
                value
                    .checked_mul(256)
                    .and_then(|value| value.checked_add(128))
                    .ok_or_else(|| {
                        Error::new(
                            ErrorCode::InputSchema,
                            "pointer coordinate exceeds the fixed-point representation",
                        )
                    })
            };
            (scale(at.0)?, scale(at.1)?)
        } else {
            at
        };
        self.append_admitted(at, points, capacity)
    }
    pub fn try_pointer_move_subpixel(
        &mut self,
        at: (i64, i64),
        points: usize,
        capacity: usize,
    ) -> Result<()> {
        let at = if self
            .gesture
            .as_ref()
            .is_some_and(|gesture| gesture.point_scale == 256)
        {
            at
        } else {
            (at.0.div_euclid(256), at.1.div_euclid(256))
        };
        self.append_admitted(at, points, capacity)
    }
    fn append_admitted(&mut self, at: (i64, i64), points: usize, capacity: usize) -> Result<()> {
        if self.gesture_is_lasso() {
            return self.append_lasso_admitted(at, points, capacity);
        }
        if self.construction.is_some() {
            self.append_point(at);
            return Ok(());
        }
        let Some(gesture) = &mut self.gesture else {
            return Ok(());
        };
        if !matches!(gesture.tool, Tool::Pencil | Tool::Brush | Tool::Eraser) {
            self.append_point(at);
            return Ok(());
        }
        if gesture.points.last() == Some(&at) {
            return Ok(());
        }
        let limit = || {
            Error::new(
                ErrorCode::ResourceLimit,
                "stroke point/storage allowance reached; release to commit the captured path",
            )
        };
        if gesture.points.len() >= points.min(POINTS_PER_OP * OPS_PER_BATCH) {
            return Err(limit());
        }
        let needed = gesture.points.len().checked_add(1).ok_or_else(limit)?;
        if gesture.points.capacity() < needed {
            let wanted = gesture
                .points
                .capacity()
                .saturating_mul(2)
                .max(4)
                .min(capacity);
            if wanted < needed {
                return Err(limit());
            }
            let mut next = Vec::new();
            next.try_reserve_exact(wanted).map_err(|_| limit())?;
            if next.capacity() > capacity {
                return Err(limit());
            }
            next.extend_from_slice(&gesture.points);
            gesture.points = next;
        }
        self.append_point(at);
        Ok(())
    }
}
