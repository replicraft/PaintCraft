use super::*;
#[test]
fn compiled_discovery_is_pure_exact_factory_inventory_and_canonical_profile() {
    let b = SharedBudget::new(1000);
    let usage = b.usage();
    let activity = b.activity();
    let c = CompiledControls8::query().unwrap();
    assert_eq!(
        c.total().unwrap(),
        c.service_bytes + c.stage_bytes + c.view_bytes
    );
    assert!(c.total().unwrap() < JOB);
    assert!(c.view_bytes > 4096);
    assert_eq!(control_fact(0).unwrap(), c.service_bytes);
    assert_eq!(control_fact(1).unwrap(), c.stage_bytes);
    assert_eq!(control_fact(2).unwrap(), c.view_bytes);
    assert!(control_fact(3).is_err());
    assert!(runtime_fact(19).is_err());
    assert!(profile_word(8).is_err());
    let mut bytes = [0; 32];
    for i in 0..8 {
        bytes[i * 4..i * 4 + 4].copy_from_slice(&profile_word(i as u32).unwrap().to_le_bytes());
    }
    assert_eq!(bytes, c.profile());
    assert_eq!(b.usage(), usage);
    assert_eq!(b.activity(), activity);
    assert_eq!(LayerBlendService::live_instances(), 0);
    let future = 10
        * paintcraft_core::types::resources::BudgetReservation::control_allocation_quote().unwrap()
        + 8 * BudgetScope::control_allocation_quote().unwrap();
    assert_eq!(
        crate::blend_service::controls::future_factory_allocations().unwrap(),
        future
    );
}
#[test]
fn constructor_singleton_and_empty_genuine_close() {
    let mut first = LayerBlendService::new().unwrap();
    assert_eq!(LayerBlendService::live_instances(), 1);
    assert!(LayerBlendService::new().is_err());
    assert!(!first.has_open_manifest());
    let mut empty = [0; WINDOW];
    empty[..4].copy_from_slice(b"PC8W");
    empty[4] = 1;
    empty[6] = 5;
    put32(&mut empty, 8, 1);
    first.window = empty;
    let b = first.budget.clone();
    assert_eq!(first.close().unwrap(), 6);
    assert_eq!(b.usage().reserved, 0);
    assert_eq!(LayerBlendService::live_instances(), 0);
    drop(LayerBlendService::new().unwrap());
    assert_eq!(LayerBlendService::live_instances(), 0);
}
#[test]
fn invalid_envelope_exact_h_length_and_flags_refuse_before_original_w_or_counter() {
    let row = rows().remove(0);
    for which in 0..5 {
        let mut s = LayerBlendService::new().unwrap();
        let mut p = packet(&row, &s);
        match which {
            0 => put64(&mut p, 208, JOB),
            1 => put64(&mut p, 208 + H * 8, s.controls.total().unwrap() - 1),
            2 => put64(&mut p, 208, unhex(text(&row, "hex")).len() as u64 - 1),
            3 => p[7] |= 2,
            _ => p[400] = 1,
        }
        let usage = s.budget.usage();
        let activity = s.budget.activity();
        s.window = p;
        assert!(s.stage().is_err());
        assert_eq!(s.phase, Phase8::Empty);
        assert!(!s.has_open_manifest());
        assert!(s.roles.is_none());
        assert!(s.dispatch_job.is_none());
        assert!(s.stage.is_none());
        assert_eq!(s.budget.usage(), usage);
        assert_eq!(s.budget.activity(), activity);
        drop(s);
    }
}
#[test]
fn first_counter_fee_exact_limit_denial_keeps_prefix_and_no_source_vector() {
    let row = rows().remove(0);
    let mut s = LayerBlendService::new().unwrap();
    let mut p = packet(&row, &s);
    put64(&mut p, 184, FIXED);
    s.window = p;
    assert_eq!(s.stage().unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(s.work_used(false), FIXED);
    assert_eq!(s.phase, Phase8::FailedOpen);
    assert!(
        s.stage.is_none() && s.roles.is_none() && s.source.is_none() && s.dispatch_job.is_none()
    );
    assert!(s.stage().is_err());
    assert_eq!(s.work_used(false), FIXED);
    close(s, &p, 2);
}
#[test]
fn corrupt_seal_and_declared_private_identity_are_terminal_and_data_first() {
    let row = rows().remove(0);
    for mismatch in 0..3 {
        let mut s = LayerBlendService::new().unwrap();
        let mut p = packet(&row, &s);
        if mismatch == 1 {
            p[72] ^= 1;
        }
        if mismatch == 2 {
            put32(&mut p, 168, 999);
        }
        s.window = p;
        s.stage().unwrap();
        let wire = unhex(text(&row, "hex"));
        s.stage.as_mut().unwrap().data.copy_from_slice(&wire);
        if mismatch == 0 {
            s.stage.as_mut().unwrap().data[10] ^= 1;
        }
        assert!(s.seal().is_err());
        assert_eq!(s.phase, Phase8::FailedOpen);
        assert!(s.stage.is_none() && s.source.is_none());
        let paid = s.work_used(false);
        assert!(paid > 0);
        assert!(s.seal().is_err());
        assert_eq!(s.work_used(false), paid);
        close(s, &p, 2);
    }
}
#[test]
fn ambient_some_is_refused_without_counter_replacement_or_new_role_factory() {
    use paintcraft_render::spatial::{WorkspaceAdmission, with_workspace};
    let row = rows().remove(0);
    let mut s = LayerBlendService::new().unwrap();
    s.window = packet(&row, &s);
    let other = SharedBudget::new(1024);
    let scope = other
        .reserve_capacity(64)
        .unwrap()
        .scope(paintcraft_core::types::resources::AllocationClass::PaintMetadata);
    let caller = WorkspaceAdmission::new(scope, 64, 100);
    let usage = s.budget.usage();
    let work = caller.work_used();
    with_workspace(caller.clone(), || {
        assert_eq!(
            s.stage().unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(s.phase, Phase8::Empty);
    assert_eq!(s.budget.usage(), usage);
    assert_eq!(caller.work_used(), work);
    drop(s);
}
