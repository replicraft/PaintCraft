//! Checked inverse of a native 2D INPUT_TRANSFORM, in Windows row-vector order.
//! Perspective/out-of-plane transforms are rejected rather than guessed.
pub(super) fn client(matrix: [f32; 16], screen: (f64, f64)) -> Option<(f64, f64)> {
    if matrix.iter().any(|v| !v.is_finite()) || !screen.0.is_finite() || !screen.1.is_finite() {
        return None;
    }
    // Plain HWND consumers use a 2D affine transform. Preserve all of its
    // translation/scale/skew/rotation; do not silently flatten 3D coordinates.
    for index in [2, 3, 6, 7, 8, 9, 11, 14] {
        if matrix[index] != 0.0 {
            return None;
        }
    }
    if matrix[10] != 1.0 || matrix[15] != 1.0 {
        return None;
    }
    let a = matrix[0] as f64;
    let b = matrix[1] as f64;
    let c = matrix[4] as f64;
    let d = matrix[5] as f64;
    let determinant = a * d - b * c;
    if !determinant.is_finite() || determinant == 0.0 {
        return None;
    }
    let x = screen.0 - matrix[12] as f64;
    let y = screen.1 - matrix[13] as f64;
    let result = ((d * x - c * y) / determinant, (a * y - b * x) / determinant);
    (result.0.is_finite() && result.1.is_finite()).then_some(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> [f32; 16] {
        let mut m = [0.0; 16];
        for i in [0, 5, 10, 15] {
            m[i] = 1.0;
        }
        m
    }
    #[test]
    fn inverse_preserves_native_subpixel_scale_rotation_and_translation() {
        let mut m = identity();
        m[0] = 0.0;
        m[1] = 2.0;
        m[4] = -3.0;
        m[5] = 0.0;
        m[12] = 100.0;
        m[13] = -50.0;
        assert_eq!(client(m, (89.5, -45.5)), Some((2.25, 3.5)));
        let mut shear = identity();
        shear[4] = 0.5;
        assert_eq!(client(shear, (6.5, 3.0)), Some((5.0, 3.0)));
    }
    #[test]
    fn malformed_or_unrepresented_native_transforms_fail_closed() {
        let mut m = identity();
        m[5] = 0.0;
        assert!(client(m, (1.0, 2.0)).is_none());
        for (index, value) in [(3, 0.5), (2, 1.0), (0, f32::NAN), (15, 0.0)] {
            let mut m = identity();
            m[index] = value;
            assert!(client(m, (1.0, 2.0)).is_none());
        }
    }
}
