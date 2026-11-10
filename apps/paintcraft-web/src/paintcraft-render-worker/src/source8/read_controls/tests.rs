use super::*;
use crate::source8::{
    entry,
    tests::{Host, fixture},
};
use paintcraft_core::types::{ErrorCode, resources::AllocationClass};
use paintcraft_render::spatial::with_workspace;

#[test]
fn compiled_reader_facts_match_target_and_preserve_existing_caller() {
    let host = Host::new(1 << 20);
    let before = host.budget.usage();
    assert_eq!(std::mem::size_of::<Source8ReaderControlBytes>(), 24);
    let facts = source8_reader_control_bytes().unwrap();
    assert_eq!(
        facts,
        Source8ReaderControlBytes {
            entry: 8192,
            assembly: 4096,
            retained_carriers: if cfg!(target_pointer_width = "64") {
                2992
            } else {
                1728
            },
        }
    );
    assert_eq!(host.budget.usage(), before);
    assert_eq!(host.root.work_used(), 0);
}

#[test]
fn quoted_entry_controls_exact_and_one_short_use_original_c() {
    let quote = source8_reader_control_bytes().unwrap();
    for short in [0, 1] {
        let host = Host::new(quote.entry - short);
        with_workspace(host.root.clone(), || {
            let check = || Ok(());
            let result = entry::Entry8::new(host.request(), host.realms(host.caps), &check);
            if short == 0 {
                let entry = result.unwrap();
                assert_eq!(host.c.footprint().live, quote.entry);
                drop(entry);
            } else {
                assert_eq!(result.err().unwrap().code, ErrorCode::ResourceLimit);
            }
            assert_eq!(host.c.footprint().live, 0);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn quoted_carriers_exact_and_one_short_use_original_k_after_real_source_read() {
    let quote = source8_reader_control_bytes().unwrap();
    let wire = fixture();
    for short in [0, 1] {
        let mut host = Host::new(1 << 20);
        host.k = host
            .budget
            .reserve_capacity(quote.retained_carriers - short)
            .unwrap()
            .scope(AllocationClass::SharedBudget);
        host.caps.k = quote.retained_carriers - short;
        with_workspace(host.root.clone(), || {
            let check = || Ok(());
            let mut entry = entry::Entry8::new(host.request(), host.realms(host.caps), &check)?;
            let result = entry.read_into_slot_checked(&wire);
            if short == 0 {
                result?;
                assert_eq!(entry.inspect_checked()?.layers(), 2);
                assert_eq!(host.k.footprint().live, quote.retained_carriers);
                // The assembly account has retired; Entry's own C control remains.
                assert_eq!(host.c.footprint().live, quote.entry);
            } else {
                assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
                assert_eq!(host.k.footprint().live, 0);
                assert_eq!(host.m.footprint().live, 0);
            }
            drop(entry);
            for scope in [&host.p, &host.m, &host.a, &host.c, &host.k] {
                assert_eq!(scope.footprint().live, 0);
            }
            Ok(())
        })
        .unwrap();
        let budget = host.budget.clone();
        drop(host);
        assert_eq!(budget.usage().reserved, 0);
    }
}
