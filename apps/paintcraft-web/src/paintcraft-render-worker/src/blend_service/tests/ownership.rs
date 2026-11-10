use super::*;
use std::cell::RefCell;
fn view(s: &mut LayerBlendService, base: &[u8; WINDOW], request: u32, token: u64) {
    let o = s.open.unwrap();
    command(
        s,
        base,
        2,
        request,
        (token, token, [0, 0, o.canvas[0], o.canvas[1]], 0, 0),
    );
    assert_eq!(s.begin_view().unwrap(), 2);
}
#[test]
fn original_full_e_component_denial_before_vector_keeps_actual_diagnostic_budget() {
    let mut s = LayerBlendService::new().unwrap();
    let row = rows().remove(0);
    let p = packet(&row, &s);
    let open = Command8::decode(&p).unwrap().open;
    let e = roles::reserve(&s.budget, open.roles[E]).unwrap();
    let w = roles::reserve(&s.budget, open.roles[W]).unwrap();
    let dispatch = StandaloneDispatch8 { kind: 1 };
    let job = OperationJob8::new(&dispatch, &open, &w, &s.h).unwrap();
    job.spend(s.controls.stage_formation_work().unwrap())
        .unwrap();
    let mut held = e.account();
    held.resize(open.roles[E]).unwrap();
    let error = Stage8::new(
        open.bytes,
        &e,
        &s.h,
        s.controls.stage_named().unwrap(),
        &job,
        &|| Ok(()),
    )
    .err()
    .unwrap();
    let denial = error.allocation_denial().unwrap();
    let before = denial.budget().usage().reserved;
    let one = s.budget.reserve_capacity(1).unwrap();
    assert_eq!(denial.budget().usage().reserved, before + 1);
    drop(one);
    assert_eq!(denial.budget().usage().reserved, before);
    drop(held);
    drop(job);
    drop(w);
    drop(e);
    let mut empty = [0; WINDOW];
    empty[..4].copy_from_slice(b"PC8W");
    empty[4] = 1;
    empty[6] = 5;
    put32(&mut empty, 8, 1);
    s.window = empty;
    assert_eq!(s.close().unwrap(), 6);
}
#[test]
fn stale_foreign_duplicate_ack_never_advances_or_replaces_current_job() {
    let row = rows().remove(0);
    let mut s = LayerBlendService::new().unwrap();
    let p = load(&mut s, &row);
    view(&mut s, &p, 2, 1);
    s.step().unwrap();
    let paid = s.work_used(false);
    let o = s.open.unwrap();
    let visible = [0, 0, o.canvas[0], o.canvas[1]];
    for which in 0..3 {
        command(&mut s, &p, 3, 3, (1, 1, visible, 0, 0));
        match which {
            0 => s.window[16] ^= 1,
            1 => put64(&mut s.window, 304, 9),
            _ => put32(&mut s.window, 340, 1),
        }
        let live = s.roles.as_ref().unwrap().o.footprint().live;
        assert!(s.ack().is_err());
        assert_eq!(s.phase, Phase8::AwaitAck);
        assert_eq!(s.view.as_ref().unwrap().ordinal, 0);
        assert_eq!(s.work_used(false), paid);
        assert_eq!(s.roles.as_ref().unwrap().o.footprint().live, live);
    }
    command(&mut s, &p, 3, 3, (1, 1, visible, 0, 0));
    assert_eq!(s.ack().unwrap(), 4);
    assert!(s.ack().is_err());
    close(s, &p, 4);
}
#[test]
fn cancelled_token_cannot_renew_and_strict_new_view_has_distinct_operation_debt() {
    let row = rows().remove(0);
    let mut s = LayerBlendService::new().unwrap();
    let p = load(&mut s, &row);
    view(&mut s, &p, 2, 1);
    let initial = s.work_used(false);
    s.step().unwrap();
    let first = s.work_used(false);
    assert!(first > initial);
    let o = s.open.unwrap();
    let visible = [0, 0, o.canvas[0], o.canvas[1]];
    command(&mut s, &p, 4, 3, (1, 1, visible, 0, 0));
    assert_eq!(s.cancel().unwrap(), 5);
    assert_eq!(s.work_used(false), first);
    assert!(s.image.is_none() && s.view.is_none());
    command(&mut s, &p, 2, 4, (1, 2, visible, 0, 0));
    assert!(s.begin_view().is_err());
    assert_eq!(s.work_used(false), first);
    view(&mut s, &p, 4, 2);
    assert_eq!(s.work_used(false), initial);
    s.step().unwrap();
    assert_eq!(s.work_used(false), first);
    close(s, &p, 5);
}
#[test]
fn original_callback_after_o_admission_moves_error_and_pays_without_output_escape() {
    let row = rows().remove(0);
    let mut s = LayerBlendService::new().unwrap();
    let p = load(&mut s, &row);
    view(&mut s, &p, 2, 1);
    let output = s.roles.as_ref().unwrap().o.clone();
    let pending = RefCell::new(Some(
        Error::new(
            ErrorCode::OperationCancelled,
            "original service callback".repeat(41),
        )
        .at(23),
    ));
    let pointer = pending.borrow().as_ref().unwrap().message.as_ptr();
    let before = s.work_used(false);
    let check = || {
        if output.footprint().live > 0 {
            match pending.borrow_mut().take() {
                Some(e) => Err(e),
                None => Ok(()),
            }
        } else {
            Ok(())
        }
    };
    let error = s.step_checked(&check).unwrap_err();
    assert_eq!(error.message.as_ptr(), pointer);
    assert_eq!(error.operation_index, Some(23));
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert!(s.work_used(false) > before);
    assert_eq!(output.footprint().live, 0);
    assert_eq!(s.phase, Phase8::FailedView);
    assert!(s.step().is_err());
    drop(output);
    close(s, &p, 3);
}
#[test]
fn real_f_denial_retains_original_budget_and_source_then_close_retires_all() {
    let row = rows().remove(0);
    let mut s = LayerBlendService::new().unwrap();
    let p = load(&mut s, &row);
    view(&mut s, &p, 2, 1);
    let mut held = s.roles.as_ref().unwrap().f.account();
    held.resize(16384).unwrap();
    let error = s.step().unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    let denial = error.allocation_denial().unwrap();
    let before = denial.budget().usage().reserved;
    let one = s.budget.reserve_capacity(1).unwrap();
    assert_eq!(denial.budget().usage().reserved, before + 1);
    drop(one);
    assert_eq!(denial.budget().usage().reserved, before);
    assert!(s.source.is_some());
    assert!(s.image.is_none());
    drop(held);
    close(s, &p, 3);
}
#[test]
fn original_e_factory_capacity_survives_encoded_vec_retirement_and_scope_close() {
    let row = rows().remove(0);
    let mut s = LayerBlendService::new().unwrap();
    let p = load(&mut s, &row);
    let roles = s.roles.as_ref().unwrap();
    assert_eq!(roles.encoded.footprint().live, 0);
    let snapshot = roles.encoded.capacity_snapshot();
    assert!(snapshot.reserved);
    assert_eq!(snapshot.ceiling, 1 << 20);
    let budget = s.budget.clone();
    view(&mut s, &p, 2, 1);
    s.step().unwrap();
    assert!(s.image.is_some());
    close(s, &p, 3);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn exact_original_noise_w_funds_resident_sidecar_without_whole_caller_grant() {
    use paintcraft_render::noise_work::NoiseAdmission;
    let row = rows().remove(0);
    for short in [0, 1] {
        let s = LayerBlendService::new().unwrap();
        let mut p = packet(&row, &s);
        p[7] = 5;
        let side = NoiseAdmission::control_quote().unwrap().counter_bytes;
        put64(&mut p, 192, 1048576);
        put64(&mut p, 200, side);
        put64(&mut p, 208 + W * 8, side - short);
        let open = Command8::decode(&p).unwrap().open;
        let w = roles::reserve(&s.budget, side - short).unwrap();
        let cap = w.capacity_snapshot();
        let dispatch = StandaloneDispatch8 { kind: 1 };
        match OperationJob8::new(&dispatch, &open, &w, &s.h) {
            Ok(job) => {
                assert_eq!(short, 0);
                assert_eq!(w.footprint().live, side);
                assert_eq!(w.capacity_snapshot().ceiling, side);
                let job::Job8::Noise(noise) = &job.job else {
                    panic!("Noise root")
                };
                assert_eq!(noise.workspace().bytes_limit(), JOB);
                assert_eq!(noise.workspace().scope().capacity_snapshot().ceiling, side);
                let mut extra = w.account();
                assert_eq!(extra.resize(1).unwrap_err().code, ErrorCode::ResourceLimit);
                drop(extra);
                drop(job);
                assert_eq!(w.footprint().live, 0);
            }
            Err(error) => {
                assert_eq!(short, 1);
                assert_eq!(error.code, ErrorCode::ResourceLimit);
                assert!(error.allocation_denial().is_some());
                assert_eq!(w.footprint().live, 0);
            }
        }
        assert_eq!(w.capacity_snapshot(), cap);
        drop(w);
        drop(s);
        assert_eq!(LayerBlendService::live_instances(), 0);
    }
}
#[test]
fn entire_declared_64m_passes_real_read_and_frame_while_one_more_refuses_preconstructor() {
    let row = rows().remove(0);
    for extra in [0, 1] {
        let mut s = LayerBlendService::new().unwrap();
        let mut p = packet(&row, &s);
        let o = Command8::decode(&p).unwrap().open;
        let other = o
            .roles
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != M)
            .map(|(_, n)| *n)
            .sum::<u64>();
        put64(&mut p, 208 + M * 8, JOB - other + extra);
        s.window = p;
        if extra == 1 {
            let before = s.budget.usage();
            let activity = s.budget.activity();
            assert_eq!(s.stage().unwrap_err().code, ErrorCode::ResourceLimit);
            assert_eq!(s.budget.usage(), before);
            assert_eq!(s.budget.activity(), activity);
            drop(s);
            continue;
        }
        assert_eq!(s.stage().unwrap(), 0);
        s.stage
            .as_mut()
            .unwrap()
            .data
            .copy_from_slice(&unhex(text(&row, "hex")));
        assert_eq!(s.seal().unwrap(), 1);
        view(&mut s, &p, 2, 1);
        assert_eq!(s.step().unwrap(), 3);
        assert_eq!(
            s.image.as_ref().unwrap().rgba(),
            unhex(text(&row, "rgba_hex"))
        );
        close(s, &p, 3);
    }
}

