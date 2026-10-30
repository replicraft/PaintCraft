//! Two-pass canonical construction; retained allocations are admitted first.
use super::*;

use std::mem::size_of;
#[derive(Clone)]
pub(super) struct Runs<'a> {
    source: &'a [Run],
    index: usize,
}
impl<'a> Runs<'a> {
    pub(super) fn new(source: &'a [Run]) -> Self {
        Self { source, index: 0 }
    }
}
impl Runs<'_> {
    pub(super) fn next_checked(&mut self, probe: &mut Probe<'_>) -> Result<Option<Run>> {
        let mut found = None;
        while let Some(run) = self.source.get(self.index) {
            probe.step()?;
            self.index += 1;
            if run.coverage != 0 {
                found = Some(*run);
                break;
            }
        }
        let Some(mut run) = found else {
            return Ok(None);
        };
        while let Some(next) = self.source.get(self.index) {
            probe.step()?;
            if next.coverage == 0 {
                self.index += 1;
                continue;
            }
            if next.x0 == run.x1 && next.coverage == run.coverage {
                run.x1 = next.x1;
                self.index += 1;
            } else {
                break;
            }
        }
        Ok(Some(run))
    }
    fn equal_checked(mut self, mut other: Self, probe: &mut Probe<'_>) -> Result<bool> {
        loop {
            match (self.next_checked(probe)?, other.next_checked(probe)?) {
                (None, None) => return Ok(true),
                (Some(a), Some(b)) if a == b => {}
                _ => return Ok(false),
            }
        }
    }
}
pub(super) fn build(
    domain: Rect,
    input: &[Band<'_>],
    scope: &BudgetScope,
    limits: Limits,
    probe: &mut Probe<'_>,
) -> Result<Option<Arc<Storage>>> {
    let mut last_y = domain.y0;
    let mut prior: Option<Band<'_>> = None;
    let (mut bands, mut runs) = (0usize, 0usize);
    for band in input {
        probe.step()?;
        if band.y0 < last_y || band.y0 >= band.y1 || band.y0 < domain.y0 || band.y1 > domain.y1 {
            return Err(schema());
        }
        last_y = band.y1;
        let mut last_x = domain.x0;
        for run in band.runs {
            probe.step()?;
            if run.x0 < last_x || run.x0 >= run.x1 || run.x0 < domain.x0 || run.x1 > domain.x1 {
                return Err(schema());
            }
            last_x = run.x1;
        }
        let mut cursor = Runs::new(band.runs);
        let mut count = 0;
        while cursor.next_checked(probe)?.is_some() {
            count += 1;
        }
        if count == 0 {
            continue;
        }
        let merge = match prior {
            Some(p) if p.y1 == band.y0 => {
                Runs::new(p.runs).equal_checked(Runs::new(band.runs), probe)?
            }
            _ => false,
        };
        if merge {
            let p = prior.as_mut().unwrap();
            p.y1 = band.y1;
        } else {
            bands = bands.checked_add(1).ok_or_else(limit)?;
            runs = runs.checked_add(count).ok_or_else(limit)?;
            prior = Some(*band);
        }
        if bands > limits.bands || runs > limits.runs {
            return Err(limit());
        }
    }
    if bands == 0 {
        return Ok(None);
    }
    let mut charge = scope.account();
    charge.resize(quote(bands, runs)?)?;
    let mut output = Vec::<Rows>::new();
    let mut values = Vec::<Run>::new();
    output.try_reserve_exact(bands).map_err(|_| limit())?;
    values.try_reserve_exact(runs).map_err(|_| limit())?;
    let bytes = quote(output.capacity(), values.capacity())?;
    if let Err(error) = charge.resize(bytes) {
        drop(output);
        drop(values);
        return Err(error);
    }
    for band in input {
        probe.step()?;
        // Input traversal remains cancellable even when every zero run disappears.
        for _ in band.runs {
            probe.step()?;
        }
        if Runs::new(band.runs).next_checked(probe)?.is_none() {
            continue;
        }
        let merge = if let Some(last) = output.last().filter(|b| b.y1 == band.y0) {
            Runs::new(&values[last.start..last.start + last.len])
                .equal_checked(Runs::new(band.runs), probe)?
        } else {
            false
        };
        if merge {
            output.last_mut().unwrap().y1 = band.y1;
            continue;
        }
        let start = values.len();
        let mut cursor = Runs::new(band.runs);
        while let Some(run) = cursor.next_checked(probe)? {
            values.push(run);
        }
        output.push(Rows {
            y0: band.y0,
            y1: band.y1,
            start,
            len: values.len() - start,
        });
    }
    probe.step()?;
    Ok(Some(Arc::new(Storage {
        bands: output,
        runs: values,
        bytes,
        _charge: charge,
    })))
}

pub(super) fn quote(bc: usize, rc: usize) -> Result<u64> {
    if bc == 0 {
        return if rc == 0 { Ok(0) } else { Err(schema()) };
    }
    if rc < bc {
        return Err(schema());
    }
    bc.checked_mul(size_of::<Rows>())
        .and_then(|b| {
            rc.checked_mul(size_of::<Run>())
                .and_then(|r| b.checked_add(r))
        })
        .and_then(|n| n.checked_add(size_of::<Storage>() + 2 * size_of::<usize>()))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(limit)
}
