//! Pinned whole-tuple policy. The Gaussian expression association is normative.
use super::*;
const D: i64 = 1i64 << 32;
const TAU: f64 = f64::from_bits(0x4019_21fb_5444_2d18);
fn initial(point: SourcePixel, seed: u16, channel: u32) -> u32 {
    0xa511_e9b3
        ^ point.x().wrapping_mul(0x9e37_79b9)
        ^ point.y().wrapping_mul(0x85eb_ca6b)
        ^ u32::from(seed).wrapping_mul(0xc2b2_ae35)
        ^ channel.wrapping_mul(0x27d4_eb2d)
}
fn mix(mut word: u32) -> u32 {
    word ^= word >> 16;
    word = word.wrapping_mul(0x7feb_352d);
    word ^= word >> 15;
    word = word.wrapping_mul(0x846c_a68b);
    word ^= word >> 16;
    word
}
fn centered(point: SourcePixel, seed: u16, channel: u32) -> i64 {
    // i64 bound: |2*word+1-D|<=D-1.
    2 * i64::from(mix(initial(point, seed, channel))) + 1 - D
}
fn uniform(p: u8, amount: u16, k: i64) -> Result<u8> {
    let denominator = 100 * D;
    let center = i64::from(p).checked_mul(denominator);
    let delta = 255i64
        .checked_mul(i64::from(amount))
        .and_then(|n| n.checked_mul(k));
    let n = center
        .and_then(|a| delta.and_then(|b| a.checked_add(b)))
        .ok_or_else(|| resource("Uniform noise integer capacity exceeded"))?;
    let n = n.clamp(0, 255 * denominator);
    let rounded = n
        .checked_add(denominator / 2)
        .ok_or_else(|| resource("Uniform noise rounding capacity exceeded"))?
        / denominator;
    u8::try_from(rounded).map_err(|_| resource("Uniform noise byte conversion failed"))
}
fn gaussian(point: SourcePixel, seed: u16, channel: u32) -> f64 {
    let w1 = mix(initial(point, seed, channel));
    let w2 = mix(w1);
    // Exact dyadics; no f32 cast, OS math, FMA or mutable RNG state.
    let u1 = (f64::from(w1) + 0.5) * (1.0 / 4294967296.0);
    let u2 = (f64::from(w2) + 0.5) * (1.0 / 4294967296.0);
    let l = libm::log(u1);
    let r = libm::generic::sqrt::<f64>((-2.0) * l);
    let theta = TAU * u2;
    r * libm::cos(theta)
}
pub(super) fn pixel(policy: AddNoiseRgbV1, point: SourcePixel, incoming: Rgba) -> Result<Rgba> {
    if policy.amount_hundredths() == 0 {
        return Ok(incoming);
    }
    let mut output = incoming;
    match policy.distribution() {
        NoiseDistribution::Uniform => {
            let mono = policy
                .monochrome()
                .then(|| centered(point, policy.seed(), 0));
            for c in 0..3 {
                let k = mono.unwrap_or_else(|| centered(point, policy.seed(), c as u32));
                output.0[c] = uniform(incoming.0[c], policy.amount_hundredths(), k)?;
            }
        }
        NoiseDistribution::Gaussian => {
            let mono = policy
                .monochrome()
                .then(|| gaussian(point, policy.seed(), 0));
            let amount = f64::from(policy.amount_hundredths()) / 100.0;
            for c in 0..3 {
                let z = mono.unwrap_or_else(|| gaussian(point, policy.seed(), c as u32));
                let delta = (amount * z) * 255.0;
                let v = f64::from(incoming.0[c]) + delta;
                if !v.is_finite() {
                    return Err(resource("Gaussian noise produced a nonfinite candidate"));
                }
                output.0[c] = libm::round(v.clamp(0.0, 255.0)) as u8;
            }
        }
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapping_field_draws_are_ordered_and_not_tile_relative() {
        let p = SourcePixel::new(u32::MAX, u32::MAX);
        let w1 = mix(initial(p, u16::MAX, 2));
        let w2 = mix(w1);
        assert_ne!(w1, w2);
        assert_eq!(centered(p, u16::MAX, 2), 2 * i64::from(w1) + 1 - D);
        assert_ne!(centered(p, 1, 0), centered(SourcePixel::new(0, 0), 1, 0));
    }
    #[test]
    fn byte_round_target_distinguishes_prev_half_and_exact_half() {
        let prev = f64::from_bits(0.5f64.to_bits() - 1);
        assert_eq!(libm::round(prev), 0.0);
        assert_eq!(libm::round(0.5), 1.0);
        assert_eq!((prev + 0.5) as u16, 1);
    }
}
