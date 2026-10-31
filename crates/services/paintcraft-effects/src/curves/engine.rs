//! Checked Hermite basis in the normative multiplication/left-sum order.
use super::*;
const ENTRY_WORK: u64 = 128;
pub(super) const TABLE_WORK: u64 = ENTRY_WORK * 256;
fn overflow() -> Error {
    resource("Curves checked arithmetic capacity")
}
fn add(a: i64, b: i64) -> Result<i64> {
    a.checked_add(b).ok_or_else(overflow)
}
fn sub(a: i64, b: i64) -> Result<i64> {
    a.checked_sub(b).ok_or_else(overflow)
}
fn mul(a: i64, b: i64) -> Result<i64> {
    a.checked_mul(b).ok_or_else(overflow)
}
fn sample(outputs: [u8; 5], input: u8) -> Result<u8> {
    // Fixed geometry avoids movable knots or any parameter-dependent index.
    // The last span is63; the adjacent centered secant denominator is127.
    let (left, right, previous, next, x0, d, dl, dr): (
        usize,
        usize,
        usize,
        usize,
        i64,
        i64,
        i64,
        i64,
    ) = match input {
        0..=63 => (0, 1, 0, 2, 0, 64, 64, 128),
        64..=127 => (1, 2, 0, 3, 64, 64, 128, 128),
        128..=191 => (2, 3, 1, 4, 128, 64, 128, 127),
        _ => (3, 4, 2, 4, 192, 63, 127, 63),
    };
    let y_l = i64::from(outputs[left]);
    let y_r = i64::from(outputs[right]);
    // Widen first: decreasing/free endpoint output values are valid.
    let delta_l = sub(y_r, i64::from(outputs[previous]))?;
    let delta_r = sub(i64::from(outputs[next]), y_l)?;
    let n = sub(i64::from(input), x0)?;
    let n2 = mul(n, n)?;
    let n3 = mul(n2, n)?;
    let d2 = mul(d, d)?;
    let d3 = mul(d2, d)?;
    let h00 = add(sub(mul(2, n3)?, mul(mul(3, n2)?, d)?)?, d3)?;
    let h10 = add(sub(n3, mul(mul(2, n2)?, d)?)?, mul(n, d2)?)?;
    let h01 = add(mul(-2, n3)?, mul(mul(3, n2)?, d)?)?;
    let h11 = sub(n3, mul(n2, d)?)?;
    let denominator = mul(mul(d3, dl)?, dr)?;
    let first = mul(mul(mul(h00, y_l)?, dl)?, dr)?;
    let second = mul(mul(mul(h10, d)?, delta_l)?, dr)?;
    let third = mul(mul(mul(h01, y_r)?, dl)?, dr)?;
    let fourth = mul(mul(mul(h11, d)?, delta_r)?, dl)?;
    let numerator = add(add(add(first, second)?, third)?, fourth)?;
    let upper = mul(255, denominator)?;
    let clamped = if numerator < 0 {
        0
    } else if numerator > upper {
        upper
    } else {
        numerator
    };
    let rounded = add(mul(2, clamped)?, denominator)?
        .checked_div(mul(2, denominator)?)
        .ok_or_else(overflow)?;
    u8::try_from(rounded).map_err(|_| overflow())
}
pub(super) fn build(
    parameters: CurvesRgbV1,
    scratch: &mut Scratch,
    output: &mut [u8],
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<u64> {
    if output.len() != 256 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Curves LUT output must contain exactly256 bytes",
        ));
    }
    let required = quote(parameters);
    if limits.scratch_bytes < required.scratch_bytes || limits.work < required.work {
        return Err(resource("Curves scratch/work allowance"));
    }
    check()?;
    let outputs = parameters.outputs();
    let mut used = 0u64;
    for (input, entry) in (0..=u8::MAX).zip(scratch.table.iter_mut()) {
        check()?;
        //128 modeled units cover<=48 arithmetic/clamp/narrowing actions plus
        //<=32 explicit knot-index/widen/selection/budget/loop/store actions.
        // A checked operation includes its typed failure branch in this model;
        // this is not a count of Option/Result internals, machine instructions,
        // elapsed CPU cycles or compiler-specific primitive operations.
        used = used
            .checked_add(ENTRY_WORK)
            .filter(|n| *n <= limits.work)
            .ok_or_else(|| resource("Curves entry work allowance"))?;
        *entry = sample(outputs, input)?;
    }
    check()?;
    output.copy_from_slice(&scratch.table);
    Ok(used)
}
