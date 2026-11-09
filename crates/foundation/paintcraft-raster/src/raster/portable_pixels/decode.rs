use super::*;
#[repr(C)]
pub(in crate::raster) struct FixedBytes {
    pub(in crate::raster) bytes: Vec<u8>,
    pub(in crate::raster) account: BudgetAccount,
}
impl FixedBytes {
    fn from_account(n: usize, account: BudgetAccount) -> Result<Self> {
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(n).map_err(|_| limit())?;
        if bytes.capacity() != n {
            return Err(limit());
        }
        Ok(Self { bytes, account })
    }
    fn new(n: usize, scope: &BudgetScope, meter: &mut Meter<'_>) -> Result<Self> {
        meter.step(64)?;
        let mut account = scope.account();
        account.resize(n as u64)?;
        Self::from_account(n, account)
    }
}
pub(in crate::raster) fn fence(
    p: EncodedTilePayload<'_>,
    g: &mut PixelGeometry,
    meter: &mut Meter<'_>,
) -> Result<()> {
    if g.encoded == 0 {
        return Err(bad());
    }
    match p.encoding {
        PortablePixelEncoding::Raw if g.encoded != g.decoded => return Err(bad()),
        PortablePixelEncoding::Runs
            if !g.encoded.is_multiple_of(6) || g.encoded > 6 * (g.decoded / 4) =>
        {
            return Err(bad());
        }
        PortablePixelEncoding::RunsLz4 => {
            meter.debit(4)?;
            let bytes: [u8; 4] = p
                .bytes
                .get(..4)
                .ok_or_else(bad)?
                .try_into()
                .map_err(|_| bad())?;
            let n = u32::from_le_bytes(bytes) as usize;
            if g.encoded <= 4 || n == 0 || !n.is_multiple_of(6) || n > 6 * (g.decoded / 4) {
                return Err(bad());
            }
            g.runs_extent = n;
        }
        _ => {}
    }
    Ok(())
}
fn lz4(bytes: &[u8], output: &mut FixedBytes, n: usize, meter: &mut Meter<'_>) -> Result<()> {
    meter.debit(n as u64)?;
    output.bytes.resize(n, 0);
    meter.step(64 + bytes.len() as u64 + n as u64)?;
    let actual = lz4_flex::block::decompress_into(bytes, &mut output.bytes).map_err(|_| bad())?;
    meter.step(32)?;
    if actual != n {
        return Err(bad());
    }
    Ok(())
}
fn runs(bytes: &[u8], output: &mut FixedBytes, n: usize, meter: &mut Meter<'_>) -> Result<()> {
    for run in bytes.as_chunks::<6>().0 {
        meter.step(38)?;
        let count = usize::from(u16::from_le_bytes([run[0], run[1]])) + 1;
        let length = count.checked_mul(4).ok_or_else(bad)?;
        if length > n.checked_sub(output.bytes.len()).ok_or_else(bad)? {
            return Err(bad());
        }
        meter.debit((count as u64).checked_mul(8).ok_or_else(limit)?)?;
        for _ in 0..count {
            output.bytes.extend_from_slice(&run[2..]);
        }
    }
    meter.step(32)?;
    if output.bytes.len() != n {
        return Err(bad());
    }
    Ok(())
}
pub(in crate::raster) fn expand(
    p: EncodedTilePayload<'_>,
    g: PixelGeometry,
    scope: &BudgetScope,
    meter: &mut Meter<'_>,
) -> Result<Option<FixedBytes>> {
    match p.encoding {
        PortablePixelEncoding::Raw => Ok(None),
        PortablePixelEncoding::Runs => {
            let mut d = FixedBytes::new(g.decoded, scope, meter)?;
            runs(p.bytes, &mut d, g.decoded, meter)?;
            Ok(Some(d))
        }
        PortablePixelEncoding::Lz4 => {
            let mut d = FixedBytes::new(g.decoded, scope, meter)?;
            lz4(p.bytes, &mut d, g.decoded, meter)?;
            Ok(Some(d))
        }
        PortablePixelEncoding::RunsLz4 => {
            meter.step(64)?;
            let mut r_account = scope.account();
            r_account.resize(g.runs_extent as u64)?;
            let mut d_account = scope.account();
            d_account.resize(g.decoded as u64)?;
            let mut r = FixedBytes::from_account(g.runs_extent, r_account)?;
            lz4(&p.bytes[4..], &mut r, g.runs_extent, meter)?;
            meter.step(64)?;
            let mut d = FixedBytes::from_account(g.decoded, d_account)?;
            runs(&r.bytes, &mut d, g.decoded, meter)?;
            meter.debit(32)?;
            drop(r);
            Ok(Some(d))
        }
    }
}
