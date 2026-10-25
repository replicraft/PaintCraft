//! Private external-image positioning; the host owns document and publication guards.
use crate::{PointerButton, PreviewRevision};
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Result, clipboard::Image, policy::COORD_ABS_MAX,
};

#[derive(Clone, Debug)]
pub struct ImagePlacement {
    image: Image,
    origin: (i64, i64),
    drag: Option<((i64, i64), (i64, i64))>,
    active: bool,
    revision: PreviewRevision,
}
impl ImagePlacement {
    pub fn new(image: Image, origin: (i64, i64)) -> Result<Self> {
        bounds(&image, origin)?;
        Ok(Self {
            image,
            origin,
            drag: None,
            active: true,
            revision: PreviewRevision::default(),
        })
    }
    pub fn image(&self) -> &Image {
        &self.image
    }
    pub fn origin(&self) -> (i64, i64) {
        self.origin
    }
    pub fn bounds(&self) -> Rect {
        bounds(&self.image, self.origin).expect("validated placement")
    }
    pub fn revision(&self) -> PreviewRevision {
        self.revision
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
    pub fn is_dragging(&self) -> bool {
        self.active && self.drag.is_some()
    }
    pub fn press(&mut self, at: (i64, i64), button: PointerButton) -> bool {
        if !self.active
            || self.drag.is_some()
            || button != PointerButton::Primary
            || !self.bounds().contains(at.0, at.1)
        {
            return false;
        }
        self.drag = Some((at, self.origin));
        true
    }
    pub fn move_to(&mut self, at: (i64, i64)) -> Result<()> {
        let Some((start, origin)) = self.drag.filter(|_| self.active) else {
            return Ok(());
        };
        let x =
            at.0.checked_sub(start.0)
                .and_then(|d| origin.0.checked_add(d));
        let y =
            at.1.checked_sub(start.1)
                .and_then(|d| origin.1.checked_add(d));
        match (x, y) {
            (Some(x), Some(y)) => self.place((x, y)),
            _ => Err(invalid()),
        }
    }
    pub fn place(&mut self, origin: (i64, i64)) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        bounds(&self.image, origin)?;
        if origin != self.origin {
            self.origin = origin;
            self.revision.advance();
        }
        Ok(())
    }
    pub fn release(&mut self, at: (i64, i64), button: PointerButton) -> Result<bool> {
        if !self.active || self.drag.is_none() || button != PointerButton::Primary {
            return Ok(false);
        }
        self.move_to(at)?;
        self.drag = None;
        Ok(true)
    }
    pub fn interrupt_drag(&mut self) {
        self.drag = None;
    }
    pub fn cancel(&mut self) {
        if self.active {
            self.active = false;
            self.drag = None;
            self.revision.advance();
        }
    }
}
fn bounds(image: &Image, origin: (i64, i64)) -> Result<Rect> {
    let r = Rect::from_origin_size(origin.0, origin.1, image.width(), image.height())
        .ok_or_else(invalid)?;
    if [r.x0, r.y0, r.x1, r.y1]
        .iter()
        .any(|v| v.unsigned_abs() > COORD_ABS_MAX as u64)
    {
        return Err(invalid());
    }
    Ok(r)
}
fn invalid() -> Error {
    Error::new(
        ErrorCode::InputSchema,
        "image placement exceeds coordinate policy",
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::resources::SharedBudget;
    fn placement() -> ImagePlacement {
        ImagePlacement::new(
            Image::from_rgba(
                2,
                1,
                vec![17, 44, 99, 0, 1, 2, 3, 127],
                &SharedBudget::new(8),
                &|| Ok(()),
            )
            .unwrap(),
            (1, 2),
        )
        .unwrap()
    }
    #[test]
    fn repeated_drags_keep_original_pixels_and_measure_each_drag_from_its_start() {
        let mut p = placement();
        let before = p.image().rgba().to_vec();
        let rev = p.revision();
        assert!(!p.press((1, 2), PointerButton::Secondary));
        assert!(p.press((1, 2), PointerButton::Primary));
        p.move_to((3, 4)).unwrap();
        p.move_to((5, 6)).unwrap();
        assert_eq!(p.origin(), (5, 6));
        assert_ne!(p.revision(), rev);
        assert!(!p.release((100, 100), PointerButton::Secondary).unwrap());
        assert!(p.is_dragging());
        assert!(p.release((4, 5), PointerButton::Primary).unwrap());
        assert_eq!(p.origin(), (4, 5));
        assert!(p.press((5, 5), PointerButton::Primary));
        p.move_to((6, 7)).unwrap();
        assert_eq!(p.origin(), (5, 7));
        assert_eq!(p.image().rgba(), before);
        p.interrupt_drag();
        assert!(!p.is_dragging());
        p.cancel();
        assert!(!p.is_active());
        assert!(!p.press((5, 7), PointerButton::Primary));
    }
    #[test]
    fn invalid_motion_preserves_last_valid_position_and_unchanged_motion_keeps_revision() {
        let mut p = placement();
        let rev = p.revision();
        p.place(p.origin()).unwrap();
        assert_eq!(p.revision(), rev);
        assert!(p.press((1, 2), PointerButton::Primary));
        assert!(p.move_to((i64::MAX, i64::MIN)).is_err());
        assert_eq!(p.origin(), (1, 2));
        assert!(p.place((COORD_ABS_MAX, 0)).is_err());
        assert_eq!(p.origin(), (1, 2));
        assert_eq!(p.revision(), rev);
    }
}
