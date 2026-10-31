//! One canonical borrowed parser, with either adopting or non-retaining limits.
use super::*;
use paintcraft_raster::coverage_mask::Limits;
use std::cell::Cell;
pub(super) trait MaskAssessment {
    fn step(&self, check: &dyn Fn() -> Result<()>) -> Result<()>;
    fn remaining(&self) -> (usize, usize);
}
impl MaskAssessment for CoverageReader<'_> {
    fn step(&self, check: &dyn Fn() -> Result<()>) -> Result<()> {
        CoverageReader::step(self, check)
    }
    fn remaining(&self) -> (usize, usize) {
        CoverageReader::remaining(self)
    }
}
struct Borrowed {
    bands: usize,
    runs: usize,
    work: Cell<u64>,
}
impl MaskAssessment for Borrowed {
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
pub(in super::super) fn skip(
    input: &mut Input<'_>,
    allowance: usize,
    limits: &mut Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let borrowed = Borrowed {
        bands: limits.bands,
        runs: limits.runs,
        work: Cell::new(limits.work),
    };
    let result = inspect(input, allowance, &borrowed, check);
    limits.work = borrowed.work.get();
    if let Some(header) = result? {
        limits.bands = limits
            .bands
            .checked_sub(header.bands)
            .ok_or_else(resource)?;
        limits.runs = limits.runs.checked_sub(header.runs).ok_or_else(resource)?;
    }
    Ok(())
}
