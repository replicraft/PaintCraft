//! Standalone exact encoded-RGB Hue/Saturation intent, not a Descriptor.
use super::{Error, ErrorCode, Result};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HueSaturationRgbV1 {
    hue_degrees: i16,
    saturation_hundredths: i16,
    lightness_hundredths: i16,
}
impl HueSaturationRgbV1 {
    pub fn new(
        hue_degrees: i16,
        saturation_hundredths: i16,
        lightness_hundredths: i16,
    ) -> Result<Self> {
        if !(-180..=180).contains(&hue_degrees)
            || !(-100..=100).contains(&saturation_hundredths)
            || !(-100..=100).contains(&lightness_hundredths)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Hue/Saturation requires degrees -180..180 and saturation/lightness hundredths -100..100",
            ));
        }
        Ok(Self {
            hue_degrees,
            saturation_hundredths,
            lightness_hundredths,
        })
    }
    pub const fn hue_degrees(self) -> i16 {
        self.hue_degrees
    }
    pub const fn saturation_hundredths(self) -> i16 {
        self.saturation_hundredths
    }
    pub const fn lightness_hundredths(self) -> i16 {
        self.lightness_hundredths
    }
    /// Complete signed intent in H,S,L order. Equal effects do not erase intent.
    pub const fn semantic_bytes(self) -> [u8; 6] {
        let h = self.hue_degrees.to_be_bytes();
        let s = self.saturation_hundredths.to_be_bytes();
        let l = self.lightness_hundredths.to_be_bytes();
        [h[0], h[1], s[0], s[1], l[0], l[1]]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_boundaries_and_complete_intent_are_not_normalized_away() {
        for h in [-180, 0, 180] {
            for s in [-100, 0, 100] {
                for l in [-100, 0, 100] {
                    let p = HueSaturationRgbV1::new(h, s, l).unwrap();
                    assert_eq!(
                        (
                            p.hue_degrees(),
                            p.saturation_hundredths(),
                            p.lightness_hundredths()
                        ),
                        (h, s, l)
                    );
                    let b = p.semantic_bytes();
                    assert_eq!(
                        [
                            i16::from_be_bytes([b[0], b[1]]),
                            i16::from_be_bytes([b[2], b[3]]),
                            i16::from_be_bytes([b[4], b[5]])
                        ],
                        [h, s, l]
                    );
                }
            }
        }
        assert_eq!(
            HueSaturationRgbV1::default(),
            HueSaturationRgbV1::new(0, 0, 0).unwrap()
        );
        assert_eq!(
            HueSaturationRgbV1::new(-180, -100, 100)
                .unwrap()
                .semantic_bytes(),
            [255, 76, 255, 156, 0, 100]
        );
        assert_ne!(
            HueSaturationRgbV1::new(-180, 0, 0).unwrap(),
            HueSaturationRgbV1::new(180, 0, 0).unwrap()
        );
    }
    #[test]
    fn invalid_axes_refuse_instead_of_clamping() {
        for h in [i16::MIN, -181, 181, i16::MAX] {
            assert_eq!(
                HueSaturationRgbV1::new(h, 0, 0).unwrap_err().code,
                ErrorCode::InputSchema
            );
        }
        for a in [i16::MIN, -101, 101, i16::MAX] {
            assert_eq!(
                HueSaturationRgbV1::new(0, a, 0).unwrap_err().code,
                ErrorCode::InputSchema
            );
            assert_eq!(
                HueSaturationRgbV1::new(0, 0, a).unwrap_err().code,
                ErrorCode::InputSchema
            );
        }
    }
    #[test]
    fn typed_catalog_is_explicit_while_legacy_descriptor_boundaries_stay_closed() {
        use super::super::Descriptor;
        use crate::types::json::Value;
        let v = Value::object([
            ("kind", Value::str("hue-saturation-rgb-v1")),
            ("hue_degrees", Value::Int(0)),
            ("saturation_hundredths", Value::Int(0)),
            ("lightness_hundredths", Value::Int(0)),
        ]);
        assert_eq!(
            Descriptor::from_value(&v).unwrap_err().code,
            ErrorCode::InputSchema
        );
        assert_eq!(
            Descriptor::from_catalog_value(&v).unwrap(),
            Descriptor::HueSaturationRgbV1(HueSaturationRgbV1::default())
        );
        assert_eq!(
            Descriptor::checked("hue-saturation-rgb-v1", None, None)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
