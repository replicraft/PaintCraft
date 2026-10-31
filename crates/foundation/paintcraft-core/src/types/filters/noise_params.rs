//! Standalone coordinate-noise intent; no Descriptor or wire registration.
use super::{Error, ErrorCode, Result};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NoiseDistribution {
    #[default]
    Uniform = 0,
    Gaussian = 1,
}
impl NoiseDistribution {
    pub const fn tag(self) -> u8 {
        self as u8
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AddNoiseRgbV1 {
    amount_hundredths: u16,
    seed: u16,
    monochrome: bool,
    distribution: NoiseDistribution,
}
impl AddNoiseRgbV1 {
    pub fn new(
        amount_hundredths: i16,
        seed: u16,
        monochrome: bool,
        distribution: NoiseDistribution,
    ) -> Result<Self> {
        if !(0..=100).contains(&amount_hundredths) {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Noise amount requires hundredths 0..100",
            ));
        }
        Ok(Self {
            amount_hundredths: amount_hundredths as u16,
            seed,
            monochrome,
            distribution,
        })
    }
    pub const fn amount_hundredths(self) -> u16 {
        self.amount_hundredths
    }
    pub const fn seed(self) -> u16 {
        self.seed
    }
    pub const fn monochrome(self) -> bool {
        self.monochrome
    }
    pub const fn distribution(self) -> NoiseDistribution {
        self.distribution
    }
    /// Complete intent: amount u16BE, seed u16BE, monochrome, distribution.
    pub const fn semantic_bytes(self) -> [u8; 6] {
        let a = self.amount_hundredths.to_be_bytes();
        let s = self.seed.to_be_bytes();
        [
            a[0],
            a[1],
            s[0],
            s[1],
            self.monochrome as u8,
            self.distribution.tag(),
        ]
    }
}
impl Default for AddNoiseRgbV1 {
    fn default() -> Self {
        Self {
            amount_hundredths: 10,
            seed: 1,
            monochrome: true,
            distribution: NoiseDistribution::Uniform,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_typed_intent_defaults_boundaries_and_tags_are_exact() {
        assert_eq!(
            AddNoiseRgbV1::default().semantic_bytes(),
            [0, 10, 0, 1, 1, 0]
        );
        for a in [0, 1, 10, 100] {
            for seed in [0, 1, u16::MAX] {
                for mono in [false, true] {
                    for d in [NoiseDistribution::Uniform, NoiseDistribution::Gaussian] {
                        let p = AddNoiseRgbV1::new(a, seed, mono, d).unwrap();
                        assert_eq!(
                            (
                                p.amount_hundredths(),
                                p.seed(),
                                p.monochrome(),
                                p.distribution()
                            ),
                            (a as u16, seed, mono, d)
                        );
                        let s = seed.to_be_bytes();
                        assert_eq!(
                            p.semantic_bytes(),
                            [0, a as u8, s[0], s[1], mono as u8, d.tag()]
                        );
                    }
                }
            }
        }
        for a in [i16::MIN, -1, 101, i16::MAX] {
            assert_eq!(
                AddNoiseRgbV1::new(a, 0, false, NoiseDistribution::Uniform)
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema
            );
        }
    }
}
