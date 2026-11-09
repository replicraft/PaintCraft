use super::*;
use crate::source8::tests::{Host, fixture};
use paintcraft_render::spatial::with_workspace;
use std::cell::{Cell, RefCell};
#[test]
fn entry_setup_exact_prefix_and_original_control_retirement() {
    let host = Host::new(1 << 20);
    let calls = Cell::new(0u64);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    host.root.reserve_work(7).unwrap();
    with_workspace(host.root.clone(), || {
        let entry = Entry8::new(host.request(), host.realms(host.caps), &check)?;
        assert_eq!(host.root.work_used(), 7 + 17248);
        assert_eq!(calls.get(), 0);
        assert_eq!(host.c.footprint().live, 8192);
        drop(entry);
        assert_eq!(host.c.footprint().live, 0);
        Ok(())
    })
    .unwrap();
}
#[test]
fn exact_capacity_mismatches_stop_before_allocation_with_literal_prefixes() {
    for (index, prefix) in [176, 288, 400, 512, 624, 800, 800].into_iter().enumerate() {
        for larger in [false, true] {
            let host = Host::new(1 << 20);
            // I/Q use positive mismatches only: capacity0 cannot subtract1.
            if index >= 5 && !larger {
                continue;
            }
            let mut caps = host.caps;
            let field = match index {
                0 => &mut caps.p,
                1 => &mut caps.m,
                2 => &mut caps.a,
                3 => &mut caps.c,
                4 => &mut caps.k,
                5 => &mut caps.i,
                _ => &mut caps.q,
            };
            *field = if larger { *field + 1 } else { *field - 1 };
            with_workspace(host.root.clone(), || {
                let error = Entry8::new(host.request(), host.realms(caps), &|| Ok(()))
                    .err()
                    .unwrap();
                assert_eq!(error.code, ErrorCode::ResourceLimit);
                assert_eq!(host.root.work_used(), prefix);
                assert_eq!(host.c.footprint().live, 0);
                assert_eq!(host.m.footprint().live, 0);
                Ok(())
            })
            .unwrap();
        }
    }
    let host = Host::new(8191);
    with_workspace(host.root.clone(), || {
        assert_eq!(
            Entry8::new(host.request(), host.realms(host.caps), &|| Ok(()))
                .err()
                .unwrap()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(host.root.work_used(), 864);
        assert_eq!(host.c.footprint().live, 0);
        Ok(())
    })
    .unwrap();
}
#[test]
fn failed_read_is_terminal_and_moves_the_exact_original_diagnostic() {
    let host = Host::new(1 << 20);
    let sentinel = Error::new(
        ErrorCode::OperationCancelled,
        "callback-owned diagnostic".repeat(300),
    )
    .at(19);
    let message_pointer = sentinel.message.as_ptr() as usize;
    let original = RefCell::new(Some(sentinel));
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        original.borrow_mut().take().map_or(Ok(()), Err)
    };
    with_workspace(host.root.clone(), || {
        let mut entry = Entry8::new(host.request(), host.realms(host.caps), &check)?;
        let error = entry.read_into_slot_checked(&fixture()).unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.operation_index, Some(19));
        assert_eq!(error.message.as_ptr() as usize, message_pointer);
        assert!(matches!(&entry.source, SourceSlot8::Failed));
        let before = (host.root.work_used(), calls.get());
        assert!(entry.read_into_slot_checked(&fixture()).is_err());
        assert_eq!((host.root.work_used(), calls.get()), before);
        assert_eq!(host.m.footprint().live, 0);
        assert_eq!(host.k.footprint().live, 0);
        drop(entry);
        assert_eq!(host.c.footprint().live, 0);
        Ok(())
    })
    .unwrap();
}
#[test]
fn successful_slot_cannot_reload_and_retirement_clone_is_drop_only() {
    let host = Host::new(1 << 20);
    // Captures Cell: the raw callback is intentionally NOT Sync.
    let calls = Cell::new(0u64);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    with_workspace(host.root.clone(), || {
        let mut entry = Entry8::new(host.request(), host.realms(host.caps), &check)?;
        entry.read_into_slot_checked(&fixture())?;
        assert!(matches!(&entry.source, SourceSlot8::Loaded(_)));
        let before = host.root.work_used();
        let _inspection = entry.inspect_checked()?;
        assert_eq!(host.root.work_used() - before, 800);
        let before = (host.root.work_used(), calls.get());
        assert!(entry.read_into_slot_checked(&fixture()).is_err());
        assert_eq!((host.root.work_used(), calls.get()), before);
        let before = host.root.work_used();
        let retirement = entry.clone_for_retirement_checked()?;
        assert_eq!(
            host.root.work_used() - before,
            if cfg!(target_pointer_width = "64") {
                674
            } else {
                658
            }
        );
        std::thread::scope(|scope| {
            scope.spawn(move || drop(retirement)).join().unwrap();
        });
        assert!(host.m.footprint().live > 0);
        assert_eq!(
            host.k.footprint().live,
            if cfg!(target_pointer_width = "64") {
                2992
            } else {
                1728
            }
        );
        drop(entry);
        assert_eq!(host.m.footprint().live, 0);
        assert_eq!(host.a.footprint().live, 0);
        assert_eq!(host.c.footprint().live, 0);
        assert_eq!(host.k.footprint().live, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn sixth_factory_and_both_iq_mismatch_directions_have_fixed_prefixes() {
    use paintcraft_core::types::resources::AllocationClass;
    for actual in [65535, 65537] {
        let mut host = Host::new(1 << 20);
        host.raster.paint_metadata_scope = host
            .budget
            .reserve_capacity(actual)
            .unwrap()
            .scope(AllocationClass::PaintMetadata);
        with_workspace(host.root.clone(), || {
            assert!(Entry8::new(host.request(), host.realms(host.caps), &|| Ok(())).is_err());
            assert_eq!(host.root.work_used(), 736);
            assert_eq!(host.c.footprint().live, 0);
            Ok(())
        })
        .unwrap();
    }
    for q in [false, true] {
        for declared in [0, 2] {
            let mut host = Host::new(1 << 20);
            let one = host.budget.reserve_capacity(1).unwrap();
            if q {
                host.q = one;
                host.caps.q = declared;
            } else {
                host.i = one;
                host.caps.i = declared;
            }
            with_workspace(host.root.clone(), || {
                assert!(Entry8::new(host.request(), host.realms(host.caps), &|| Ok(())).is_err());
                assert_eq!(host.root.work_used(), 800);
                assert_eq!(host.c.footprint().live, 0);
                Ok(())
            })
            .unwrap();
        }
    }
}
#[test]
fn reading_slot_refuses_reentry_before_decoder_or_counter_activity() {
    let host = Host::new(1 << 20);
    with_workspace(host.root.clone(), || {
        let mut entry = Entry8::new(host.request(), host.realms(host.caps), &|| Ok(()))?;
        entry.source = SourceSlot8::Reading;
        let before = host.root.work_used();
        assert!(entry.read_into_slot_checked(&fixture()).is_err());
        assert_eq!(host.root.work_used(), before);
        assert!(matches!(&entry.source, SourceSlot8::Reading));
        assert_eq!(host.m.footprint().live, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn original_callback_allocation_denial_is_not_replaced_by_source_context() {
    use paintcraft_core::types::resources::SharedBudget;
    let host = Host::new(1 << 20);
    let diagnostic_realm = SharedBudget::new(1);
    let original_error = diagnostic_realm.reserve(2).unwrap_err().at(23);
    let pointer = original_error.message.as_ptr() as usize;
    let original = RefCell::new(Some(original_error));
    let check = || original.borrow_mut().take().map_or(Ok(()), Err);
    with_workspace(host.root.clone(), || {
        let mut entry = Entry8::new(host.request(), host.realms(host.caps), &check)?;
        let error = entry.read_into_slot_checked(&fixture()).unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(error.operation_index, Some(23));
        assert_eq!(error.message.as_ptr() as usize, pointer);
        let denial = error.allocation_denial().unwrap();
        assert_eq!(denial.required_bytes(), 2);
        assert_eq!(denial.budget().usage(), diagnostic_realm.usage());
        assert_eq!(denial.budget().usage().limit, 1);
        assert_eq!(host.m.footprint().live, 0);
        assert_eq!(host.k.footprint().live, 0);
        Ok(())
    })
    .unwrap();
}
