//! Standalone encoded-RGB Vibrance intent. No Descriptor or wire registration.
use super::{Error, ErrorCode, Result};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VibranceRgbV1 {
    amount_hundredths: i16,
}
impl VibranceRgbV1 {
    pub fn new(amount_hundredths: i16) -> Result<Self> {
        if !(-100..=100).contains(&amount_hundredths) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Vibrance requires amount hundredths -100..100",
            ));
        }
        Ok(Self { amount_hundredths })
    }
    pub const fn amount_hundredths(self) -> i16 {
        self.amount_hundredths
    }
    /// Complete signed intent; this does not register a persistence codec.
    pub const fn semantic_bytes(self) -> [u8; 2] {
        self.amount_hundredths.to_be_bytes()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_signed_intent_and_default_are_preserved() {
        for (a, bytes) in [
            (-100, [255, 156]),
            (-1, [255, 255]),
            (0, [0, 0]),
            (1, [0, 1]),
            (100, [0, 100]),
        ] {
            let p = VibranceRgbV1::new(a).unwrap();
            assert_eq!(p.amount_hundredths(), a);
            assert_eq!(p.semantic_bytes(), bytes);
        }
        assert_eq!(VibranceRgbV1::default(), VibranceRgbV1::new(0).unwrap());
        assert_ne!(
            VibranceRgbV1::new(-1).unwrap(),
            VibranceRgbV1::new(1).unwrap()
        );
    }
    #[test]
    fn invalid_intent_is_refused_not_clamped() {
        for a in [i16::MIN, -101, 101, i16::MAX] {
            assert_eq!(
                VibranceRgbV1::new(a).unwrap_err().code,
                ErrorCode::InputSchema
            );
        }
    }
}
