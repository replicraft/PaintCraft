//! Exact EV intent for the standalone linear-light sRGB Exposure foundation.
use super::{Error, ErrorCode, Result};

/// Channel-independent linear-light sRGB exposure, preserving alpha and
/// transforming hidden RGB. Stops are the exact rational `stops_hundredths/100`.
/// This type alone grants no Descriptor, persistence, command or UI capability.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ExposureSrgbV1 {
    stops_hundredths: i16,
}

impl ExposureSrgbV1 {
    pub fn new(stops_hundredths: i16) -> Result<Self> {
        if !(-800..=800).contains(&stops_hundredths) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Exposure requires EV hundredths -800..800",
            ));
        }
        Ok(Self { stops_hundredths })
    }

    pub const fn stops_hundredths(self) -> i16 {
        self.stops_hundredths
    }

    /// Complete editable intent, independent of a transport or derived LUT.
    /// Signed two's-complement big-endian EV hundredths; zero is exact identity.
    pub const fn semantic_bytes(self) -> [u8; 2] {
        self.stops_hundredths.to_be_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_extremes_and_neutral_have_canonical_distinct_intentions() {
        for (ev, bytes) in [
            (-800, [0xfc, 0xe0]),
            (-1, [0xff, 0xff]),
            (0, [0, 0]),
            (1, [0, 1]),
            (800, [3, 0x20]),
        ] {
            let p = ExposureSrgbV1::new(ev).unwrap();
            assert_eq!(p.semantic_bytes(), bytes);
            assert_eq!(i16::from_be_bytes(p.semantic_bytes()), ev);
        }
        assert_eq!(ExposureSrgbV1::default(), ExposureSrgbV1::new(0).unwrap());
        assert_ne!(
            ExposureSrgbV1::new(-1).unwrap(),
            ExposureSrgbV1::new(1).unwrap()
        );
    }

    #[test]
    fn out_of_domain_intentions_are_refused_instead_of_clamped() {
        for ev in [i16::MIN, -801, 801, i16::MAX] {
            assert_eq!(
                ExposureSrgbV1::new(ev).unwrap_err().code,
                ErrorCode::InputSchema
            );
        }
    }
}
