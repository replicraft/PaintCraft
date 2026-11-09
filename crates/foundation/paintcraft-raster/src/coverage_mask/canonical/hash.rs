//! Only fresh canonical builder output: no clip, shift, zero runs or equal rows.
use super::*;
const DOMAIN: &[u8] = b"paintcraft.a8-mask/v1\0";
pub(super) fn canonical(
    domain: Rect,
    rows: &[Rows],
    runs: &[Run],
    meter: &mut Meter<'_>,
) -> Result<[u8; 32]> {
    meter.debit(128)?;
    let mut hash = Sha256::new();
    meter.debit(2 * (DOMAIN.len() as u64 + 32 + 8))?;
    hash.update(DOMAIN);
    for v in [domain.x0, domain.y0, domain.x1, domain.y1] {
        hash.update(v.to_be_bytes());
    }
    hash.update((rows.len() as u64).to_be_bytes());
    for row in rows {
        meter.check()?;
        meter.debit(48)?;
        hash.update(row.y0.to_be_bytes());
        hash.update(row.y1.to_be_bytes());
        hash.update((row.len as u64).to_be_bytes());
        for chunk in runs[row.start..row.start + row.len].chunks(128) {
            meter.check()?;
            meter.debit(34 * chunk.len() as u64)?;
            for run in chunk {
                hash.update(run.x0.to_be_bytes());
                hash.update(run.x1.to_be_bytes());
                hash.update([run.coverage]);
            }
        }
    }
    meter.debit(128)?;
    let digest = hash.finalize();
    meter.debit(32)?;
    Ok(digest.into())
}
