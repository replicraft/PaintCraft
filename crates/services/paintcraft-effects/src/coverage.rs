//! One post-filter coverage application for alpha-preserving point filters.
use paintcraft_core::types::Rgba;

/// Mix an unmasked candidate into this stage's incoming RGB, rounded to the
/// nearest byte. Coverage is not opacity: the exact incoming alpha is retained,
/// even when zero, and candidate alpha is ignored. Apply once per filter, before
/// layer opacity/composition. Alpha-changing/spatial algorithms need their own
/// coverage contract; this function does not define those semantics.
pub fn mix_point_coverage(original: Rgba, candidate: Rgba, coverage: u8) -> Rgba {
    let mut output = original;
    let weight = u32::from(coverage);
    for (out, candidate) in output.0[..3].iter_mut().zip(candidate.0) {
        let numerator = (255 - weight) * u32::from(*out) + weight * u32::from(candidate);
        *out = ((numerator + 127) / 255) as u8;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_fractional_coverage_preserves_hidden_rgb_and_original_alpha() {
        let source = [0, 255, 10];
        let candidate = [255, 0, 245];
        for alpha in [0, 1, 127, 128, 254, 255] {
            for (coverage, rgb) in [
                (0, [0, 255, 10]),
                (1, [1, 254, 11]),
                (127, [127, 128, 127]),
                (128, [128, 127, 128]),
                (254, [254, 1, 244]),
                (255, [255, 0, 245]),
            ] {
                assert_eq!(
                    mix_point_coverage(
                        Rgba([source[0], source[1], source[2], alpha]),
                        Rgba([candidate[0], candidate[1], candidate[2], 255 - alpha]),
                        coverage,
                    ),
                    Rgba([rgb[0], rgb[1], rgb[2], alpha]),
                );
            }
        }
    }

    #[test]
    fn every_byte_pair_and_coverage_matches_independent_real_interpolation() {
        // Every exact value has denominator 255: it cannot be a half-integer.
        // The nearest half is at least 1/510 away, far beyond f64 error here.
        // This oracle uses real interpolation/rounding, not integer numerator
        // arithmetic shared with the implementation.
        for incoming in 0..=255u8 {
            for candidate in 0..=255u8 {
                for coverage in 0..=255u8 {
                    let exact = f64::from(incoming)
                        + (f64::from(candidate) - f64::from(incoming))
                            * (f64::from(coverage) / 255.0);
                    let expected = exact.round() as u8;
                    assert_eq!(
                        mix_point_coverage(Rgba([incoming; 4]), Rgba([candidate; 4]), coverage,),
                        Rgba([expected, expected, expected, incoming]),
                        "incoming={incoming} candidate={candidate} coverage={coverage}",
                    );
                }
            }
        }
    }

    #[test]
    fn coverage_is_per_stage_and_is_not_applied_twice() {
        use crate::evaluate_pixel;
        use paintcraft_core::types::filters::Descriptor;
        let source = Rgba([0, 0, 0, 17]);
        let first = mix_point_coverage(
            source,
            evaluate_pixel(&[Descriptor::InvertRgbV1], source).unwrap(),
            128,
        );
        assert_eq!(first, Rgba([128, 128, 128, 17]));
        let second = mix_point_coverage(
            first,
            evaluate_pixel(&[Descriptor::InvertRgbV1], first).unwrap(),
            128,
        );
        assert_eq!(second, Rgba([127, 127, 127, 17]));
        assert_ne!(second, source, "Do not fuse masks after the whole stack");
        assert_ne!(
            mix_point_coverage(source, first, 128),
            first,
            "Do not apply the same stage coverage twice",
        );
    }
}
