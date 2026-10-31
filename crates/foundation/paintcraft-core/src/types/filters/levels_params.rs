//! Complete numerical Levels policy, constructed through the explicit catalog boundary.
use super::{Error, ErrorCode, Result};

/// Encoded RGB channel-independent Levels, preserving alpha and transforming
/// hidden RGB. Gamma is the exact rational `gamma_hundredths / 100`.
///
/// A future closed wire object must require all five integer fields. UI defaults
/// never authorize missing fields, float coercion or silently discarded gamma.
/// This type alone adds no algorithm to Descriptor, persistence or automation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LevelsRgbV1 {
    input_black: u8,
    input_white: u8,
    gamma_hundredths: u16,
    output_black: u8,
    output_white: u8,
}

impl LevelsRgbV1 {
    pub fn new(
        input_black: i16,
        input_white: i16,
        gamma_hundredths: i16,
        output_black: i16,
        output_white: i16,
    ) -> Result<Self> {
        if !(0..=254).contains(&input_black)
            || !(1..=255).contains(&input_white)
            || input_black >= input_white
            || !(10..=999).contains(&gamma_hundredths)
            || !(0..=255).contains(&output_black)
            || !(0..=255).contains(&output_white)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Levels requires ordered input endpoints, gamma hundredths 10..999 and byte output endpoints",
            ));
        }
        Ok(Self {
            input_black: input_black as u8,
            input_white: input_white as u8,
            gamma_hundredths: gamma_hundredths as u16,
            output_black: output_black as u8,
            output_white: output_white as u8,
        })
    }

    pub const fn input_black(self) -> u8 {
        self.input_black
    }
    pub const fn input_white(self) -> u8 {
        self.input_white
    }
    pub const fn gamma_hundredths(self) -> u16 {
        self.gamma_hundredths
    }
    pub const fn output_black(self) -> u8 {
        self.output_black
    }
    pub const fn output_white(self) -> u8 {
        self.output_white
    }

    /// Canonical numerical intent, independent of transport codec or LUT bytes.
    /// Field order is input black, input white, big-endian gamma hundredths,
    /// output black, output white. This grants no document or format capability.
    pub const fn semantic_bytes(self) -> [u8; 6] {
        let gamma = self.gamma_hundredths.to_be_bytes();
        [
            self.input_black,
            self.input_white,
            gamma[0],
            gamma[1],
            self.output_black,
            self.output_white,
        ]
    }
}

impl Default for LevelsRgbV1 {
    fn default() -> Self {
        Self {
            input_black: 0,
            input_white: 255,
            gamma_hundredths: 100,
            output_black: 0,
            output_white: 255,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_identity_retains_every_parameter_without_lut_collapse() {
        assert_eq!(
            LevelsRgbV1::new(17, 201, 999, 240, 16)
                .unwrap()
                .semantic_bytes(),
            [17, 201, 3, 231, 240, 16]
        );
        assert_eq!(
            LevelsRgbV1::default().semantic_bytes(),
            [0, 255, 0, 100, 0, 255]
        );
        // Input/gamma changes share the same constant LUT here; endpoint
        // changes alter it. Both retain their complete editable intent.
        let base = LevelsRgbV1::new(0, 255, 100, 73, 73).unwrap();
        for alternate in [
            LevelsRgbV1::new(1, 255, 100, 73, 73).unwrap(),
            LevelsRgbV1::new(0, 254, 100, 73, 73).unwrap(),
            LevelsRgbV1::new(0, 255, 999, 73, 73).unwrap(),
            LevelsRgbV1::new(0, 255, 100, 74, 73).unwrap(),
            LevelsRgbV1::new(0, 255, 100, 73, 74).unwrap(),
        ] {
            assert_ne!(base.semantic_bytes(), alternate.semantic_bytes());
        }
    }

    #[test]
    fn every_input_interval_and_gamma_boundary_retains_exact_user_intent() {
        for black in 0..=254 {
            for white in black + 1..=255 {
                let p = LevelsRgbV1::new(black, white, 100, 255, 0).unwrap();
                assert_eq!(
                    (i16::from(p.input_black()), i16::from(p.input_white())),
                    (black, white)
                );
                assert_eq!((p.output_black(), p.output_white()), (255, 0));
            }
        }
        for gamma in 10..=999 {
            let p = LevelsRgbV1::new(0, 255, gamma, 73, 73).unwrap();
            assert_eq!(p.gamma_hundredths(), gamma as u16);
            assert_eq!((p.output_black(), p.output_white()), (73, 73));
        }
        assert_eq!(
            LevelsRgbV1::default(),
            LevelsRgbV1::new(0, 255, 100, 0, 255).unwrap()
        );
    }

    #[test]
    fn invalid_fields_and_cross_field_order_are_refused_without_coercion() {
        for values in [
            [-1, 255, 100, 0, 255],
            [255, 255, 100, 0, 255],
            [0, 0, 100, 0, 255],
            [0, 256, 100, 0, 255],
            [70, 70, 100, 0, 255],
            [71, 70, 100, 0, 255],
            [0, 255, 9, 0, 255],
            [0, 255, 1000, 0, 255],
            [0, 255, 100, -1, 255],
            [0, 255, 100, 256, 255],
            [0, 255, 100, 0, -1],
            [0, 255, 100, 0, 256],
        ] {
            let [ib, iw, g, ob, ow] = values;
            assert_eq!(
                LevelsRgbV1::new(ib, iw, g, ob, ow).unwrap_err().code,
                ErrorCode::InputSchema
            );
        }
        for index in 0..5 {
            for outside in [i16::MIN, i16::MAX] {
                let mut values = [0, 255, 100, 0, 255];
                values[index] = outside;
                let [ib, iw, g, ob, ow] = values;
                assert_eq!(
                    LevelsRgbV1::new(ib, iw, g, ob, ow).unwrap_err().code,
                    ErrorCode::InputSchema
                );
            }
        }
    }

    #[test]
    fn legacy_descriptor_grammars_remain_closed() {
        use crate::types::{filters::Descriptor, json::Value};
        assert_eq!(
            Descriptor::checked("levels-rgb-v1", None, None)
                .unwrap_err()
                .code,
            ErrorCode::CapabilityUnsupported
        );
        let full_intent = Value::object([
            ("kind", Value::str("levels-rgb-v1")),
            ("input_black", Value::Int(0)),
            ("input_white", Value::Int(255)),
            ("gamma_hundredths", Value::Int(100)),
            ("output_black", Value::Int(0)),
            ("output_white", Value::Int(255)),
        ]);
        assert_eq!(
            Descriptor::from_value(&full_intent).unwrap_err().code,
            ErrorCode::InputSchema,
            "The strict closed parser retains its existing unknown-kind error"
        );
    }
}
