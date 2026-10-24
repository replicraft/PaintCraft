//! A bounded floating-selection drag. Hosts retain and verify the source session.
use crate::{Action, PointerButton, Preview, PreviewRevision};
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba, policy::COORD_ABS_MAX};
use paintcraft_raster::Selection;
use uuid::Uuid;

/// Captured once at pointer-down. Bounds are derived feedback, coverage is authority.
#[derive(Clone, Debug, PartialEq)]
pub struct FloatingOptions {
    pub layer: Uuid,
    pub selection: Selection,
    pub bounds: Rect,
    pub clear: Rgba,
    pub transparent_key: Option<Rgba>,
    pub token: u64,
    pub commit_on_release: bool,
}

#[derive(Debug)]
pub struct FloatingSelection {
    options: FloatingOptions,
    start: (i64, i64),
    offset: (i64, i64),
    start_offset: (i64, i64),
    held: bool,
    revision: PreviewRevision,
    active: bool,
}
impl FloatingSelection {
    /// Holes start a new selection instead of grabbing a bounding rectangle.
    pub fn hit_test(selection: &Selection, at: (i64, i64)) -> bool {
        if !crate::selection::valid_point(at) {
            return false;
        }
        match selection {
            Selection::Rect(rect) => rect.contains(at.0, at.1),
            Selection::Mask(mask) => mask.coverage(at.0, at.1) != 0,
            Selection::Inactive | Selection::Empty => false,
        }
    }
    pub fn begin(
        options: FloatingOptions,
        at: (i64, i64),
        button: PointerButton,
    ) -> Result<Option<Self>> {
        if button != PointerButton::Primary || !Self::hit_test(&options.selection, at) {
            return Ok(None);
        }
        let b = options.bounds;
        if !crate::selection::valid_point((b.x0, b.y0))
            || !crate::selection::valid_point((b.x1, b.y1))
            || b.x0 >= b.x1
            || b.y0 >= b.y1
            || !b.contains(at.0, at.1)
        {
            return Err(invalid());
        }
        Ok(Some(Self {
            options,
            start: at,
            offset: (0, 0),
            start_offset: (0, 0),
            held: true,
            revision: PreviewRevision::default(),
            active: true,
        }))
    }
    pub fn begin_subpixel(
        options: FloatingOptions,
        at: (i64, i64),
        button: PointerButton,
    ) -> Result<Option<Self>> {
        Self::begin(options, pixels(at), button)
    }
    pub fn token(&self) -> u64 {
        self.options.token
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
    pub fn pointer_is_down(&self) -> bool {
        self.active && self.held
    }
    pub fn regrab(&mut self, at: (i64, i64), button: PointerButton) -> bool {
        if !self.active
            || self.held
            || button != PointerButton::Primary
            || !crate::selection::valid_point(at)
            || !self.bounds().contains(at.0, at.1)
        {
            return false;
        }
        let Some(source_x) = at.0.checked_sub(self.offset.0) else {
            return false;
        };
        let Some(source_y) = at.1.checked_sub(self.offset.1) else {
            return false;
        };
        if !Self::hit_test(&self.options.selection, (source_x, source_y)) {
            return false;
        }
        self.start = at;
        self.start_offset = self.offset;
        self.held = true;
        self.revision.advance();
        true
    }
    pub fn offset(&self) -> (i64, i64) {
        self.offset
    }
    pub fn source_selection(&self) -> &Selection {
        &self.options.selection
    }
    pub fn revision(&self) -> PreviewRevision {
        self.revision
    }
    pub fn bounds(&self) -> Rect {
        self.options
            .bounds
            .translated(self.offset.0, self.offset.1)
            .expect("admitted floating translation")
    }
    /// Every motion is measured from the original pointer, never a previous preview.
    pub fn update(&mut self, at: (i64, i64)) -> Result<()> {
        if !self.active || !self.held {
            return Ok(());
        }
        if !crate::selection::valid_point(at) {
            self.cancel();
            return Err(invalid());
        }
        let bounds = self.options.bounds;
        let max = COORD_ABS_MAX;
        let dx =
            at.0.checked_sub(self.start.0)
                .and_then(|value| value.checked_add(self.start_offset.0))
                .ok_or_else(invalid)?
                .clamp((-max).max(-max - bounds.x0), max.min(max - bounds.x1));
        let dy =
            at.1.checked_sub(self.start.1)
                .and_then(|value| value.checked_add(self.start_offset.1))
                .ok_or_else(invalid)?
                .clamp((-max).max(-max - bounds.y0), max.min(max - bounds.y1));
        if self.offset != (dx, dy) {
            self.offset = (dx, dy);
            self.revision.advance();
        }
        Ok(())
    }
    pub fn update_subpixel(&mut self, at: (i64, i64)) -> Result<()> {
        self.update(pixels(at))
    }
    pub fn preview(&self) -> Option<Preview> {
        (self.active && self.offset != (0, 0)).then_some(Preview::MoveSelection {
            layer: self.options.layer,
            offset: self.offset,
            clear: self.options.clear,
            transparent_key: self.options.transparent_key,
        })
    }
    /// Foreign releases preserve ownership and do not admit their coordinates.
    pub fn release(
        &mut self,
        at: (i64, i64),
        button: PointerButton,
    ) -> Result<Option<(u64, Action)>> {
        if !self.active || !self.held || button != PointerButton::Primary {
            return Ok(None);
        }
        self.update(at)?;
        self.held = false;
        self.revision.advance();
        Ok(if self.options.commit_on_release {
            self.finish()
        } else {
            None
        })
    }
    pub fn finish(&mut self) -> Option<(u64, Action)> {
        if !self.active {
            return None;
        }
        self.active = false;
        self.held = false;
        self.revision.advance();
        (self.offset != (0, 0)).then_some((
            self.options.token,
            Action::MoveSelection {
                layer: self.options.layer,
                offset: self.offset,
                clear: self.options.clear,
                transparent_key: self.options.transparent_key,
            },
        ))
    }
    pub fn release_subpixel(
        &mut self,
        at: (i64, i64),
        button: PointerButton,
    ) -> Result<Option<(u64, Action)>> {
        self.release(pixels(at), button)
    }
    /// Escape, tool changes and source-ticket changes all discard the complete drag.
    pub fn cancel(&mut self) {
        if self.active {
            self.active = false;
            self.held = false;
            self.revision.advance();
        }
    }
}
fn pixels(at: (i64, i64)) -> (i64, i64) {
    (at.0.div_euclid(256), at.1.div_euclid(256))
}
fn invalid() -> Error {
    Error::new(
        ErrorCode::InputSchema,
        "floating selection exceeds coordinate bounds",
    )
}

impl crate::ToolState {
    pub fn begin_selection_move(
        &mut self,
        mut options: FloatingOptions,
        at: (i64, i64),
        button: PointerButton,
    ) -> Result<bool> {
        if self.tool != crate::Tool::Select || self.is_editing_text() {
            return Ok(false);
        }
        if let Some(floating) = &mut self.floating {
            let grabbed = floating.regrab(at, button);
            if grabbed {
                self.preview_revision.advance();
            }
            return Ok(grabbed);
        }
        if self.gesture_in_progress() {
            return Ok(false);
        }
        let next = self
            .next_gesture
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "gesture token range exhausted"))?;
        options.token = next;
        let Some(floating) = FloatingSelection::begin(options, at, button)? else {
            return Ok(false);
        };
        self.next_gesture = next;
        self.floating = Some(floating);
        self.preview_revision.advance();
        Ok(true)
    }
    pub fn begin_selection_move_subpixel(
        &mut self,
        options: FloatingOptions,
        at: (i64, i64),
        button: PointerButton,
    ) -> Result<bool> {
        self.begin_selection_move(options, pixels(at), button)
    }
    pub fn floating_selection(&self) -> Option<&FloatingSelection> {
        self.floating
            .as_ref()
            .filter(|f| f.is_active() && self.tool == crate::Tool::Select)
    }
    pub(super) fn update_floating(&mut self, at: (i64, i64)) -> Option<Result<()>> {
        self.floating.as_ref()?;
        if self.tool != crate::Tool::Select {
            self.cancel_gesture();
            return Some(Ok(()));
        }
        let floating = self.floating.as_mut().expect("floating drag");
        let revision = floating.revision();
        let result = floating.update(at);
        if revision != floating.revision() {
            self.preview_revision.advance();
        }
        if result.is_err() {
            self.floating = None;
        }
        Some(result)
    }
    pub fn finish_selection_move(&mut self) -> Option<(u64, Action)> {
        if self.tool != crate::Tool::Select {
            self.cancel_gesture();
            return None;
        }
        let mut floating = self.floating.take()?;
        self.preview_revision.advance();
        floating.finish()
    }
    pub(super) fn release_floating(
        &mut self,
        button: PointerButton,
    ) -> Option<Option<(u64, Action)>> {
        self.floating.as_ref()?;
        if button != PointerButton::Primary {
            return Some(None);
        }
        if self.tool != crate::Tool::Select {
            self.cancel_gesture();
            return Some(None);
        }
        let floating = self.floating.as_mut().expect("floating drag");
        let at = (
            floating.start.0 + floating.offset.0 - floating.start_offset.0,
            floating.start.1 + floating.offset.1 - floating.start_offset.1,
        );
        let revision = floating.revision();
        let result = floating
            .release(at, button)
            .expect("admitted floating point");
        if revision != floating.revision() {
            self.preview_revision.advance();
        }
        if !floating.is_active() {
            self.floating = None;
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests;
