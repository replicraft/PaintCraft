//! Checked half-open rectangles in signed document or layer coordinates.

/// `[x0, x1) × [y0, y1)`. Always non-empty when constructed through [`Rect::new`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x0: i64,
    pub y0: i64,
    pub x1: i64,
    pub y1: i64,
}

impl Rect {
    /// `None` for an empty rectangle or on arithmetic overflow.
    pub fn new(x: i64, y: i64, width: i64, height: i64) -> Option<Self> {
        let (x1, y1) = (x.checked_add(width)?, y.checked_add(height)?);
        (width > 0 && height > 0).then_some(Self {
            x0: x,
            y0: y,
            x1,
            y1,
        })
    }

    pub fn from_origin_size(x: i64, y: i64, width: u32, height: u32) -> Option<Self> {
        Self::new(x, y, i64::from(width), i64::from(height))
    }

    pub fn width(&self) -> i64 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> i64 {
        self.y1 - self.y0
    }

    pub fn contains(&self, x: i64, y: i64) -> bool {
        self.x0 <= x && x < self.x1 && self.y0 <= y && y < self.y1
    }

    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let r = Rect {
            x0: self.x0.max(other.x0),
            y0: self.y0.max(other.y0),
            x1: self.x1.min(other.x1),
            y1: self.y1.min(other.y1),
        };
        (r.x0 < r.x1 && r.y0 < r.y1).then_some(r)
    }

    /// Area in pixels; fits `u128` for any representable rectangle.
    pub fn area(&self) -> u128 {
        (self.width() as u128) * (self.height() as u128)
    }

    /// Translation that fails rather than wrapping.
    pub fn translated(&self, dx: i64, dy: i64) -> Option<Rect> {
        Some(Rect {
            x0: self.x0.checked_add(dx)?,
            y0: self.y0.checked_add(dy)?,
            x1: self.x1.checked_add(dx)?,
            y1: self.y1.checked_add(dy)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_overflowing() {
        assert!(Rect::new(0, 0, 0, 1).is_none());
        assert!(Rect::new(0, 0, 1, -1).is_none());
        assert!(Rect::new(i64::MAX, 0, 1, 1).is_none());
        assert!(Rect::new(i64::MAX - 1, 0, 1, 1).is_some());
    }

    #[test]
    fn half_open_contains_and_intersect() {
        let a = Rect::new(-2, -2, 4, 4).unwrap();
        assert!(a.contains(-2, -2) && a.contains(1, 1));
        assert!(!a.contains(2, 0) && !a.contains(0, 2));
        let b = Rect::new(1, 1, 5, 5).unwrap();
        assert_eq!(a.intersect(&b), Rect::new(1, 1, 1, 1));
        assert_eq!(a.intersect(&Rect::new(2, 0, 3, 3).unwrap()), None);
    }

    #[test]
    fn translation_is_checked() {
        let a = Rect::new(0, 0, 1, 1).unwrap();
        assert_eq!(a.translated(-3, 2), Rect::new(-3, 2, 1, 1));
        assert_eq!(a.translated(i64::MAX, 0), None);
    }
}
