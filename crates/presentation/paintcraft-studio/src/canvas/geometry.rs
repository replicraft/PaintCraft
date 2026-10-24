//! Navigation bounds, measured in logical screen points independently per axis.
use egui::Vec2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Geometry {
    pub viewport: Vec2,
    pub leading: Vec2,
    pub extent: Vec2,
    pub maximum: Vec2,
    pub initial: Vec2,
}

impl Geometry {
    pub fn new(viewport: Vec2, canvas: Vec2, overscroll: bool) -> Self {
        let viewport = viewport.max(Vec2::splat(1.0));
        let leading = if overscroll {
            viewport - canvas.min(viewport) * 0.5
        } else {
            ((viewport - canvas) * 0.5).max(Vec2::splat(12.0))
        };
        let extent = if overscroll {
            canvas + leading * 2.0
        } else {
            (leading + canvas).max(viewport)
        };
        let maximum = (extent - viewport).max(Vec2::ZERO);
        let initial = if overscroll {
            maximum * 0.5
        } else {
            Vec2::ZERO
        };
        Self {
            viewport,
            leading,
            extent,
            maximum,
            initial,
        }
    }

    pub fn clamp(self, offset: Vec2) -> Vec2 {
        offset.max(Vec2::ZERO).min(self.maximum)
    }

    /// Keep the same screen-space image point under the viewport center when
    /// padding or viewport geometry changes. Hosts still own explicit zoom targets.
    pub fn preserve_center(self, previous: Self, offset: Vec2) -> Vec2 {
        self.clamp(
            offset + self.leading - previous.leading + (previous.viewport - self.viewport) * 0.5,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn small_images_can_move_half_offscreen_and_large_edges_reach_the_center() {
        let view = Vec2::new(400.0, 300.0);
        let small = Geometry::new(view, Vec2::new(100.0, 80.0), true);
        assert_eq!(small.leading, Vec2::new(350.0, 260.0));
        assert_eq!(small.leading - small.maximum, Vec2::new(-50.0, -40.0));
        assert_eq!(small.leading - small.initial, Vec2::new(150.0, 110.0));
        let large = Geometry::new(view, Vec2::new(1000.0, 800.0), true);
        assert_eq!(large.leading, Vec2::new(200.0, 150.0));
        assert_eq!(large.leading - large.maximum, Vec2::new(-800.0, -650.0));
        let mixed = Geometry::new(view, Vec2::new(100.0, 800.0), true);
        assert_eq!(mixed.leading, Vec2::new(350.0, 150.0));
        assert_eq!(mixed.leading - mixed.maximum, Vec2::new(-50.0, -650.0));
    }
    #[test]
    fn enabling_and_resizing_keep_the_center_and_disabled_small_images_cannot_scroll() {
        let size = Vec2::new(100.0, 80.0);
        let old = Geometry::new(Vec2::new(400.0, 300.0), size, false);
        let on = Geometry::new(old.viewport, size, true);
        assert_eq!(old.maximum, Vec2::ZERO);
        assert_eq!(on.preserve_center(old, Vec2::ZERO), on.initial);
        assert_eq!(old.preserve_center(on, on.initial), Vec2::ZERO);
        let resized = Geometry::new(Vec2::new(600.0, 400.0), size, true);
        assert_eq!(resized.preserve_center(on, on.initial), resized.initial);
    }
}
