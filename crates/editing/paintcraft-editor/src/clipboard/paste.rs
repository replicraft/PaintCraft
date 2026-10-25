//! An immutable external image with unpublished, source-bound placement metadata.
use super::*;
use paintcraft_core::types::{
    Rect,
    policy::{CANVAS_SIDE_MAX, COORD_ABS_MAX},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteCanvas {
    /// Preserve the canvas and retain all off-canvas image pixels in the new layer.
    Keep,
    /// Grow right/down to contain a nonnegative placement, within canvas limits.
    Grow,
}

#[derive(Clone, Debug)]
pub struct PasteDraft {
    pub(crate) source: HistoryTicket,
    pub(crate) image: Image,
    pub(crate) canvas: (u32, u32),
    pub(crate) origin: (i64, i64),
    pub(crate) mode: PasteCanvas,
}
impl PasteDraft {
    pub fn new(session: &Session, image: Image) -> Result<Self> {
        if image.width() > CANVAS_SIDE_MAX || image.height() > CANVAS_SIDE_MAX {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "pasted layer dimensions exceed document limits",
            ));
        }
        Ok(Self {
            source: session.history_ticket(),
            image,
            canvas: session.document().canvas,
            origin: (0, 0),
            mode: PasteCanvas::Keep,
        })
    }
    pub fn image(&self) -> &Image {
        &self.image
    }
    pub fn origin(&self) -> (i64, i64) {
        self.origin
    }
    pub fn mode(&self) -> PasteCanvas {
        self.mode
    }
    pub fn source(&self) -> HistoryTicket {
        self.source
    }
    pub fn bounds(&self) -> Rect {
        Rect::from_origin_size(
            self.origin.0,
            self.origin.1,
            self.image.width(),
            self.image.height(),
        )
        .expect("validated placement")
    }
    pub fn canvas_size(&self) -> (u32, u32) {
        if self.mode == PasteCanvas::Keep {
            self.canvas
        } else {
            (
                self.canvas.0.max(self.bounds().x1 as u32),
                self.canvas.1.max(self.bounds().y1 as u32),
            )
        }
    }
    /// Repeated placements change only this draft. A rejected placement retains
    /// the earlier valid position; dropping the draft cancels without publication.
    pub fn place(&mut self, origin: (i64, i64), mode: PasteCanvas) -> Result<()> {
        let bounds =
            Rect::from_origin_size(origin.0, origin.1, self.image.width(), self.image.height())
                .ok_or_else(|| Error::new(ErrorCode::InputSchema, "paste placement overflows"))?;
        if [bounds.x0, bounds.y0, bounds.x1, bounds.y1]
            .iter()
            .any(|n| !(-COORD_ABS_MAX..=COORD_ABS_MAX).contains(n))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "paste placement exceeds coordinate limits",
            ));
        }
        if mode == PasteCanvas::Grow
            && (origin.0 < 0
                || origin.1 < 0
                || bounds.x1 > i64::from(CANVAS_SIDE_MAX)
                || bounds.y1 > i64::from(CANVAS_SIDE_MAX))
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "canvas growth requires a nonnegative placement within canvas limits",
            ));
        }
        self.origin = origin;
        self.mode = mode;
        Ok(())
    }
}
