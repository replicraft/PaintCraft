//! Mapping between screen points and document pixels.
//!
//! At `zoom == 1` one document pixel covers exactly one physical screen pixel,
//! whatever the display scale (`pixels_per_point`).

use paintcraft_core::types::coordinates::{
    Affine, Displacement, DocumentSpace, Point, ScreenSpace,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Top-left of the canvas in screen points.
    pub origin: (f32, f32),
    /// Whole-number magnification; 1 is actual pixels.
    pub zoom: u32,
    pub pixels_per_point: f32,
    mapping: Option<Affine<DocumentSpace, ScreenSpace>>,
    inverse: Option<Affine<ScreenSpace, DocumentSpace>>,
}

impl View {
    pub fn new(origin: (f32, f32), zoom: u32, pixels_per_point: f32) -> View {
        let zoom = zoom.max(1);
        let pixels_per_point = if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
            pixels_per_point
        } else {
            1.0
        };
        let scale = f64::from(zoom) / f64::from(pixels_per_point);
        let mapping = Displacement::new(f64::from(origin.0), f64::from(origin.1))
            .and_then(|offset| Affine::scale_and_translation([scale, scale], offset));
        let inverse = mapping.and_then(|transform| transform.inverse(0.0));
        View {
            origin,
            zoom,
            pixels_per_point,
            mapping,
            inverse,
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
        let (x, y) = self
            .inverse?
            .transform_point(Point::new(f64::from(point.0), f64::from(point.1))?)?
            .coordinates();
        (x.abs() < 1e12 && y.abs() < 1e12).then(|| (x.floor() as i64, y.floor() as i64))
    }

    pub fn to_screen(&self, doc: (i64, i64)) -> (f32, f32) {
        let result = self
            .mapping
            .and_then(|mapping| mapping.transform_point(Point::new(doc.0 as f64, doc.1 as f64)?));
        result.map_or((f32::NAN, f32::NAN), |point| {
            let (x, y) = point.coordinates();
            (x as f32, y as f32)
        })
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
