//! Complete exact Levels LUT foundation, not a registered filter descriptor.
//! Successful LUT arithmetic/storage uses caller buffers, no heap or powf.
//! Caller callbacks and ordinary allocating typed-error diagnostics are excluded
//! from that storage claim; no shared error API or product parser is changed.
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba, filters::LevelsRgbV1};
mod integer;
use integer::Integer;

/// Caller-owned scratch includes every multiword value and the private LUT.
/// On failure scratch is unspecified, while the caller's output stays intact.
#[derive(Debug)]
pub struct Scratch {
    denominator: Integer,
    base: Integer,
    temporary: Integer,
    left: Integer,
    right: Integer,
    table: [u8; 256],
}
impl Default for Scratch {
    fn default() -> Self {
        Self {
            denominator: Integer::zero(),
            base: Integer::zero(),
            temporary: Integer::zero(),
            left: Integer::zero(),
            right: Integer::zero(),
            table: [0; 256],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub scratch_bytes: u64,
    pub work: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quote {
    pub scratch_bytes: u64,
    pub work: u64,
}
impl From<Quote> for Limits {
    fn from(value: Quote) -> Self {
        Self {
            scratch_bytes: value.scratch_bytes,
            work: value.work,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkUsed {
    pub units: u64,
}
fn resource(message: &'static str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn gcd(mut a: u16, mut b: u16) -> u16 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
fn exponents(p: LevelsRgbV1) -> (u16, u16) {
    let d = gcd(100, p.gamma_hundredths());
    (100 / d, p.gamma_hundredths() / d)
}
/// Conservative units: word multiply/compare/clear, or one complete bounded byte
/// interpolation. This excludes callback time and is not a CPU/RSS/time bound.
/// All factors are capped by the validated byte/gamma domains (<80 million).
/// Scratch is actual platform size, not hidden kernel stack copies or a realm.
pub fn quote(p: LevelsRgbV1) -> Quote {
    let scratch_bytes = std::mem::size_of::<Scratch>() as u64;
    let width = u16::from(p.input_white()) - u16::from(p.input_black());
    let difference = u16::from(p.output_white().abs_diff(p.output_black()));
    if p.gamma_hundredths() == 100 || width == 1 || difference == 0 {
        return Quote {
            scratch_bytes,
            work: 512,
        };
    }
    let (a, b) = exponents(p);
    let denominator = integer::words(width, a);
    let input = integer::words(width - 1, a);
    let threshold = integer::words(2 * difference - 1, b);
    let base = integer::words(2 * difference, b);
    let products = integer::product_quote(input, base) * u64::from(width - 1)
        + integer::product_quote(denominator, threshold) * u64::from(difference);
    let powers = integer::power_quote(width, a)
        + integer::power_quote(2 * difference, b)
        + integer::power_quote(width - 1, a) * u64::from(width - 1)
        + integer::power_quote(2 * difference - 1, b) * u64::from(difference);
    let compares = (u64::from(width - 1) + u64::from(difference))
        * (1 + input.max(denominator) + base.max(threshold));
    Quote {
        scratch_bytes,
        work: 512 + products + powers + compares,
    }
}
struct Budget<'a> {
    used: u64,
    limit: u64,
    check: &'a dyn Fn() -> Result<()>,
}
impl Budget<'_> {
    fn charge(&mut self, units: u64) -> Result<()> {
        (self.check)()?;
        self.used = self
            .used
            .checked_add(units)
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| resource("Levels work allowance"))?;
        Ok(())
    }
}
fn linear(p: LevelsRgbV1, v: u8) -> u8 {
    let d = u32::from(p.input_white() - p.input_black());
    let q = u32::from(v.saturating_sub(p.input_black())).min(d);
    let n = (d - q) * u32::from(p.output_black()) + q * u32::from(p.output_white());
    ((2 * n + d) / (2 * d)) as u8
}
/// Build all256 entries into private scratch, then publish with one infallible
/// 256-byte copy after the final check. Wrong output geometry, resource refusal,
/// integer overflow or cancellation never publishes a partial LUT. Safe Rust
/// disjoint borrows are required; unsafe-created aliases are unsupported.
///
/// For nonlinear input, compare q^p*(2D)^r against d^p*(2j+1)^r. Each input
/// power and each crossed threshold power is computed only once. Reversed
/// output ranges cross STRICTLY above the half threshold: subtracting an
/// independently rounded loss would incorrectly round nonneutral ties down.
pub fn build_lut(
    p: LevelsRgbV1,
    scratch: &mut Scratch,
    output: &mut [u8],
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<WorkUsed> {
    if output.len() != 256 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Levels LUT requires256 output bytes",
        ));
    }
    let required = quote(p);
    if limits.scratch_bytes < required.scratch_bytes || limits.work < required.work {
        return Err(resource("Levels scratch/work preflight"));
    }
    let mut budget = Budget {
        used: 0,
        limit: limits.work,
        check,
    };
    check()?;
    let width = u16::from(p.input_white() - p.input_black());
    let difference = u16::from(p.output_white().abs_diff(p.output_black()));
    if p.gamma_hundredths() == 100 || width == 1 || difference == 0 {
        for (v, out) in scratch.table.iter_mut().enumerate() {
            budget.charge(1)?;
            *out = linear(p, v as u8);
        }
    } else {
        let (power, gamma) = exponents(p);
        scratch.denominator.pow(width, power, &mut budget)?;
        scratch.base.pow(2 * difference, gamma, &mut budget)?;
        let mut threshold = 0u16;
        let mut loaded = false;
        let inverted = p.output_white() < p.output_black();
        for (v, out) in scratch.table.iter_mut().enumerate() {
            budget.charge(1)?;
            let q = (v as u16)
                .saturating_sub(u16::from(p.input_black()))
                .min(width);
            if q == 0 {
                *out = p.output_black();
                continue;
            }
            if q == width {
                *out = p.output_white();
                continue;
            }
            scratch.temporary.pow(q, power, &mut budget)?;
            scratch
                .left
                .product(&scratch.temporary, &scratch.base, &mut budget)?;
            while threshold < difference {
                if !loaded {
                    scratch
                        .temporary
                        .pow(2 * threshold + 1, gamma, &mut budget)?;
                    scratch
                        .right
                        .product(&scratch.denominator, &scratch.temporary, &mut budget)?;
                    loaded = true;
                }
                let comparison = scratch.left.compare(&scratch.right, &mut budget)?;
                if comparison.is_lt() || (inverted && comparison.is_eq()) {
                    break;
                }
                threshold += 1;
                loaded = false;
            }
            *out = if inverted {
                p.output_black() - threshold as u8
            } else {
                p.output_black() + threshold as u8
            };
        }
    }
    // One bounded publication with no fallible operation after the copy starts.
    budget.charge(256)?;
    output.copy_from_slice(&scratch.table);
    Ok(WorkUsed { units: budget.used })
}
/// Table lookup preserves alpha and transforms RGB hidden under alpha zero.
/// This standalone helper grants no Descriptor, document or wire capability.
pub fn evaluate_pixel(table: &[u8; 256], pixel: Rgba) -> Rgba {
    let [r, g, b, a] = pixel.0;
    Rgba([
        table[usize::from(r)],
        table[usize::from(g)],
        table[usize::from(b)],
        a,
    ])
}
#[cfg(test)]
mod oracles;
#[cfg(test)]
mod tests;
