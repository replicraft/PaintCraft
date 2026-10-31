//! Complete byte-output intent for the standalone five-knot RGB Curves foundation.

/// One encoded-RGB curve shared by the three color channels. Input knots are
/// fixed at `[0, 64, 128, 192, 255]`; these five bytes are their output values.
/// Outputs are independent: endpoints are unlocked and decreasing, constant or
/// nonmonotone curves are valid intentions. Alpha is not a curve channel.
///
/// This parameter type alone registers no Descriptor, parser, persistence,
/// execution capability, command or UI. A derived LUT is not its identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CurvesRgbV1 {
    outputs: [u8; 5],
}

impl CurvesRgbV1 {
    /// Every five-byte array is valid; no sorting, clamping or normalization.
    pub const fn new(outputs: [u8; 5]) -> Self {
        Self { outputs }
    }

    pub const fn outputs(self) -> [u8; 5] {
        self.outputs
    }

    /// Complete editable intent, independent of transport or a compiled LUT.
    /// Byte order follows the fixed ascending input knots; all five bytes count.
    pub const fn semantic_bytes(self) -> [u8; 5] {
        self.outputs
    }
}

impl Default for CurvesRgbV1 {
    fn default() -> Self {
        Self::new([0, 64, 128, 192, 255])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeSet, HashSet};

    #[test]
    fn identity_default_is_the_complete_const_five_byte_key() {
        const IDENTITY: CurvesRgbV1 = CurvesRgbV1::new([0, 64, 128, 192, 255]);
        const KEY: [u8; 5] = IDENTITY.semantic_bytes();
        assert_eq!(CurvesRgbV1::default(), IDENTITY);
        assert_eq!(KEY, [0, 64, 128, 192, 255]);
        assert_eq!(IDENTITY.outputs(), KEY);
    }

    #[test]
    fn free_endpoints_constant_and_nonmonotone_intent_is_preserved() {
        for outputs in [
            [0; 5],
            [255; 5],
            [255, 192, 128, 64, 0],
            [255, 0, 200, 10, 0],
            [17, 64, 128, 192, 231],
        ] {
            let parameter = CurvesRgbV1::new(outputs);
            assert_eq!(parameter.outputs(), outputs);
            assert_eq!(parameter.semantic_bytes(), outputs);
            assert_ne!(parameter, CurvesRgbV1::default());
        }
    }

    #[test]
    fn all_output_cube_vertices_keep_distinct_complete_equality_and_hash_keys() {
        // Literal corners of the five-dimensional byte-output cube. Fixed input
        // positions are not parameters and no monotonicity constraint removes a corner.
        const VERTICES: [[u8; 5]; 32] = [
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 255],
            [0, 0, 0, 255, 0],
            [0, 0, 0, 255, 255],
            [0, 0, 255, 0, 0],
            [0, 0, 255, 0, 255],
            [0, 0, 255, 255, 0],
            [0, 0, 255, 255, 255],
            [0, 255, 0, 0, 0],
            [0, 255, 0, 0, 255],
            [0, 255, 0, 255, 0],
            [0, 255, 0, 255, 255],
            [0, 255, 255, 0, 0],
            [0, 255, 255, 0, 255],
            [0, 255, 255, 255, 0],
            [0, 255, 255, 255, 255],
            [255, 0, 0, 0, 0],
            [255, 0, 0, 0, 255],
            [255, 0, 0, 255, 0],
            [255, 0, 0, 255, 255],
            [255, 0, 255, 0, 0],
            [255, 0, 255, 0, 255],
            [255, 0, 255, 255, 0],
            [255, 0, 255, 255, 255],
            [255, 255, 0, 0, 0],
            [255, 255, 0, 0, 255],
            [255, 255, 0, 255, 0],
            [255, 255, 0, 255, 255],
            [255, 255, 255, 0, 0],
            [255, 255, 255, 0, 255],
            [255, 255, 255, 255, 0],
            [255, 255, 255, 255, 255],
        ];
        let mut typed = HashSet::new();
        let mut keys = BTreeSet::new();
        for outputs in VERTICES {
            let parameter = CurvesRgbV1::new(outputs);
            assert_eq!(parameter.semantic_bytes(), outputs);
            assert!(typed.insert(parameter));
            assert!(!typed.insert(CurvesRgbV1::new(outputs)));
            assert!(keys.insert(parameter.semantic_bytes()));
        }
        assert_eq!(typed.len(), 32);
        assert_eq!(keys.len(), 32);
        assert!(typed.insert(CurvesRgbV1::default()));
        assert!(keys.insert(CurvesRgbV1::default().semantic_bytes()));
    }
}
