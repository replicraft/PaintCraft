//! Complete conservative prepayment around the unchanged borrowed wire probe.
//! No callback adapter: old step512 callbacks stay paid exactly once; this buys
//! the raw sort topology and otherwise uninstrumented helper work BEFORE entry.
use super::{assembly_ledger as work, entry::EntryAuth8};
use paintcraft_core::types::Result;
use paintcraft_formats::pcnd::{
    RetainedSourceLayoutSummary8, layer_blend_appearance::CONTROL_BYTES,
};
use std::mem::size_of;
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Facts {
    layers: u64,
    filters: u64,
    masks: u64,
    bands: u64,
    runs: u64,
    metadata: u64,
}
fn quote(f: Facts) -> Result<u64> {
    let w = size_of::<usize>();
    let native = w == 8;
    if !matches!(w, 4 | 8) {
        return Err(work::limit());
    }
    let registry_sort = if native { 330880 } else { 255680 };
    let index = if native { 24 } else { 20 };
    let headers = (b"paintcraft.worker-coverage-bounds/v1\0".len()
        + 2
        + if native { 11 } else { 9 }
        + 2
        + 4
        + b"paintcraft.worker-mask-inventory/v1\0".len()
        + 4
        + b"paintcraft.worker-layer-masks/v1\0".len()
        + 4) as u64;
    let canonical = work::sum(&[
        work::mul(62, f.masks)?,
        work::mul(24, f.bands)?,
        work::mul(17, f.runs)?,
    ])?;
    work::sum(&[
        work::mul(16, CONTROL_BYTES)?,
        work::mul(208, f.metadata)?,
        work::mul(512, work::add(f.layers, 1)?)?,
        work::mul(
            128,
            work::sum(&[2, f.layers, f.filters, work::mul(6, f.masks)?])?,
        )?,
        work::mul(
            512,
            work::add(
                work::bare(f.layers)?,
                work::bare(f.filters)?.min(work::mul(16, f.filters)?),
            )?,
        )?,
        work::mul(3 * 1706, f.filters)?,
        1706 * 13 + registry_sort + 1280 + 64 * 13,
        work::mul(4 * index, work::add(f.layers, f.filters)?)?,
        work::mul(3856, f.masks)?,
        work::add(
            work::mul(
                16,
                work::add(work::mul(3, canonical)?, work::mul(384, f.masks)?)?,
            )?,
            work::mul(1440, f.masks)?,
        )?,
        work::add(
            work::mul(16, work::sum(&[headers, work::mul(327, f.masks)?, 384])?)?,
            1440,
        )?,
        2528,
    ])
}
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
