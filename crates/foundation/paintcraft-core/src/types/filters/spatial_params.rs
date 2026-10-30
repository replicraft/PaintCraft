//! Validated spatial-kernel policies. These are not yet persistent descriptors.
use super::{Error, ErrorCode, Result};

/// Exact square encoded-RGB, premultiplied-alpha box blur, with true finite
/// source-edge clamping. Radius zero is exact straight-RGBA identity.
/// This policy is independently specified, not Palette byte-output parity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BoxBlurV1(u8);

impl BoxBlurV1 {
    pub fn new(radius: i16) -> Result<Self> {
        if !(0..=32).contains(&radius) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Box blur radius must be 0..32",
            ));
        }
        Ok(Self(radius as u8))
    }

    pub const fn radius(self) -> u8 {
        self.0
    }
}

impl Default for BoxBlurV1 {
    fn default() -> Self {
        Self(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radius_domain_is_closed_and_integer() {
        assert_eq!(BoxBlurV1::default().radius(), 2);
        for radius in 0..=32 {
            assert_eq!(i16::from(BoxBlurV1::new(radius).unwrap().radius()), radius);
        }
        for radius in [i16::MIN, -1, 33, i16::MAX] {
            assert_eq!(
                BoxBlurV1::new(radius).unwrap_err().code,
                ErrorCode::InputSchema
            );
        }
    }
}
