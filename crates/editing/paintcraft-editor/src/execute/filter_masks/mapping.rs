//! Per-result inventory, distinct from cumulative transaction work/retained bytes.
use super::*;
use paintcraft_document::Layer;
#[derive(Clone, Copy)]
pub(in crate::execute) struct Counts {
    bands: usize,
    runs: usize,
}
impl Default for Counts {
    fn default() -> Self {
        let l = Limits::default();
        Self {
            bands: l.bands,
            runs: l.runs,
        }
    }
}
impl Counts {
    pub(super) fn limits(self, work: u64) -> Limits {
        Limits {
            work,
            bands: self.bands,
            runs: self.runs,
        }
    }
    pub(super) fn include(&mut self, field: &A8Mask) -> Result<()> {
        let s = field.stats();
        self.bands = self.bands.checked_sub(s.bands).ok_or_else(limit)?;
        self.runs = self.runs.checked_sub(s.runs).ok_or_else(limit)?;
        Ok(())
    }
}
fn inventory(
    stage: &Stage<'_>,
    excluded: impl Fn(&Layer) -> bool,
    filter: Option<(Uuid, Uuid)>,
    own: Option<Uuid>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Counts> {
    let mut counts = Counts::default();
    for layer in stage.doc.layers.values() {
        if excluded(layer) {
            continue;
        }
        check()?;
        for entry in layer.filters.iter() {
            if filter == Some((layer.id, entry.id())) {
                continue;
            }
            if let Some(mask) = entry.mask() {
                check()?;
                counts.include(mask.coverage())?;
            }
        }
        if let Some(mask) = layer.mask.as_ref().filter(|_| own != Some(layer.id)) {
            check()?;
            counts.include(mask.coverage())?;
        }
    }
    Ok(counts)
}
impl Stage<'_> {
    /// One ledger for the WHOLE operation group, including unchanged fields.
    /// Replay has its retained fields and never reconstructs inventory or masks.
    pub(in crate::execute) fn mask_mapping_counts(
        &mut self,
        excluded: impl Fn(&Layer) -> bool,
    ) -> Result<Counts> {
        if self.planning.is_none()
            || !self.doc.layers.values().any(|l| {
                excluded(l) && (l.mask.is_some() || l.filters.iter().any(|e| e.mask().is_some()))
            })
        {
            return Ok(Counts::default());
        }
        let remaining = Cell::new(self.coverage_work);
        let check = || {
            (self.check)()?;
            remaining.set(remaining.get().checked_sub(1).ok_or_else(limit)?);
            Ok(())
        };
        let counts = inventory(self, excluded, None, None, &check)?;
        self.coverage_work = remaining.get();
        Ok(counts)
    }
    pub(super) fn filter_mask_counts(&mut self, layer: Uuid, filter: Uuid) -> Result<Counts> {
        let remaining = Cell::new(self.coverage_work);
        let check = || {
            (self.check)()?;
            remaining.set(remaining.get().checked_sub(1).ok_or_else(limit)?);
            Ok(())
        };
        let counts = inventory(self, |_| false, Some((layer, filter)), None, &check)?;
        self.coverage_work = remaining.get();
        Ok(counts)
    }
    pub(super) fn layer_mask_counts(&mut self, layer: Uuid) -> Result<Counts> {
        let remaining = Cell::new(self.coverage_work);
        let check = || {
            (self.check)()?;
            remaining.set(remaining.get().checked_sub(1).ok_or_else(limit)?);
            Ok(())
        };
        let counts = inventory(self, |_| false, None, Some(layer), &check)?;
        self.coverage_work = remaining.get();
        Ok(counts)
    }
}
pub(super) fn coverage(
    mask: &A8Mask,
    stage: &Stage<'_>,
    mapping: Mapping,
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<A8Mask> {
    match mapping {
        Mapping::StretchSkew(g, extent) => {
            mask.stretch_skew_checked(g, extent, &stage.adm.coverage_budget(), limits, check)
        }
        Mapping::Empty => A8Mask::from_bands(
            Rect::from_origin_size(0, 0, 1, 1).unwrap(),
            &[],
            &stage.adm.coverage_budget(),
            limits,
            check,
        ),
        Mapping::Crop(area) => mask.cropped_checked(area, limits.work, check),
        Mapping::Orthogonal(size, t) => {
            mask.orthogonal_checked(size, t, &stage.adm.coverage_budget(), limits, check)
        }
        Mapping::Rebase { offset } => {
            mask.translated_checked(offset.0, offset.1, limits.work, check)
        }
    }
}
