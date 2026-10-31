use super::*;

fn add(a: i64, b: i64) -> Result<i64> {
    a.checked_add(b)
        .ok_or_else(|| resource("Vibrance integer addition overflow"))
}
fn sub(a: i64, b: i64) -> Result<i64> {
    a.checked_sub(b)
        .ok_or_else(|| resource("Vibrance integer subtraction overflow"))
}
fn mul(a: i64, b: i64) -> Result<i64> {
    a.checked_mul(b)
        .ok_or_else(|| resource("Vibrance integer multiplication overflow"))
}

pub(super) fn pixel(policy: VibranceRgbV1, incoming: Rgba) -> Result<Rgba> {
    let [r, g, b, alpha] = incoming.0;
    let hi = i64::from(r.max(g).max(b));
    let lo = i64::from(r.min(g).min(b));
    let amount = i64::from(policy.amount_hundredths());
    if amount == 0 || hi == lo {
        return Ok(incoming);
    }
    let sum = add(hi, lo)?;
    let delta = sub(hi, lo)?;
    // sum in0..510, so this checked absolute value cannot be i64::MIN.
    let centered = sub(sum, 255)?
        .checked_abs()
        .ok_or_else(|| resource("Vibrance integer absolute overflow"))?;
    let q = sub(255, centered)?;
    let denominator = mul(100, q)?;
    let factor = add(denominator, mul(amount, sub(q, delta)?)?)?;
    let center = mul(sum, denominator)?;
    let divisor = mul(2, denominator)?;
    // q>0 on this chromatic branch. For amount[-100,100], saturation remains
    // in[0,1] mathematically: no clamp/intermediate rounding is required.
    // Every intermediate's magnitude is<2^25; final numerator<=13,030,500.
    let channel = |v: u8| -> Result<u8> {
        let displacement = sub(mul(2, i64::from(v))?, sum)?;
        let numerator = add(center, mul(displacement, factor)?)?;
        let rounded = add(numerator, denominator)?
            .checked_div(divisor)
            .ok_or_else(|| resource("Vibrance integer division failed"))?;
        u8::try_from(rounded).map_err(|_| resource("Vibrance byte result outside proven bounds"))
    };
    // All candidates refer to the immutable original RGB tuple; alpha is exact.
    Ok(Rgba([channel(r)?, channel(g)?, channel(b)?, alpha]))
}