#[test]
fn exact_encoded_data_capacity_uses_h_for_stager_and_keeps_original_e_factory() {
    let row = rows().remove(0);
    let mut s = LayerBlendService::new().unwrap();
    let mut p = packet(&row, &s);
    let wire = unhex(text(&row, "hex"));
    put64(&mut p, 208 + E * 8, wire.len() as u64);
    s.window = p;
    assert_eq!(s.stage().unwrap(), 0);
    assert_eq!(s.stage.as_ref().unwrap().data.capacity(), wire.len());
    assert_eq!(
        s.roles.as_ref().unwrap().encoded.footprint().live,
        wire.len() as u64
    );
    s.stage.as_mut().unwrap().data.copy_from_slice(&wire);
    assert_eq!(s.seal().unwrap(), 1);
    assert_eq!(
        s.roles
            .as_ref()
            .unwrap()
            .encoded
            .capacity_snapshot()
            .ceiling,
        wire.len() as u64
    );
    assert_eq!(s.roles.as_ref().unwrap().encoded.footprint().live, 0);
    close(s, &p, 2);
}

#[test]
fn private_stager_owner_keeps_data_accounts_and_original_factories_after_host_drop() {
    let row = rows().remove(0);
    let mut s = LayerBlendService::new().unwrap();
    let p = packet(&row, &s);
    s.window = p;
    s.stage().unwrap();
    let wire = unhex(text(&row, "hex"));
    s.stage.as_mut().unwrap().data.copy_from_slice(&wire);
    let budget = s.budget.clone();
    let h = s.controls.total().unwrap();
    let stage = s.stage.take().unwrap();
    drop(s);
    assert_eq!(stage.data, wire);
    assert_eq!(budget.usage().reserved, (1 << 20) + h);
    drop(stage);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(LayerBlendService::live_instances(), 0);
}
