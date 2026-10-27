use super::*;
/// Each mip is sampled directly from the immutable base, never recursively.
/// Box weights are exact integer overlap areas; rounding is nearest, ties up.
pub(super) fn generate(
    base: &[u8],
    width: u32,
    height: u32,
    target: &MipLevel,
    filter: MipFilter,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    let length = u64::from(target.width) * u64::from(target.height) * 4;
    let mut out = Vec::new();
    out.try_reserve_exact(length as usize)
        .map_err(|_| resource("DDS mip allocation"))?;
    for y in 0..target.height {
        check()?;
        for x in 0..target.width {
            if x.is_multiple_of(128) {
                check()?;
            }
            if filter == MipFilter::Nearest {
                let sx = ((2 * u64::from(x) + 1) * u64::from(width) / (2 * u64::from(target.width)))
                    as usize;
                let sy = ((2 * u64::from(y) + 1) * u64::from(height)
                    / (2 * u64::from(target.height))) as usize;
                let offset = (sy * width as usize + sx) * 4;
                out.extend_from_slice(&base[offset..offset + 4]);
                continue;
            }
            let (dw, dh) = (u64::from(target.width), u64::from(target.height));
            let x0 = u64::from(x) * u64::from(width);
            let x1 = (u64::from(x) + 1) * u64::from(width);
            let y0 = u64::from(y) * u64::from(height);
            let y1 = (u64::from(y) + 1) * u64::from(height);
            let mut sums = [0u64; 4];
            let weight = u64::from(width) * u64::from(height);
            for sy in y0 / dh..y1.div_ceil(dh) {
                check()?;
                let wy = y1.min((sy + 1) * dh) - y0.max(sy * dh);
                for sx in x0 / dw..x1.div_ceil(dw) {
                    if sx.is_multiple_of(1024) {
                        check()?;
                    }
                    let wx = x1.min((sx + 1) * dw) - x0.max(sx * dw);
                    let pixel = &base[((sy * u64::from(width) + sx) * 4) as usize..][..4];
                    let a = u64::from(pixel[3]) * wx * wy;
                    sums[3] += a;
                    for channel in 0..3 {
                        sums[channel] += u64::from(pixel[channel]) * a;
                    }
                }
            }
            for channel in 0..3 {
                out.push(
                    (sums[channel] + sums[3] / 2)
                        .checked_div(sums[3])
                        .unwrap_or(0) as u8,
                );
            }
            out.push(((sums[3] + weight / 2) / weight) as u8);
        }
    }
    check()?;
    Ok(out)
}
