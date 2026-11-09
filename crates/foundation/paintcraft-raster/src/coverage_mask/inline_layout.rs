//! Actual defining-type guard for the finite rooted-frame source-action ledger.
use super::{
    Probe, Run,
    views::{Visible, VisibleBand, VisibleRuns},
};
use paintcraft_core::types::{Error, Result};
use std::mem::size_of;

/// Whether the actual target fits the declared inline upper bounds for regional
/// coverage proofs. Pure, callback/allocation/TLS free; no value is constructed.
/// This establishes no source authority, caller capacity, heap or RSS bound.
/// The host pays its layout-control phase before entering appearance helpers.
/// Arbitrary diagnostic String backing storage is outside this inline policy.
pub const fn uniform_region_inline_layout_supported() -> bool {
    let plain = match size_of::<usize>() {
        8 => {
            size_of::<VisibleRuns<'static>>() <= 48
                && size_of::<VisibleBand<'static>>() <= 64
                && size_of::<Option<VisibleBand<'static>>>() <= 72
                && size_of::<Visible<'static>>() <= 88
                && size_of::<Probe<'static>>() <= 24
        }
        4 => {
            size_of::<VisibleRuns<'static>>() <= 40
                && size_of::<VisibleBand<'static>>() <= 56
                && size_of::<Option<VisibleBand<'static>>>() <= 64
                && size_of::<Visible<'static>>() <= 72
                && size_of::<Probe<'static>>() <= 16
        }
        _ => false,
    };
    plain
        && size_of::<Run>() <= 24
        && size_of::<Option<Run>>() <= 32
        && size_of::<Error>() <= 96
        && size_of::<Result<()>>() <= 112
        && size_of::<Result<Option<Run>>>() <= 112
        && size_of::<Result<Option<VisibleBand<'static>>>>() <= 112
        && size_of::<Result<Option<u8>>>() <= 112
        && size_of::<Result<bool>>() <= 112
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_owned_module_types_fit_the_frozen_inline_envelope() {
        assert!(uniform_region_inline_layout_supported());
        assert!(size_of::<Error>() <= 96);
        assert!(size_of::<Result<Option<VisibleBand<'static>>>>() <= 112);
        assert!(size_of::<Result<Option<Run>>>() <= 112);
        assert!(size_of::<Result<()>>() <= 112);
    }
}
