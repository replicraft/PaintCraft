//! Exact half-byte classification. No product oracle, floats, roots or tables.
use super::{Error, ErrorCode, ExposureSrgbV1, Result, integer, resource, word::Word};
use crate::exact::{Budget as ArithmeticBudget, Fault};
use integer::Integer;
#[derive(Debug)]
pub(super) struct Workspace<W: Word, const N: usize> {
    input_den: Integer<W, N>,
    cut_den: Integer<W, N>,
    temporary: Integer<W, N>,
    left: Integer<W, N>,
    right: Integer<W, N>,
    table: [u8; 256],
}
impl<W: Word, const N: usize> Default for Workspace<W, N> {
    fn default() -> Self {
        Self {
            input_den: Integer::zero(),
            cut_den: Integer::zero(),
            temporary: Integer::zero(),
            left: Integer::zero(),
            right: Integer::zero(),
            table: [0; 256],
        }
    }
}
fn gcd(mut a: u16, mut b: u16) -> u16 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
#[derive(Clone, Copy)]
struct Regime {
    input_low: bool,
    cut_low: bool,
    input_power: u16,
    cut_power: u16,
    shift: u16,
}
impl Regime {
    fn new(input_low: bool, cut_low: bool, stops: u16) -> Self {
        let common = if input_low && cut_low {
            100
        } else if !input_low && !cut_low {
            240
        } else {
            20
        };
        let d = gcd(common, stops);
        Self {
            input_low,
            cut_low,
            input_power: (if input_low { 100 } else { 240 }) / d,
            cut_power: (if cut_low { 100 } else { 240 }) / d,
            shift: stops / d,
        }
    }
    fn pair(self) -> (bool, bool) {
        (self.input_low, self.cut_low)
    }
    fn mixed(self) -> bool {
        self.input_low != self.cut_low
    }
    fn input(self, v: u16) -> u16 {
        if self.input_low {
            if self.cut_low { 2 * v } else { 5 * v }
        } else {
            40 * v + 561
        }
    }
    fn cut(self, j: u16) -> u16 {
        if self.cut_low {
            if self.input_low {
                2 * j + 1
            } else {
                5 * (2 * j + 1)
            }
        } else {
            40 * j + 581
        }
    }
}
/// All paths use at most516 powers and514 products/shifts/comparisons:255
/// nonzero inputs+255 cuts, two regime transitions with at most four reloads,
/// and two fixed denominator powers in the sole possible mixed regime.
/// LL -> LN -> NN or LL -> NL -> NN; both coordinates are monotone.
///
/// Reduced bases fit u16. Unreduced exponents <=240/100 and EV shift <=800
/// bound mixed products by14*240+16*100+800=5760bits. Rounded operand word
/// lengths give180 u32 or91 u64 limbs, below the retained10240-bit capacity.
/// Quotes may overprice impossible pairings; they never require a LUT build.
pub(super) fn quote_work<W: Word>(p: ExposureSrgbV1) -> u64 {
    let stops = p.stops_hundredths().unsigned_abs();
    if stops == 0 {
        return 512;
    }
    let d = gcd(20, stops);
    let power = 240 / d;
    let shift = stops / d;
    let words = integer::words::<W>(32946, power);
    let capacity = integer::words::<W>(10761, power)
        + integer::words::<W>(32946, 100 / d)
        + u64::from(shift).div_ceil(u64::from(W::BITS));
    let shifted = 1 + u64::from(shift).div_ceil(u64::from(W::BITS - 1)) * (capacity + 2);
    4096 + 516 * integer::power_quote::<W>(32946, power)
        + 514 * (integer::product_quote(words, words) + shifted + 1 + capacity)
}
pub(super) struct Budget<'a> {
    pub(super) used: u64,
    pub(super) limit: u64,
    pub(super) check: &'a dyn Fn() -> Result<()>,
}
impl ArithmeticBudget for Budget<'_> {
    fn charge(&mut self, units: u64) -> Result<()> {
        (self.check)()?;
        self.used = self
            .used
            .checked_add(units)
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| resource("Exposure work allowance"))?;
        Ok(())
    }
    fn resource(&self, fault: Fault) -> Error {
        resource(match fault {
            Fault::ScalarPower => "Exposure scalar power capacity",
            Fault::IntegerCapacity => "Exposure integer capacity",
            Fault::ProductCapacity => "Exposure product capacity",
        })
    }
}
fn load_input<W: Word, const N: usize>(
    r: Regime,
    v: u16,
    positive: bool,
    s: &mut Workspace<W, N>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    if r.mixed() {
        s.temporary.pow(r.input(v), r.input_power, budget)?;
        s.left.product(&s.temporary, &s.cut_den, budget)?;
    } else {
        s.left.pow(r.input(v), r.input_power, budget)?;
    }
    if positive {
        s.left.scale_pow2(r.shift, budget)?;
    }
    Ok(())
}
fn load_cut<W: Word, const N: usize>(
    r: Regime,
    j: u16,
    positive: bool,
    s: &mut Workspace<W, N>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    if r.mixed() {
        s.temporary.pow(r.cut(j), r.cut_power, budget)?;
        s.right.product(&s.input_den, &s.temporary, budget)?;
    } else {
        s.right.pow(r.cut(j), r.cut_power, budget)?;
    }
    if !positive {
        s.right.scale_pow2(r.shift, budget)?;
    }
    Ok(())
}
/// Input low branch v<=10; half-byte low branch j<=9. None of these cuts lies
/// in the small standardized encode/decode-knee discrepancy. Equality crosses
/// upward; all255 crossed cuts also implement the clamp at one exactly.
/// Only scalar locals accompany caller-owned large integers/private LUT.
pub(super) fn build_lut<W: Word, const N: usize>(
    p: ExposureSrgbV1,
    s: &mut Workspace<W, N>,
    out: &mut [u8],
    scratch_bytes: u64,
    work_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<u64> {
    if out.len() != 256 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Exposure LUT requires256 output bytes",
        ));
    }
    if scratch_bytes < (std::mem::size_of::<Workspace<W, N>>() as u64)
        || work_limit < quote_work::<W>(p)
    {
        return Err(resource("Exposure scratch/work preflight"));
    }
    check()?;
    let mut budget = Budget {
        used: 0,
        limit: work_limit,
        check,
    };
    let stops = p.stops_hundredths();
    if stops == 0 {
        for (v, out) in s.table.iter_mut().enumerate() {
            budget.charge(1)?;
            *out = v as u8;
        }
    } else {
        let mut regime: Option<Regime> = None;
        let mut cut = 0u16;
        let mut cut_loaded = false;
        for v in 0..256u16 {
            budget.charge(1)?;
            if v == 0 {
                s.table[0] = 0;
                continue;
            }
            let mut input_loaded = false;
            while cut < 255 {
                let pair = (v <= 10, cut <= 9);
                if regime.map(Regime::pair) != Some(pair) {
                    budget.charge(1)?;
                    let r = Regime::new(pair.0, pair.1, stops.unsigned_abs());
                    if r.mixed() {
                        s.input_den.pow(
                            if r.input_low { 16473 } else { 10761 },
                            r.input_power,
                            &mut budget,
                        )?;
                        s.cut_den.pow(
                            if r.cut_low { 32946 } else { 10761 },
                            r.cut_power,
                            &mut budget,
                        )?;
                    }
                    regime = Some(r);
                    input_loaded = false;
                    cut_loaded = false;
                }
                // Regime is installed above before either side is used.
                let r = regime.ok_or_else(|| resource("Exposure regime invariant"))?;
                if !input_loaded {
                    load_input(r, v, stops > 0, s, &mut budget)?;
                    input_loaded = true;
                }
                if !cut_loaded {
                    load_cut(r, cut, stops > 0, s, &mut budget)?;
                    cut_loaded = true;
                }
                if s.left.compare(&s.right, &mut budget)?.is_lt() {
                    break;
                }
                cut += 1;
                cut_loaded = false;
            }
            s.table[usize::from(v)] = cut as u8;
        }
    }
    budget.charge(256)?;
    out.copy_from_slice(&s.table);
    Ok(budget.used)
}
