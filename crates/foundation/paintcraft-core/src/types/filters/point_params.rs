//! Validated integer parameters for the encoded-RGB point profiles.
use super::*;
macro_rules! parameter {
    ($name:ident,$getter:ident,$low:literal,$high:literal,$default:literal) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name(u8);
        impl $name {
            pub fn new(value: i16) -> Result<Self> {
                if !($low..=$high).contains(&value) {
                    return Err(Error::new(
                        ErrorCode::InputSchema,
                        concat!(
                            stringify!($getter),
                            " is outside the versioned point-profile range"
                        ),
                    ));
                }
                Ok(Self(value as u8))
            }
            pub const fn $getter(self) -> u8 {
                self.0
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self($default)
            }
        }
    };
}
parameter!(BlackWhiteV1, strength, 0, 100, 100);
parameter!(ThresholdLumaV1, level, 0, 255, 128);
parameter!(PosterizeRgbV1, levels, 2, 32, 6);
