//! Two RGBA pixels per widened vector; all channel arithmetic fits in u16.
use wide::{u8x16, u16x8};

const FULL: u16x8 = u16x8::splat(255);
const ROUND: u16x8 = u16x8::splat(128);

pub(super) struct Opaque {
    opacity: u16x8,
}
impl Opaque {
    pub(super) fn new(opacity: u8) -> Self {
        Self {
            opacity: u16x8::splat(u16::from(opacity)),
        }
    }
    pub(super) fn blend(&self, dst: &mut [u8; 16], src: &[u8; 16]) {
        let s = u8x16::new(*src);
        let d = u8x16::new(*dst);
        let alphas = s.shuffle(u8x16::new([
            3, 3, 3, 3, 7, 7, 7, 7, 11, 11, 11, 11, 15, 15, 15, 15,
        ]));
        let blend = |s: u16x8, d: u16x8, alpha: u16x8| {
            let alpha = round255(alpha * self.opacity);
            // Each weighted sum is <= 255 * (alpha + inverse) = 65025.
            round255(s * alpha + d * (FULL - alpha))
        };
        let low = blend(
            u16x8::from_u8x16_low(s),
            u16x8::from_u8x16_low(d),
            u16x8::from_u8x16_low(alphas),
        );
        let high = blend(
            u16x8::from_u8x16_high(s),
            u16x8::from_u8x16_high(d),
            u16x8::from_u8x16_high(alphas),
        );
        // Results are 0..255, so signed narrowing is exact. Destination alpha is 255.
        let result: u8x16 = u8x16::narrow_i16x8(low.cast_signed(), high.cast_signed())
            | u8x16::new([0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]);
        *dst = result.to_array();
    }
}

fn round255(n: u16x8) -> u16x8 {
    // n <= 65025, t <= 65153, final numerator <= 65407: no lane wraps.
    let t = n + ROUND;
    (t + (t >> 8)) >> 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u16_rounding_matches_integer_division_throughout_the_proven_domain() {
        for n in 0..=65025u16 {
            assert_eq!(round255(u16x8::splat(n)).to_array(), [(n + 127) / 255; 8]);
        }
    }
}
