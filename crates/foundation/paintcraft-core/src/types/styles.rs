//! Pure style policies. These are not numerical filter Descriptors.
use super::{Error, ErrorCode, Result};

/// The style blends against its own post-filter RGB, never the lower layers.
/// This separate type/domain grants none of LayerBlendMode's authorities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EffectBlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
}
impl EffectBlendMode {
    pub const fn code(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Multiply => 1,
            Self::Screen => 2,
        }
    }
    pub fn from_code(code: u8) -> Result<Self> {
        match code {
            0 => Ok(Self::Normal),
            1 => Ok(Self::Multiply),
            2 => Ok(Self::Screen),
            _ => Err(Error::new(
                ErrorCode::InputSchema,
                "Unknown effect blend mode",
            )),
        }
    }
}

/// Exact byte-opacity constant-color style. Alpha is not a color parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ColorOverlayRgbV1 {
    color: [u8; 3],
    opacity: u8,
    blend: EffectBlendMode,
}
impl ColorOverlayRgbV1 {
    pub const PROFILE: &'static str = "color-overlay-rgb-v1";
    pub const fn new(color: [u8; 3], opacity: u8, blend: EffectBlendMode) -> Self {
        Self {
            color,
            opacity,
            blend,
        }
    }
    pub const fn color(self) -> [u8; 3] {
        self.color
    }
    pub const fn opacity(self) -> u8 {
        self.opacity
    }
    pub const fn blend(self) -> EffectBlendMode {
        self.blend
    }
    /// Complete policy intent in this profile's separate style namespace.
    /// No default/identity normalization; opacity0 preserves color and mode.
    pub const fn semantic_bytes(self) -> [u8; 5] {
        [
            self.color[0],
            self.color[1],
            self.color[2],
            self.opacity,
            self.blend.code(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_effect_modes_and_complete_identity_intent() {
        for (code, mode) in [
            (0, EffectBlendMode::Normal),
            (1, EffectBlendMode::Multiply),
            (2, EffectBlendMode::Screen),
        ] {
            assert_eq!(EffectBlendMode::from_code(code).unwrap(), mode);
            assert_eq!(mode.code(), code);
            let p = ColorOverlayRgbV1::new([1, 127, 255], 0, mode);
            assert_eq!(p.color(), [1, 127, 255]);
            assert_eq!(p.opacity(), 0);
            assert_eq!(p.blend(), mode);
            assert_eq!(p.semantic_bytes(), [1, 127, 255, 0, code]);
        }
        for code in 3..=u8::MAX {
            assert_eq!(
                EffectBlendMode::from_code(code).unwrap_err().code,
                ErrorCode::InputSchema
            );
        }
        assert_ne!(
            ColorOverlayRgbV1::new([0; 3], 0, EffectBlendMode::Normal),
            ColorOverlayRgbV1::new([0; 3], 0, EffectBlendMode::Multiply)
        );
    }
}
