//! Navigation bounds, measured in logical screen points independently per axis.
use egui::Vec2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Geometry {
    pub viewport: Vec2,
    canvas: Vec2,
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
            canvas,
            leading,
            extent,
            maximum,
            initial,
        }
    }

    pub fn clamp(self, offset: Vec2) -> Vec2 {
        offset.max(Vec2::ZERO).min(self.maximum)
    }

    /// Keep the same relative image point under the viewport center when the
    /// displayed image size or viewport changes. For a fixed document this
    /// preserves its pixel coordinate across zoom. Explicit anchors still win.
    pub fn preserve_center(self, previous: Self, offset: Vec2) -> Vec2 {
        let point = (offset + previous.viewport * 0.5 - previous.leading)
            / previous.canvas.max(Vec2::splat(f32::MIN_POSITIVE));
        self.clamp(self.leading + point * self.canvas - self.viewport * 0.5)
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

#[cfg(test)]
mod zoom_tests {
    use super::*;
    #[test]
    fn panned_document_point_survives_zoom_and_combined_viewport_resize() {
        let old = Geometry::new(Vec2::new(400.0, 300.0), Vec2::new(1000.0, 800.0), true);
        let panned = Vec2::new(650.0, 300.0);
        let small = Geometry::new(old.viewport, Vec2::new(125.0, 100.0), true);
        assert_eq!(small.preserve_center(old, panned), Vec2::new(218.75, 137.5));
        assert_eq!(small.preserve_center(old, old.initial), small.initial);
        let resized = Geometry::new(Vec2::new(600.0, 400.0), Vec2::new(250.0, 200.0), true);
        assert_eq!(
            resized.preserve_center(old, panned),
            Vec2::new(337.5, 175.0)
        );
        let original = old.preserve_center(small, small.preserve_center(old, panned));
        assert!((original - panned).length() < 0.001);
    }
    #[test]
    fn overscroll_profiles_keep_reachable_points_and_clamp_unreachable_points() {
        let size = Vec2::new(1000.0, 800.0);
        let off = Geometry::new(Vec2::new(400.0, 300.0), size, false);
        let on = Geometry::new(off.viewport, size, true);
        assert_eq!(
            on.preserve_center(off, Vec2::new(350.0, 210.0)),
            Vec2::new(538.0, 348.0)
        );
        let small = Geometry::new(off.viewport, Vec2::new(100.0, 80.0), false);
        assert_eq!(
            small.preserve_center(on, Vec2::new(650.0, 300.0)),
            Vec2::ZERO
        );
        assert_eq!(small.maximum, Vec2::ZERO);
        // Sub-point displayed dimensions remain meaningful, not rounded up to1.
        let tiny = Geometry::new(on.viewport, Vec2::new(0.125, 0.125), true);
        let grown = Geometry::new(on.viewport, Vec2::new(1.0, 1.0), true);
        assert_eq!(grown.preserve_center(tiny, tiny.initial), grown.initial);
    }
}
