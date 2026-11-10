//! Shared defining quote before the unchanged borrowed Source8 wire probe.
use super::{assembly_ledger as work, entry::EntryAuth8};
use paintcraft_core::types::Result;
use paintcraft_formats::pcnd::RetainedSourceLayoutSummary8;
#[cfg(test)]
use paintcraft_formats::pcnd::layer_blend_appearance::CONTROL_BYTES;
pub(super) use paintcraft_formats::pcnd::layer_blend_appearance::{
    Source8ProbeWorkFacts as Facts, source8_probe_supplemental_work_quote as quote,
};
#[cfg(test)]
use std::mem::size_of;
pub(super) fn prepay(summary: &RetainedSourceLayoutSummary8, auth: &EntryAuth8<'_>) -> Result<()> {
    auth.spend(64)?;
    let metadata = summary
        .metadata_bytes()
        .checked_sub(summary.profile_bytes())
        .ok_or_else(work::invalid)?;
    let units = quote(Facts {
        layers: summary.layers(),
        filters: summary.filters(),
        masks: summary.mask_present(),
        bands: summary.bands(),
        runs: summary.runs(),
        metadata,
    })?;
    auth.spend(units)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_fixed_examples_and_inline_controls_match_frozen_targets() {
        assert_eq!(size_of::<Facts>(), 48);
        let native = cfg!(target_pointer_width = "64");
        for (facts, expected) in [
            (
                Facts {
                    layers: 2,
                    filters: 0,
                    masks: 0,
                    bands: 0,
                    runs: 0,
                    metadata: 110,
                },
                if native { 396114 } else { 320850 },
            ),
            (
                Facts {
                    layers: 1,
                    filters: 1,
                    masks: 1,
                    bands: 1,
                    runs: 1,
                    metadata: 128,
                },
                if native { 423264 } else { 348000 },
            ),
            (
                Facts {
                    layers: 1,
                    filters: 256,
                    masks: 256,
                    bands: 256,
                    runs: 256,
                    metadata: 8192,
                },
                if native { 11262226 } else { 11182882 },
            ),
        ] {
            assert_eq!(quote(facts).unwrap(), 16 * CONTROL_BYTES + expected);
        }
    }
    #[test]
    fn total_filter_sort_bound_handles_all_trivial_and_supported_stacks() {
        for n in 0..=256 {
            assert!(work::bare(n).unwrap() <= 16 * n);
        }
        for a in 0..=256 {
            for b in [0, 1, 2, 255, 256] {
                assert!(
                    work::bare(a).unwrap() + work::bare(b).unwrap() <= work::bare(a + b).unwrap()
                );
            }
        }
    }
}
