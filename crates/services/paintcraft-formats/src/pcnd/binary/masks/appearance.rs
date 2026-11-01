//! Borrowed canonical A8 facts; never materializes a mask or coverage realm.
use super::*;
use paintcraft_raster::coverage_mask::Limits;
use std::cell::Cell;
#[derive(Clone, Copy, Debug)]
pub(in crate::pcnd) struct Fact {
    pub enabled: bool,
    pub domain: [i64; 4],
    pub digest: [u8; 32],
    pub bands: u64,
    pub runs: u64,
    pub upper: u64,
}
struct Assessment {
    bands: usize,
    runs: usize,
    work: Cell<u64>,
}
impl assessment::MaskAssessment for Assessment {
    fn step(&self, check: &dyn Fn() -> Result<()>) -> Result<()> {
        check()?;
        self.work
            .set(self.work.get().checked_sub(1).ok_or_else(resource)?);
        Ok(())
    }
    fn remaining(&self) -> (usize, usize) {
        (self.bands, self.runs)
    }
}
pub(in crate::pcnd) fn probe(
    input: &mut Input<'_>,
    allowance: usize,
    limits: &mut Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Option<Fact>> {
    let original = *input;
    let assessment = Assessment {
        bands: limits.bands,
        runs: limits.runs,
        work: Cell::new(limits.work),
    };
    let result = inspect(input, allowance, &assessment, check);
    limits.work = assessment.work.get();
    let Some(header) = result? else {
        return Ok(None);
    };
    limits.bands = limits
        .bands
        .checked_sub(header.bands)
        .ok_or_else(resource)?;
    limits.runs = limits.runs.checked_sub(header.runs).ok_or_else(resource)?;
    let domain = [
        header.domain.x0,
        header.domain.y0,
        header.domain.x1,
        header.domain.y1,
    ];
    let mut h = Sha256::new();
    h.update(b"paintcraft.a8-mask/v1\0");
    for n in domain {
        h.update(n.to_be_bytes());
    }
    h.update((header.bands as u64).to_be_bytes());
    let mut wire = original;
    wire.take(3)?;
    for _ in 0..4 {
        wire.signed()?;
    }
    wire.number(u64::MAX)?;
    for _ in 0..header.bands {
        assessment::MaskAssessment::step(&assessment, check)?;
        let y0 = add(header.domain.y0, wire.number(u64::MAX)?)?;
        let y1 = add(y0, wire.number(u64::MAX)?)?;
        let count = wire.number(u64::MAX)?;
        h.update(y0.to_be_bytes());
        h.update(y1.to_be_bytes());
        h.update(count.to_be_bytes());
        for _ in 0..count {
            assessment::MaskAssessment::step(&assessment, check)?;
            let x0 = add(header.domain.x0, wire.number(u64::MAX)?)?;
            let x1 = add(x0, wire.number(u64::MAX)?)?;
            h.update(x0.to_be_bytes());
            h.update(x1.to_be_bytes());
            h.update([wire.byte()?]);
        }
    }
    if wire.remaining() != input.remaining() {
        return Err(invalid());
    }
    limits.work = assessment.work.get();
    Ok(Some(Fact {
        enabled: header.enabled,
        domain,
        digest: h.finalize().into(),
        bands: header.bands as u64,
        runs: header.runs as u64,
        upper: decode_quote(header.bands, header.runs)?,
    }))
}
