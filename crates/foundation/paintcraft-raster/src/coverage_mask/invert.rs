//! Sparse finite complement. One shared work probe covers both passes and hash.
use super::*;

/// Canonical visible rows are disjoint; complement is a pointwise bijection,
/// so unequal neighboring row functions stay unequal. Zero output rows vanish.
/// Gaps become full255 rows, distinct from any inverted nonzero input row.
fn walk(
    mask: &A8Mask,
    probe: &mut Probe<'_>,
    mut emit: impl FnMut(Visit) -> Result<()>,
) -> Result<()> {
    let domain = mask.domain;
    let mut y = domain.y0;
    let mut bands = mask.visible();
    while let Some(band) = bands.next_checked(probe)? {
        probe.step()?;
        if y < band.y0 {
            full_row(domain, y, band.y0, probe, &mut emit)?;
        }
        let mut count = 0usize;
        row_runs(domain, band.runs(), probe, |_| {
            count = count.checked_add(1).ok_or_else(limit)?;
            Ok(())
        })?;
        if count != 0 {
            probe.step()?;
            emit(Visit::Band {
                y0: band.y0,
                y1: band.y1,
                runs: count,
            })?;
            row_runs(domain, band.runs(), probe, |run| emit(Visit::Run(run)))?;
        }
        y = band.y1;
    }
    if y < domain.y1 {
        full_row(domain, y, domain.y1, probe, &mut emit)?;
    }
    probe.step()
}
fn full_row(
    domain: Rect,
    y0: i64,
    y1: i64,
    probe: &mut Probe<'_>,
    emit: &mut impl FnMut(Visit) -> Result<()>,
) -> Result<()> {
    probe.step()?;
    emit(Visit::Band { y0, y1, runs: 1 })?;
    probe.step()?;
    emit(Visit::Run(Run {
        x0: domain.x0,
        x1: domain.x1,
        coverage: 255,
    }))
}
fn row_runs(
    domain: Rect,
    mut runs: views::VisibleRuns<'_>,
    probe: &mut Probe<'_>,
    mut emit: impl FnMut(Run) -> Result<()>,
) -> Result<()> {
    let mut x = domain.x0;
    while let Some(run) = runs.next_checked(probe)? {
        if x < run.x0 {
            probe.step()?;
            emit(Run {
                x0: x,
                x1: run.x0,
                coverage: 255,
            })?;
        }
        if run.coverage != 255 {
            probe.step()?;
            emit(Run {
                x0: run.x0,
                x1: run.x1,
                coverage: 255 - run.coverage,
            })?;
        }
        x = run.x1;
    }
    if x < domain.x1 {
        probe.step()?;
        emit(Run {
            x0: x,
            x1: domain.x1,
            coverage: 255,
        })?;
    }
    Ok(())
}
impl A8Mask {
    /// Complement coverage inside the retained domain only; outside stays zero.
    /// Missing/fully clipped storage becomes one full rectangle, regardless of
    /// logical area. The input is immutable and remains live throughout admission.
    /// `limits` bounds the aggregate canonical result and ALL traversal/hash work.
    pub fn inverted_checked(
        &self,
        budget: &SharedBudget,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        self.inverted_with_account(
            || budget.account(AllocationClass::CoverageMasks),
            limits,
            check,
        )
    }
    /// Preserve the exact supplied scope/reservation for the final canonical
    /// payload. The unchanged sparse algorithm allocates no dense field.
    pub fn inverted_in_scope_checked(
        &self,
        scope: &BudgetScope,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        self.inverted_with_account(|| scope.account(), limits, check)
    }
    fn inverted_with_account(
        &self,
        account: impl FnOnce() -> BudgetAccount,
        limits: Limits,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let mut probe = Probe {
            remaining: Cell::new(limits.work),
            check,
        };
        probe.step()?;
        valid(self.domain)?;
        let (mut bands, mut runs) = (0usize, 0usize);
        walk(self, &mut probe, |event| {
            if let Visit::Band { runs: count, .. } = event {
                bands = bands.checked_add(1).ok_or_else(limit)?;
                runs = runs.checked_add(count).ok_or_else(limit)?;
                if bands > limits.bands || runs > limits.runs {
                    return Err(limit());
                }
            }
            Ok(())
        })?;
        let storage = if bands == 0 {
            None
        } else {
            // Admission precedes allocation; retain actual capacities and the
            // existing Storage/Arc allowance. No uncharged staging Vec or pool.
            let mut charge = account();
            charge.resize(builder::quote(bands, runs)?)?;
            let mut output = Vec::<Rows>::new();
            let mut values = Vec::<Run>::new();
            output.try_reserve_exact(bands).map_err(|_| limit())?;
            values.try_reserve_exact(runs).map_err(|_| limit())?;
            let bytes = builder::quote(output.capacity(), values.capacity())?;
            if let Err(error) = charge.resize(bytes) {
                drop(output);
                drop(values);
                return Err(error);
            }
            walk(self, &mut probe, |event| {
                match event {
                    Visit::Band { y0, y1, runs } => output.push(Rows {
                        y0,
                        y1,
                        start: values.len(),
                        len: runs,
                    }),
                    Visit::Run(run) => values.push(run),
                }
                Ok(())
            })?;
            probe.step()?;
            Some(Arc::new(Storage {
                bands: output,
                runs: values,
                bytes,
                _charge: charge,
            }))
        };
        let mut result = Self {
            domain: self.domain,
            support: Some(self.domain),
            offset: (0, 0),
            storage,
            digest: [0; 32],
            stats: Stats::default(),
        };
        (result.digest, result.stats) = result.hash(&mut probe)?;
        probe.step()?;
        Ok(result)
    }
}
