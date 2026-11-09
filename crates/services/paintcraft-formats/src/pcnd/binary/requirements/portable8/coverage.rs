//! SAME Plan/original coverage/M bound once; views cannot authorize another source.
use super::*;
use paintcraft_raster::coverage_mask::{A8Mask, CanonicalA8Builder, CanonicalA8Config, Run};
/// Narrow materialization primitive, not the future closed Source8 carrier.
/// Caller retains the ORIGINAL full M union through every returned mask alias.
/// A borrowed description cannot authorize a different source:
/// ```compile_fail
/// use paintcraft_formats::pcnd::{PortableCoverageMaterializer8, PortableMaskView};
/// fn forge(m: &PortableCoverageMaterializer8<'_, '_>, v: PortableMaskView<'_>) {
///     let _ = m.materialize(Some(v));
/// }
/// ```
pub struct PortableCoverageMaterializer8<'p, 'a> {
    plan: &'p PortableSource8Plan<'a>,
    scope: &'p BudgetScope,
    guard: &'p BudgetAccount,
    _control: BudgetAccount,
}
impl std::fmt::Debug for PortableCoverageMaterializer8<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortableCoverageMaterializer8")
            .finish_non_exhaustive()
    }
}
impl<'a> PortableSource8Plan<'a> {
    pub fn coverage_materializer<'p>(
        &'p self,
        scope: &'p BudgetScope,
        guard: &'p BudgetAccount,
    ) -> Result<PortableCoverageMaterializer8<'p, 'a>> {
        self.meter.debit(64)?;
        let bytes = [
            size_of::<PortableCoverageMaterializer8<'static, 'static>>(),
            size_of::<wire::Cursor<'static, 'static>>(),
            size_of::<PortableMaskView<'static>>(),
            size_of::<PortableLayerCursor<'static, 'static>>(),
            4 * size_of::<u64>(),
            2 * size_of::<usize>(),
        ]
        .into_iter()
        .try_fold(0usize, |n, v| n.checked_add(v).ok_or_else(bound))?;
        let mut control = scope.account();
        control.resize(bytes as u64)?;
        self.meter.step(16)?;
        Ok(PortableCoverageMaterializer8 {
            plan: self,
            scope,
            guard,
            _control: control,
        })
    }
}
impl PortableCoverageMaterializer8<'_, '_> {
    pub fn layer(&self, layer: usize) -> Result<Option<(bool, A8Mask)>> {
        let cursor = self.plan.layer(layer)?;
        self.materialize(cursor.mask()?)
    }
    pub fn filter(&self, layer: usize, filter: usize) -> Result<Option<(bool, A8Mask)>> {
        let cursor = self.plan.layer(layer)?;
        self.materialize(cursor.filter(filter)?.mask)
    }
    fn materialize(&self, view: Option<PortableMaskView<'_>>) -> Result<Option<(bool, A8Mask)>> {
        let Some(view) = view else {
            return Ok(None);
        };
        let mut cursor = wire::Cursor::new(view.bytes, &self.plan.meter);
        if cursor.byte()? != 1 {
            return Err(invalid());
        }
        let enabled = cursor.flag()?;
        if cursor.byte()? != 1 {
            return Err(invalid());
        }
        let (x0, y0, x1, y1) = (
            cursor.signed()?,
            cursor.signed()?,
            cursor.signed()?,
            cursor.signed()?,
        );
        self.plan.meter.step(32)?;
        let domain = Rect::new(
            x0,
            y0,
            x1.checked_sub(x0).ok_or_else(invalid)?,
            y1.checked_sub(y0).ok_or_else(invalid)?,
        )
        .ok_or_else(invalid)?;
        let bands = cursor.number(self.plan.limits.coverage.bands as u64)?;
        if domain != view.domain || bands != view.bands || enabled != view.enabled {
            return Err(invalid());
        }
        let spend = |n| self.plan.meter.debit(n);
        // Raster has already debited its512 slot; record this actual raw check
        // in the SAME Plan census without a second step/debit.
        let check = || {
            self.plan
                .meter
                .checks
                .set(add(self.plan.meter.checks.get(), 1)?);
            (self.plan.meter.check)()
        };
        let mut builder = CanonicalA8Builder::new(
            CanonicalA8Config {
                domain,
                bands: usize::try_from(bands).map_err(|_| bound())?,
                runs: usize::try_from(view.runs).map_err(|_| bound())?,
                limits: self.plan.limits.coverage,
            },
            self.scope,
            self.guard,
            &check,
            &spend,
        )?;
        let mut total = 0u64;
        for _ in 0..bands {
            self.plan.meter.debit(32)?;
            let y0 = domain
                .y0
                .checked_add(i64::try_from(cursor.number(u64::MAX)?).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            let y1 = y0
                .checked_add(i64::try_from(cursor.number(u64::MAX)?).map_err(|_| invalid())?)
                .ok_or_else(invalid)?;
            let count = cursor.number(view.runs)?;
            total = total
                .checked_add(count)
                .filter(|n| *n <= view.runs)
                .ok_or_else(invalid)?;
            builder = builder.band(y0, y1, usize::try_from(count).map_err(|_| bound())?)?;
            for _ in 0..count {
                self.plan.meter.debit(32)?;
                let x0 = domain
                    .x0
                    .checked_add(i64::try_from(cursor.number(u64::MAX)?).map_err(|_| invalid())?)
                    .ok_or_else(invalid)?;
                let x1 = x0
                    .checked_add(i64::try_from(cursor.number(u64::MAX)?).map_err(|_| invalid())?)
                    .ok_or_else(invalid)?;
                builder = builder.run(Run {
                    x0,
                    x1,
                    coverage: cursor.byte()?,
                })?;
            }
        }
        self.plan.meter.step(32)?;
        if total != view.runs || cursor.input.remaining() != 0 {
            return Err(invalid());
        }
        Ok(Some((enabled, builder.finish()?)))
    }
}
#[cfg(test)]
mod tests;
