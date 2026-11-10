use super::*;
use std::cell::Cell;
#[path = "test_host.rs"]
mod test_host;
use test_host::*;
fn assert_same(a: &caller::RequiredCaller8, b: &caller::RequiredCaller8) {
    match (a, b) {
        (caller::RequiredCaller8::Base(a), caller::RequiredCaller8::Base(b)) => {
            assert!(a.same_job(b));
            assert_eq!(
                a.prepared_owner_union_bytes(None).unwrap(),
                b.prepared_owner_union_bytes(None).unwrap()
            );
        }
        (
            caller::RequiredCaller8::Noise {
                selected: a,
                ambient: x,
            },
            caller::RequiredCaller8::Noise {
                selected: b,
                ambient: y,
            },
        ) => {
            assert!(a.same_job(b));
            assert!(x.same_job(y));
            assert_eq!(a.used().unwrap(), b.used().unwrap());
            assert_eq!(
                a.workspace()
                    .prepared_owner_union_bytes(Some(x.workspace()))
                    .unwrap(),
                b.workspace()
                    .prepared_owner_union_bytes(Some(y.workspace()))
                    .unwrap()
            );
        }
        _ => panic!("caller kind changed"),
    }
}
#[test]
fn independent_layout_and_transfer_limits_match_fixed_original_control() {
    layout::validate().unwrap();
    let n = cfg!(target_pointer_width = "64");
    assert_eq!(
        layout::transfer_named().unwrap(),
        if n { 3888 } else { 3116 }
    );
    assert_eq!(layout::begin_named().unwrap(), if n { 5192 } else { 4336 });
    assert_eq!(layout::view_named().unwrap(), if n { 3808 } else { 3240 });
    assert_eq!(transfer_work().unwrap(), if n { 7826 } else { 6282 });
    assert_eq!(patch_auth_work().unwrap(), if n { 7648 } else { 6512 });
}
#[test]
fn extraction_is_terminal_and_does_not_renew_failed_read() {
    let host = Host::new(false);
    let wire = bytes(&row("identity-full"));
    host.run(|| {
        let check = || Ok(());
        let mut entry = entry::Entry8::new(host.request(), host.realms(), &check)?;
        assert!(entry.take_service_source_checked().is_err());
        assert!(entry.read_into_slot_checked(b"invalid").is_err());
        assert!(entry.take_service_source_checked().is_err());
        assert!(entry.read_into_slot_checked(&wire).is_err());
        drop(entry);
        let mut entry = entry::Entry8::new(host.request(), host.realms(), &check)?;
        entry.read_into_slot_checked(&wire)?;
        let before = host.c.footprint().live;
        let source = entry.take_service_source_checked()?;
        assert_eq!(source._control.bytes(), 8192);
        assert_eq!(
            entry.inspect_checked().err().unwrap().code,
            ErrorCode::InvariantViolation
        );
        assert!(entry.take_service_source_checked().is_err());
        assert!(entry.read_into_slot_checked(&wire).is_err());
        drop(entry);
        assert_eq!(host.c.footprint().live, before);
        let mut original_p = source.factories._p.account();
        original_p.resize(2 << 20)?;
        assert_eq!(host.p.footprint().live, 2 << 20);
        drop(original_p);
        drop(source);
        assert_eq!(host.c.footprint().live, 0);
        Ok(())
    })
    .unwrap();
}
#[test]
fn literal_exact_transfer_work_and_one_short_preserve_paid_prefix_before_take() {
    let n = cfg!(target_pointer_width = "64");
    for (extra, success) in [
        (if n { 9202 } else { 7658 }, true),
        (if n { 9201 } else { 7657 }, false),
    ] {
        let host = Host::new(false);
        let wire = bytes(&row("identity-full"));
        host.run(|| {
            let check = || Ok(());
            let mut entry = entry::Entry8::new(host.request(), host.realms(), &check)?;
            entry.read_into_slot_checked(&wire)?;
            let used = host.base().work_used();
            host.base().reserve_work(268435456 - used - extra)?;
            let before = host.base().work_used();
            let result = entry.take_service_source_checked();
            if success {
                let source = result?;
                assert_eq!(host.base().work_used() - before, extra);
                drop(source);
            } else {
                assert_eq!(result.err().unwrap().code, ErrorCode::ResourceLimit);
                assert_eq!(
                    host.base().work_used() - before,
                    if n { 8402 } else { 6858 }
                );
                assert!(entry.take_service_source_checked().is_err());
            }
            Ok(())
        })
        .unwrap();
    }
}
#[test]
fn every_existing_display_golden_uses_one_owned_source_and_same_captured_caller() {
    for r in rows() {
        let noise = r.get("mode").and_then(|v| v.as_str()) == Some("Noise");
        let host = Host::new(noise);
        let wire = bytes(&r);
        let expected = unhex(r.get("rgba_hex").unwrap().as_str().unwrap());
        host.run(|| {
            let source = load(&host, &wire)?;
            let required = caller::RequiredCaller8::capture(&host.request())?;
            let before = host.base().work_used();
            let mut view =
                source
                    .begin_view_checked(&required, &host.originals(), &host.frame(), &|| Ok(()))?;
            assert_eq!(
                host.base().work_used() - before,
                if cfg!(target_pointer_width = "64") {
                    11634
                } else {
                    9922
                }
            );
            assert_same(&required, &view.caller);
            for _ in 0..2 {
                let prior = host.base().work_used();
                let noise_prior = match &required {
                    caller::RequiredCaller8::Noise { selected, .. } => Some(selected.used()?.noise),
                    _ => None,
                };
                let image =
                    view.render_patch_checked(&source, &host.frame(), &display(&r), &|| Ok(()))?;
                assert_eq!(image.rgba(), expected.as_slice());
                assert!(host.base().work_used() > prior);
                assert_same(&required, &view.caller);
                if let (Some(prior), caller::RequiredCaller8::Noise { selected, .. }) =
                    (noise_prior, &required)
                {
                    assert!(selected.used()?.noise > prior);
                }
                drop(image);
            }
            assert_eq!(wire, bytes(&r));
            drop(view);
            assert_eq!(host.h.footprint().live, 0);
            drop(source);
            Ok(())
        })
        .unwrap();
        for s in [
            &host.p, &host.m, &host.a, &host.c, &host.k, &host.f, &host.o,
        ] {
            assert_eq!(s.footprint().live, 0);
        }
    }
}
#[test]
fn original_e_h_equality_and_declared_union_refuse_before_view_admission() {
    let host = Host::new(false);
    let wire = bytes(&row("identity-full"));
    host.run(|| {
        let source = load(&host, &wire)?;
        let required = caller::RequiredCaller8::capture(&host.request())?;
        for (e, h) in [(1048575, 16384), (1048576, 16383)] {
            let originals = ServiceOriginals8 {
                encoded_capacity: e,
                control_capacity: h,
                ..host.originals()
            };
            assert_eq!(
                source
                    .begin_view_checked(&required, &originals, &host.frame(), &|| Ok(()))
                    .err()
                    .unwrap()
                    .code,
                ErrorCode::ResourceLimit
            );
            assert_eq!(host.h.footprint().live, 0);
        }
        let mut frame = host.frame();
        frame.caps.output = 16 << 20;
        assert_eq!(
            source
                .begin_view_checked(&required, &host.originals(), &frame, &|| Ok(()))
                .err()
                .unwrap()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(host.h.footprint().live, 0);
        assert_eq!(host.f.footprint().live, 0);
        Ok(())
    })
    .unwrap();
}
#[test]
fn typed_patch_cancellation_is_terminal_and_keeps_original_paid_counter() {
    let host = Host::new(false);
    let r = row("identity-full");
    let wire = bytes(&r);
    host.run(|| {
        let source = load(&host, &wire)?;
        let required = caller::RequiredCaller8::capture(&host.request())?;
        let mut view =
            source.begin_view_checked(&required, &host.originals(), &host.frame(), &|| Ok(()))?;
        let before = host.base().work_used();
        let calls = Cell::new(0);
        let error = view
            .render_patch_checked(&source, &host.frame(), &display(&r), &|| {
                calls.set(calls.get() + 1);
                Err(Error::new(ErrorCode::OperationCancelled, "bridge-cancel"))
            })
            .err()
            .unwrap();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.message, "bridge-cancel");
        assert_eq!(calls.get(), 1);
        assert_eq!(
            host.base().work_used() - before,
            if cfg!(target_pointer_width = "64") {
                8256
            } else {
                7120
            }
        );
        let used = host.base().work_used();
        assert!(
            view.render_patch_checked(&source, &host.frame(), &display(&r), &|| Ok(()))
                .is_err()
        );
        assert_eq!(host.base().work_used(), used);
        assert_eq!(host.o.footprint().live, 0);
        Ok(())
    })
    .unwrap();
}
#[test]
fn source_and_view_factories_survive_entry_and_host_then_image_retires_last() {
    let host = Host::new(false);
    let r = row("identity-full");
    let wire = bytes(&r);
    let budget = host.budget.clone();
    let (source, view, image) = host
        .run(|| {
            let source = load(&host, &wire)?;
            let required = caller::RequiredCaller8::capture(&host.request())?;
            let mut view =
                source
                    .begin_view_checked(&required, &host.originals(), &host.frame(), &|| Ok(()))?;
            let image =
                view.render_patch_checked(&source, &host.frame(), &display(&r), &|| Ok(()))?;
            Ok((source, view, image))
        })
        .unwrap();
    drop(host);
    assert_eq!(budget.usage().reserved, 15 * 1048576 + 20480);
    drop(source);
    assert_eq!(budget.usage().reserved, 7 * 1048576 + 16384);
    drop(view);
    assert_eq!(budget.usage().reserved, 2 * 1048576);
    assert_eq!(
        image.rgba(),
        unhex(r.get("rgba_hex").unwrap().as_str().unwrap()).as_slice()
    );
    drop(image);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn transfer_cancel_precedes_every_owner_move_and_retry_keeps_original_debt() {
    let host = Host::new(false);
    let wire = bytes(&row("identity-full"));
    host.run(|| {
        let armed = Cell::new(false);
        let calls = Cell::new(0);
        let check = || {
            if armed.get() {
                calls.set(calls.get() + 1);
                return Err(Error::new(ErrorCode::OperationCancelled, "transfer-cancel"));
            }
            Ok(())
        };
        let mut entry = entry::Entry8::new(host.request(), host.realms(), &check)?;
        entry.read_into_slot_checked(&wire)?;
        let c = host.c.footprint().live;
        let before = host.base().work_used();
        armed.set(true);
        let error = entry.take_service_source_checked().err().unwrap();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.message, "transfer-cancel");
        assert_eq!(calls.get(), 1);
        assert_eq!(host.c.footprint().live, c);
        assert_eq!(
            host.base().work_used() - before,
            if cfg!(target_pointer_width = "64") {
                8402
            } else {
                6858
            }
        );
        armed.set(false);
        let prior = host.base().work_used();
        let source = entry.take_service_source_checked()?;
        assert_eq!(
            host.base().work_used() - prior,
            if cfg!(target_pointer_width = "64") {
                9202
            } else {
                7658
            }
        );
        drop(entry);
        assert_eq!(host.c.footprint().live, c);
        drop(source);
        Ok(())
    })
    .unwrap();
}
#[test]
fn exact_small_h_original_is_refused_without_admitting_or_switching_factory() {
    let mut host = Host::new(false);
    host.h = host
        .budget
        .reserve_capacity(4095)
        .unwrap()
        .scope(paintcraft_core::types::resources::AllocationClass::SharedBudget);
    let wire = bytes(&row("identity-full"));
    host.run(|| {
        let source = load(&host, &wire)?;
        let required = caller::RequiredCaller8::capture(&host.request())?;
        let originals = ServiceOriginals8 {
            control_capacity: 4095,
            ..host.originals()
        };
        assert_eq!(
            source
                .begin_view_checked(&required, &originals, &host.frame(), &|| Ok(()))
                .err()
                .unwrap()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(host.h.footprint().live, 0);
        assert_eq!(host.h.capacity_snapshot().ceiling, 4095);
        Ok(())
    })
    .unwrap();
}
#[test]
fn nonzero_complete_prepared_owner_remains_same_across_successful_identity_patches() {
    use paintcraft_core::types::{
        LayerBlendMode, Rgba,
        filters::{Descriptor, LevelsRgbV1},
        ids::deterministic_id,
    };
    use paintcraft_document::{FilterInstance, FilterStack};
    use paintcraft_render::{blend::service, spatial::with_workspace};
    let host = Host::new(false);
    let mut doc = paintcraft_document::new_document(
        1,
        1,
        Some(Rgba([32, 64, 96, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.blend_mode = LayerBlendMode::Multiply;
    layer.filters = FilterStack::new(vec![FilterInstance::new(
        deterministic_id(&["owned-bridge-identity"]),
        true,
        Descriptor::LevelsRgbV1(LevelsRgbV1::default()),
    )])
    .unwrap();
    host.run(|| {
        let check = || Ok(());
        let spend = |n| host.base().reserve_work(n);
        let fact = paintcraft_formats::pcnd::count_portable_blend_snapshot_rooted_checked(
            &doc, &host.e, &check, &spend,
        )?;
        let bytes = fact.encode()?;
        let source = load(&host, bytes.as_bytes())?;
        let programs = service::prepare_programs_checked(
            &doc,
            service::Caller::Base(host.base()),
            &check,
            &spend,
        )?;
        assert!(programs.retained_bytes() > 0);
        with_workspace(programs.workspace().clone(), || {
            let required = caller::RequiredCaller8::capture(&host.request())?;
            let caller::RequiredCaller8::Base(base) = &required else {
                panic!("expected Base");
            };
            let union = base.prepared_owner_union_bytes(None)?;
            assert!(union > 0);
            let mut view =
                source.begin_view_checked(&required, &host.originals(), &host.frame(), &check)?;
            let display = Source8DisplayPatch {
                visible: paintcraft_core::types::Rect::new(0, 0, 1, 1).unwrap(),
                level: 0,
                patch: paintcraft_core::types::Rect::new(0, 0, 1, 1).unwrap(),
            };
            for _ in 0..2 {
                let image = view.render_patch_checked(&source, &host.frame(), &display, &check)?;
                assert_eq!(image.rgba(), [32, 64, 96, 255]);
                assert_same(&required, &view.caller);
                let caller::RequiredCaller8::Base(captured) = &view.caller else {
                    panic!("expected Base");
                };
                assert_eq!(captured.prepared_owner_union_bytes(None)?, union);
                drop(image);
            }
            Ok(())
        })?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn foreign_content_refuses_before_frame_allocation_and_terminally_fails_view() {
    let mut host = Host::new(false);
    let two_sources_k = source8_reader_control_bytes()
        .unwrap()
        .retained_carriers
        .checked_mul(2)
        .unwrap();
    // Both retained sources share this original job; K funds both carrier pairs.
    host.k = host
        .budget
        .reserve_capacity(two_sources_k)
        .unwrap()
        .scope(paintcraft_core::types::resources::AllocationClass::SharedBudget);
    let r = row("identity-full");
    host.run(|| {
        let load_source = |wire: &[u8]| -> Result<ServiceSource8> {
            let check = || Ok(());
            let mut realms = host.realms();
            realms.caps.k = two_sources_k;
            let mut entry = entry::Entry8::new(host.request(), realms, &check)?;
            entry.read_into_slot_checked(wire)?;
            entry.take_service_source_checked()
        };
        let source = load_source(&bytes(&r))?;
        let other = load_source(&bytes(&row("spatial-65-clamp-radius1-display")))?;
        let required = caller::RequiredCaller8::capture(&host.request())?;
        let mut view =
            source.begin_view_checked(&required, &host.originals(), &host.frame(), &|| Ok(()))?;
        let before = host.base().work_used();
        let e = view
            .render_patch_checked(&other, &host.frame(), &display(&r), &|| Ok(()))
            .err()
            .unwrap();
        assert_eq!(e.code, ErrorCode::ConflictStaleBase);
        assert_eq!(
            host.base().work_used() - before,
            if cfg!(target_pointer_width = "64") {
                8256
            } else {
                7120
            }
        );
        assert_eq!(host.f.footprint().live, 0);
        assert_eq!(host.o.footprint().live, 0);
        assert!(
            view.render_patch_checked(&source, &host.frame(), &display(&r), &|| Ok(()))
                .is_err()
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn pre_h_checks_all_actual_frame_originals_on_same_tightened_counter() {
    use paintcraft_render::spatial::capture_required_workspace;
    let host = Host::new(false);
    let wire = bytes(&row("identity-full"));
    host.run(|| {
        let source = load(&host, &wire)?;
        let tight = capture_required_workspace(12 << 20, 268435456)?;
        assert!(host.base().same_job(&tight));
        assert_eq!(tight.bytes_limit(), 12 << 20);
        let required = caller::RequiredCaller8::Base(tight);
        for (field, value, index) in [
            (0, 0, 1),
            (0, 4194303, 1),
            (1, 0, 2),
            (1, 16383, 2),
            (2, 0, 3),
            (2, 2097151, 3),
            (3, 0, 1),
            (4, 0, 4),
        ] {
            let mut frame = host.frame();
            match field {
                0 => frame.caps.workspace = value,
                1 => frame.caps.control = value,
                2 => frame.caps.output = value,
                3 => {
                    frame.caps.workspace = 0;
                    frame.caps.control = 0;
                    frame.caps.output = 0;
                }
                _ => {}
            }
            let before = host.base().work_used();
            let checks = Cell::new(0);
            let error = source
                .begin_view_checked(&required, &host.originals(), &frame, &|| {
                    checks.set(checks.get() + 1);
                    Ok(())
                })
                .err()
                .unwrap();
            assert_eq!(error.code, ErrorCode::ResourceLimit);
            let expected = if cfg!(target_pointer_width = "64") {
                match index {
                    1 => 10834,
                    2 => 10946,
                    3 => 11058,
                    _ => 11122,
                }
            } else {
                match index {
                    1 => 9122,
                    2 => 9234,
                    3 => 9346,
                    _ => 9410,
                }
            };
            assert_eq!(host.base().work_used() - before, expected);
            assert_eq!(checks.get(), 0);
            assert_eq!(host.h.footprint().live, 0);
            assert_eq!(host.f.footprint().live, 0);
            assert_eq!(host.o.footprint().live, 0);
        }
        Ok(())
    })
    .unwrap();
}
