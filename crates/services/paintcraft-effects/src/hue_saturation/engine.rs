use super::*;
fn add(a: i64, b: i64) -> Result<i64> {
    a.checked_add(b)
        .ok_or_else(|| resource("Hue/Saturation integer addition overflow"))
}
fn sub(a: i64, b: i64) -> Result<i64> {
    a.checked_sub(b)
        .ok_or_else(|| resource("Hue/Saturation integer subtraction overflow"))
}
fn mul(a: i64, b: i64) -> Result<i64> {
    a.checked_mul(b)
        .ok_or_else(|| resource("Hue/Saturation integer multiplication overflow"))
}
fn abs(a: i64) -> Result<i64> {
    a.checked_abs()
        .ok_or_else(|| resource("Hue/Saturation integer absolute overflow"))
}
fn byte(n: i64, d: i64) -> Result<u8> {
    let x = add(n, d / 2)?
        .checked_div(d)
        .ok_or_else(|| resource("Hue/Saturation integer rounding failed"))?;
    u8::try_from(x).map_err(|_| resource("Hue/Saturation byte outside proven bounds"))
}
pub(super) fn pixel(policy: HueSaturationRgbV1, incoming: Rgba) -> Result<Rgba> {
    let h = i64::from(policy.hue_degrees());
    let s = i64::from(policy.saturation_hundredths());
    let l = i64::from(policy.lightness_hundredths());
    if h == 0 && s == 0 && l == 0 {
        return Ok(incoming);
    }
    let [r, g, b, a] = incoming.0;
    let r = i64::from(r);
    let g = i64::from(g);
    let b = i64::from(b);
    let hi = r.max(g).max(b);
    let lo = r.min(g).min(b);
    let total = add(hi, lo)?;
    let delta = sub(hi, lo)?;
    let u = add(mul(50, total)?, mul(255, l)?)?.clamp(0, 25500);
    if delta == 0 || u == 0 || u == 25500 {
        let c = byte(u, 100)?;
        return Ok(Rgba([c, c, c, a]));
    }
    let q = sub(255, abs(sub(total, 255)?)?)?;
    let ds = mul(100, q)?;
    let ns = mul(delta, add(100, s)?)?.min(ds);
    // R/G/B priority matches the exact source hue; ties give the same hue.
    let n = if hi == r {
        sub(g, b)?
    } else if hi == g {
        add(sub(b, r)?, mul(2, delta)?)?
    } else {
        add(sub(r, g)?, mul(4, delta)?)?
    };
    let dh = mul(60, delta)?;
    // Euclidean wrap is essential for negative hue; positive divisor is proven.
    let w = add(mul(60, n)?, mul(h, delta)?)?
        .checked_rem_euclid(mul(6, dh)?)
        .ok_or_else(|| resource("Hue/Saturation integer hue wrap failed"))?;
    let k = w
        .checked_div(dh)
        .ok_or_else(|| resource("Hue/Saturation hue division failed"))?;
    let f = w
        .checked_rem(dh)
        .ok_or_else(|| resource("Hue/Saturation hue remainder failed"))?;
    let z = if k % 2 == 0 { f } else { sub(dh, f)? };
    let p = match k {
        0 => [dh, z, 0],
        1 => [z, dh, 0],
        2 => [0, dh, z],
        3 => [0, z, dh],
        4 => [z, 0, dh],
        5 => [dh, 0, z],
        _ => return Err(resource("Hue/Saturation sector outside proven bounds")),
    };
    let m = sub(25500, abs(sub(mul(2, u)?, 25500)?)?)?;
    let denominator = mul(mul(200, ds)?, dh)?;
    let center = mul(mul(mul(2, u)?, ds)?, dh)?;
    let chroma = mul(m, ns)?;
    // All ordered intermediates fit<2^45; final N is0..255*denominator.
    // No HSL/channel intermediate is rounded; one ties-up byte division follows.
    let channel = |coefficient| {
        byte(
            add(center, mul(chroma, sub(mul(2, coefficient)?, dh)?)?)?,
            denominator,
        )
    };
    Ok(Rgba([channel(p[0])?, channel(p[1])?, channel(p[2])?, a]))
}
