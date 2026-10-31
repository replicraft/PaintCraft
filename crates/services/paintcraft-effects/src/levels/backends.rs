//! Frozen prototype/reference accounting regressions, not elapsed comparisons.
use super::*;
use std::cell::Cell;
#[test]
fn explicit_backends_keep_original_quotes_and_distinct_actual_word_counts() {
    // Pinned from the immutable native and Wasm crossover evidence, before
    // sharing the scalar engine. These intentionally are not equal units.
    for (p, q32, used32, q64, used64) in [
        ([0, 255, 10, 0, 255], 55_132, 42_183, 82_741, 69_765),
        ([0, 255, 50, 0, 255], 40_862, 37_726, 73_566, 70_238),
        (
            [0, 255, 999, 0, 255],
            76_873_778,
            12_090_642,
            37_915_382,
            2_943_265,
        ),
        (
            [0, 255, 333, 255, 0],
            10_187_406,
            2_199_462,
            4_926_102,
            622_111,
        ),
    ] {
        let p = LevelsRgbV1::new(p[0], p[1], p[2], p[3], p[4]).unwrap();
        assert_eq!(quote(p).work, q32);
        assert_eq!(u64::quote(p).work, q64);
        let reference = {
            let mut scratch = Scratch::default();
            let mut out = [199; 256];
            assert_eq!(
                build_lut(p, &mut scratch, &mut out, quote(p).into(), &|| Ok(()))
                    .unwrap()
                    .units,
                used32
            );
            out
        };
        let candidate = {
            let mut scratch = u64::Scratch::default();
            let mut out = [198; 256];
            assert_eq!(
                u64::build_lut(p, &mut scratch, &mut out, u64::quote(p).into(), &|| Ok(()))
                    .unwrap()
                    .units,
                used64
            );
            out
        };
        assert_eq!(reference, candidate);
    }
}
#[test]
fn backend_grouping_check_boundaries_match_frozen_counted_calls() {
    let p = LevelsRgbV1::new(0, 4, 200, 255, 0).unwrap();
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let reference = {
        let mut s = Scratch::default();
        let mut out = [0; 256];
        build_lut(p, &mut s, &mut out, quote(p).into(), &check).unwrap();
        out
    };
    assert_eq!(calls.get(), 2287);
    calls.set(0);
    let candidate = {
        let mut s = u64::Scratch::default();
        let mut out = [0; 256];
        u64::build_lut(p, &mut s, &mut out, u64::quote(p).into(), &check).unwrap();
        out
    };
    assert_eq!(calls.get(), 2512);
    assert_eq!(reference, candidate);
}

#[test]
fn scratch_wrappers_have_exact_inner_size_and_alignment() {
    assert_eq!(
        std::mem::size_of::<Scratch>(),
        std::mem::size_of::<engine::Workspace<word::U32, 320>>()
    );
    assert_eq!(
        std::mem::align_of::<Scratch>(),
        std::mem::align_of::<engine::Workspace<word::U32, 320>>()
    );
    assert_eq!(
        std::mem::size_of::<u64::Scratch>(),
        std::mem::size_of::<engine::Workspace<word::U64, 160>>()
    );
    assert_eq!(
        std::mem::align_of::<u64::Scratch>(),
        std::mem::align_of::<engine::Workspace<word::U64, 160>>()
    );
}
