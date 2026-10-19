//! Mapping between screen points and document pixels.
//!
//! At `zoom == 1` one document pixel covers exactly one physical screen pixel,
//! whatever the display scale (`pixels_per_point`).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Top-left of the canvas in screen points.
    pub origin: (f32, f32),
    /// Whole-number magnification; 1 is actual pixels.
    pub zoom: u32,
    pub pixels_per_point: f32,
}

impl View {
    pub fn new(origin: (f32, f32), zoom: u32, pixels_per_point: f32) -> View {
        View {
            origin,
            zoom: zoom.max(1),
            pixels_per_point: pixels_per_point.max(f32::MIN_POSITIVE),
        }
    }

    /// Screen points covered by one document pixel.
    pub fn points_per_pixel(&self) -> f32 {
        self.zoom as f32 / self.pixels_per_point
    }

    /// Size of the whole canvas in screen points.
    pub fn canvas_size(&self, width: u32, height: u32) -> (f32, f32) {
        (
            width as f32 * self.points_per_pixel(),
            height as f32 * self.points_per_pixel(),
        )
    }

    /// The document pixel under a screen point; may lie outside the canvas.
    /// Non-finite input maps to `None`.
    pub fn to_doc(&self, point: (f32, f32)) -> Option<(i64, i64)> {
        let s = self.points_per_pixel();
        let (x, y) = ((point.0 - self.origin.0) / s, (point.1 - self.origin.1) / s);
        (x.is_finite() && y.is_finite() && x.abs() < 1e12 && y.abs() < 1e12)
            .then(|| (x.floor() as i64, y.floor() as i64))
    }

    pub fn to_screen(&self, doc: (i64, i64)) -> (f32, f32) {
        let s = self.points_per_pixel();
        (
            self.origin.0 + doc.0 as f32 * s,
            self.origin.1 + doc.1 as f32 * s,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_pixels_follow_the_display_scale() {
        for ppp in [1.0, 1.5, 2.0] {
            let v = View::new((10.0, 20.0), 1, ppp);
            // One document pixel is exactly one physical pixel.
            assert!((v.points_per_pixel() * ppp - 1.0).abs() < 1e-6);
            assert_eq!(v.to_doc(v.to_screen((7, 3))), Some((7, 3)));
        }
    }

    #[test]
    fn floors_toward_negative_infinity_and_handles_zoom() {
        let v = View::new((100.0, 100.0), 4, 1.0);
        assert_eq!(v.to_doc((100.0, 100.0)), Some((0, 0)));
        assert_eq!(v.to_doc((103.9, 107.9)), Some((0, 1)));
        assert_eq!(v.to_doc((99.9, 99.0)), Some((-1, -1)));
        assert_eq!(v.canvas_size(10, 5), (40.0, 20.0));
    }

    #[test]
    fn nonsense_input_is_rejected_not_wrapped() {
        let v = View::new((0.0, 0.0), 1, 1.0);
        assert_eq!(v.to_doc((f32::NAN, 0.0)), None);
        assert_eq!(v.to_doc((f32::INFINITY, 0.0)), None);
        assert_eq!(v.to_doc((1e30, 0.0)), None);
        assert_eq!(View::new((0.0, 0.0), 0, 0.0).zoom, 1);
    }
}
