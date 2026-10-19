//! Continuous coordinates have explicit spaces; integer pixel addressing stays in [`super::Rect`].
//! Glam is an implementation detail, never an on-disk, command or GPU representation.
use glam::{DAffine2, DVec2};
use std::marker::PhantomData;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DocumentSpace {}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LayerSpace {}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScreenSpace {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point<S> {
    value: DVec2,
    space: PhantomData<S>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Displacement<S> {
    value: DVec2,
    space: PhantomData<S>,
}

impl<S> Point<S> {
    pub fn new(x: f64, y: f64) -> Option<Self> {
        let value = DVec2::new(x, y);
        value.is_finite().then_some(Self {
            value,
            space: PhantomData,
        })
    }
    pub fn coordinates(&self) -> (f64, f64) {
        (self.value.x, self.value.y)
    }
    pub fn offset(&self, by: Displacement<S>) -> Option<Self> {
        let value = self.value + by.value;
        Self::new(value.x, value.y)
    }
}
impl<S> Displacement<S> {
    pub fn new(x: f64, y: f64) -> Option<Self> {
        let value = DVec2::new(x, y);
        value.is_finite().then_some(Self {
            value,
            space: PhantomData,
        })
    }
    pub fn coordinates(&self) -> (f64, f64) {
        (self.value.x, self.value.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine<From, To> {
    value: DAffine2,
    spaces: PhantomData<(From, To)>,
}
impl<From, To> Affine<From, To> {
    /// Column order: m00, m10, m01, m11, translation_x, translation_y.
    pub fn from_columns(columns: [f64; 6]) -> Option<Self> {
        let value = DAffine2::from_cols_array(&columns);
        value.is_finite().then_some(Self {
            value,
            spaces: PhantomData,
        })
    }
    pub fn scale_and_translation(scale: [f64; 2], translation: Displacement<To>) -> Option<Self> {
        let (x, y) = translation.coordinates();
        Self::from_columns([scale[0], 0.0, 0.0, scale[1], x, y])
    }
    pub fn transform_point(&self, point: Point<From>) -> Option<Point<To>> {
        let value = self.value.transform_point2(point.value);
        Point::new(value.x, value.y)
    }
    pub fn transform_displacement(&self, vector: Displacement<From>) -> Option<Displacement<To>> {
        let value = self.value.transform_vector2(vector.value);
        Displacement::new(value.x, value.y)
    }
    /// Apply this transform, then `next`; composition order follows the spaces.
    pub fn then<Next>(&self, next: &Affine<To, Next>) -> Option<Affine<From, Next>> {
        let value = next.value * self.value;
        value.is_finite().then_some(Affine {
            value,
            spaces: PhantomData,
        })
    }
    /// The operation chooses its minimum accepted absolute determinant.
    pub fn inverse(&self, minimum_determinant: f64) -> Option<Affine<To, From>> {
        let determinant = self.value.matrix2.determinant();
        if !minimum_determinant.is_finite()
            || minimum_determinant < 0.0
            || !determinant.is_finite()
            || determinant.abs() <= minimum_determinant
        {
            return None;
        }
        let value = self.value.inverse();
        value.is_finite().then_some(Affine {
            value,
            spaces: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spaces_composition_and_vectors_preserve_their_meaning() {
        let layer_to_doc = Affine::<LayerSpace, DocumentSpace>::scale_and_translation(
            [-2.0, 3.0],
            Displacement::new(100.0, -50.0).unwrap(),
        )
        .unwrap();
        let doc_to_screen = Affine::<DocumentSpace, ScreenSpace>::scale_and_translation(
            [0.5, 0.5],
            Displacement::new(10.0, 20.0).unwrap(),
        )
        .unwrap();
        let point = Point::new(4.0, 6.0).unwrap();
        let composed = layer_to_doc.then(&doc_to_screen).unwrap();
        assert_eq!(
            composed.transform_point(point).unwrap().coordinates(),
            (56.0, 4.0)
        );
        assert_eq!(
            composed.transform_point(point),
            doc_to_screen.transform_point(layer_to_doc.transform_point(point).unwrap())
        );
        assert_eq!(
            layer_to_doc
                .transform_displacement(Displacement::new(4.0, 6.0).unwrap())
                .unwrap()
                .coordinates(),
            (-8.0, 18.0)
        );
    }
    #[test]
    fn inverse_rejects_nonfinite_and_operation_specific_singularities() {
        assert!(Point::<DocumentSpace>::new(f64::NAN, 0.0).is_none());
        assert!(
            Affine::<DocumentSpace, ScreenSpace>::from_columns([
                1.0,
                0.0,
                0.0,
                1.0,
                f64::INFINITY,
                0.0
            ])
            .is_none()
        );
        for scale in [0.0, 1e-15] {
            let transform = Affine::<DocumentSpace, ScreenSpace>::scale_and_translation(
                [scale, 1.0],
                Displacement::new(0.0, 0.0).unwrap(),
            )
            .unwrap();
            assert!(transform.inverse(1e-12).is_none());
        }
    }
    #[test]
    fn local_rebasing_preserves_large_document_pixel_centers() {
        let origin = (1i64 << 42) as f64;
        let transform = Affine::<DocumentSpace, ScreenSpace>::scale_and_translation(
            [2.0, 2.0],
            Displacement::new(-origin * 2.0, -origin * 2.0).unwrap(),
        )
        .unwrap();
        let source = Point::new(origin + 0.5, origin + 7.5).unwrap();
        let local = transform.transform_point(source).unwrap();
        assert_eq!(local.coordinates(), (1.0, 15.0));
        assert_eq!(
            transform.inverse(0.0).unwrap().transform_point(local),
            Some(source)
        );
    }
}
