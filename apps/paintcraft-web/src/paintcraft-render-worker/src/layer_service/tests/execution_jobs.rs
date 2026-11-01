//! Actual service controls, authored only; Root owns all execution qualification.
use super::*;
use paintcraft_render::{
    noise_work::{NoiseAdmission, NoiseJobPolicy, NoiseLimits, with_noise_workspace},
    spatial::{WorkspaceAdmission, with_workspace},
};
fn render(service: &mut LayerAppearanceService) {
    for _ in 0..1024 {
        if service.step().unwrap() {
            return;
        }
    }
    panic!("Source7 bounded view did not complete");
}
fn level(value: &mut Value, n: i64) {
    let mut view = field(value, "view").unwrap().clone();
    set(&mut view, "level", Value::Int(n));
    set(value, "view", view);
}
fn exhaust(job: &OpenJob) {
    let used = job.workspace().work_used();
    job.workspace()
        .reserve_work(job.workspace().work_limit() - used)
        .unwrap();
}
#[test]
fn captured_base_and_noise_parents_remain_cumulative_after_success_and_tls_exit() {
    for noise in [false, true] {
        let (bytes, c) = source(&fixture(128, true), admission(noise));
        let budget = SharedBudget::new(64 << 20);
        let scope = budget.scope(AllocationClass::SharedBudget);
        let (mut service, parent) = if noise {
            let p = NoiseAdmission::new_root(
                scope,
                NoiseJobPolicy::BrowserV1,
                NoiseLimits {
                    bytes: 64 << 20,
                    base_work: 268435456,
                    noise_work: 1 << 35,
                },
            )
            .unwrap();
            (
                with_noise_workspace(p.clone(), || {
                    LayerAppearanceService::open(&bytes, &text(&c))
                })
                .unwrap(),
                OpenJob::Noise(p),
            )
        } else {
            let p = WorkspaceAdmission::new(scope, 64 << 20, 268435456);
            (
                with_workspace(p.clone(), || {
                    LayerAppearanceService::open(&bytes, &text(&c))
                })
                .unwrap(),
                OpenJob::Base(p),
            )
        };
        let req = request(&service, 1, true, Value::Null);
        service.prepare(&text(&req)).unwrap();
        render(&mut service);
        drop(service.take_frame().unwrap());
        assert_eq!(parent.used().unwrap(), service.state.job.used().unwrap());
        exhaust(&parent);
        let mut req = request(&service, 2, true, Value::Null);
        level(&mut req, 1);
        assert_eq!(
            service.prepare(&text(&req)).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(parent.used().unwrap(), service.state.job.used().unwrap());
        if let (OpenJob::Noise(a), OpenJob::Noise(b)) = (&parent, &service.state.job) {
            assert!(a.same_job(b));
        }
    }
}
#[test]
fn cancelled_and_denied_same_intent_do_not_refresh_but_valid_changed_view_can() {
    let mut service = open(&fixture(128, true), false);
    service
        .prepare(&text(&request(&service, 1, true, Value::Null)))
        .unwrap();
    service.cancel(1).unwrap();
    let old = service.state.job.clone();
    exhaust(&old);
    for generation in 2..=3 {
        let req = request(&service, generation, true, Value::Null);
        assert_eq!(
            service.prepare(&text(&req)).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(service.state.job.used().unwrap(), old.used().unwrap());
    }
    let mut changed = request(&service, 4, true, Value::Null);
    level(&mut changed, 1);
    service.prepare(&text(&changed)).unwrap();
    render(&mut service);
    drop(service.take_frame().unwrap());
    assert!(service.state.job.workspace().work_used() < old.workspace().work_used());
    // Every replacement carries at least the actual original OPEN setup debit.
    assert!(service.state.job.workspace().work_used() > 0);
}
#[test]
fn malformed_stale_and_quoteonly_attempts_keep_debt_without_new_execution_owner() {
    let mut service = open(&fixture(128, true), false);
    let before = service.state.job.used().unwrap();
    assert!(service.prepare("{").is_err());
    let malformed = service.state.execution.pending_work();
    assert!(malformed > 0);
    let req = request(&service, 1, false, Value::Null);
    service.prepare(&text(&req)).unwrap();
    let first = service.state.execution.pending_work();
    assert!(first > malformed);
    assert!(service.prepare(&text(&req)).is_err());
    assert!(service.state.execution.pending_work() > first);
    assert_eq!(service.state.job.used().unwrap(), before);
    assert!(service.state.frame.is_none());
    assert!(!service.state.job.workspace().has_prepared_programs());
    let mut wrong = request(&service, 2, true, Value::Null);
    let mut owner = field(&wrong, "owner").unwrap().clone();
    set(&mut owner, "epoch", Value::str("999"));
    set(&mut wrong, "owner", owner);
    assert!(service.prepare(&text(&wrong)).is_err());
    assert_eq!(service.state.job.used().unwrap(), before);
    let mut unsupported = request(&service, 3, true, Value::Null);
    set(
        &mut unsupported,
        "operation",
        Value::object([("op", Value::str("layer.translate"))]),
    );
    set(
        &mut unsupported,
        "target",
        Value::str(service.state.base.roots[0].to_string()),
    );
    assert_eq!(
        service.prepare(&text(&unsupported)).unwrap_err().code,
        ErrorCode::CapabilityUnsupported
    );
    assert_eq!(service.state.job.used().unwrap(), before);
    let mut wrong_fact = request(&service, 4, true, Value::Null);
    let mut union = field(&wrong_fact, "unionRequirements").unwrap().clone();
    set(&mut union, "capabilitySha256", Value::str("00".repeat(32)));
    set(&mut wrong_fact, "unionRequirements", union);
    assert!(service.prepare(&text(&wrong_fact)).is_err());
    assert_eq!(service.state.job.used().unwrap(), before);
    // Debt is bounded and does not reset just because parsing failed.
    let debt = service.state.execution.pending_work();
    service
        .state
        .execution
        .spend(&service.state.job, u64::MAX)
        .unwrap_err();
    assert!(service.state.execution.pending_work() >= debt);
    let next = request(&service, 5, true, Value::Null);
    assert_eq!(
        service.prepare(&text(&next)).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(service.state.job.used().unwrap(), before);
}
#[test]
fn held_native_packet_blocks_handover_and_cancel_cannot_reuse_success_token() {
    let mut service = open(&fixture(128, true), false);
    service
        .prepare(&text(&request(&service, 1, true, Value::Null)))
        .unwrap();
    render(&mut service);
    let packet = service.take_frame().unwrap();
    let old = service.state.job.clone();
    exhaust(&old);
    let next = request(&service, 2, true, Value::Null);
    assert_eq!(
        service.prepare(&text(&next)).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(service.state.job.used().unwrap(), old.used().unwrap());
    drop(packet);
    // Actual cancellation after delivery revokes the success token.
    service.cancel(1).unwrap();
    assert_eq!(
        service.prepare(&text(&next)).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(service.state.job.used().unwrap(), old.used().unwrap());
}
#[test]
fn noise_root_control_denial_is_same_scope_and_does_not_offer_retry_mint() {
    let mut service = open(&fixture(128, true), true);
    service
        .prepare(&text(&request(&service, 1, true, Value::Null)))
        .unwrap();
    render(&mut service);
    drop(service.take_frame().unwrap());
    service.retire();
    let old = service.state.job.clone();
    exhaust(&old);
    let scope = old.workspace().scope();
    let free = old.workspace().bytes_limit() - scope.footprint().live;
    let mut filler = scope.account();
    filler.resize(free).unwrap();
    let next = request(&service, 2, true, Value::Null);
    assert_eq!(
        service.prepare(&text(&next)).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    drop(filler);
    let retry = request(&service, 3, true, Value::Null);
    assert_eq!(
        service.prepare(&text(&retry)).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let (OpenJob::Noise(a), OpenJob::Noise(b)) = (&old, &service.state.job) else {
        panic!("paired")
    };
    assert!(a.same_job(b));
}

#[test]
fn transferred_captured_authority_with_no_worker_tls_never_becomes_standalone() {
    for noise in [false, true] {
        let mut a = admission(noise);
        a.work_authority = metadata::Origin7::CapturedJob;
        let (bytes, c) = source(&fixture(128, true), a);
        // Intentionally no installed Rust parent: wire origin carries the captured grant.
        let mut service = LayerAppearanceService::open(&bytes, &text(&c)).unwrap();
        service
            .prepare(&text(&request(&service, 1, true, Value::Null)))
            .unwrap();
        render(&mut service);
        drop(service.take_frame().unwrap());
        let old = service.state.job.clone();
        exhaust(&old);
        let mut changed = request(&service, 2, true, Value::Null);
        level(&mut changed, 1);
        assert_eq!(
            service.prepare(&text(&changed)).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(old.used().unwrap(), service.state.job.used().unwrap());
        if let (OpenJob::Noise(a), OpenJob::Noise(b)) = (&old, &service.state.job) {
            assert!(a.same_job(b));
        }
    }
}

#[test]
fn changing_only_duplicate_alias_cannot_refresh_cancelled_semantic_intent() {
    let mut doc = fixture(128, true);
    let id = deterministic_id(&["view job duplicate alias"]);
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            id,
            false,
            Descriptor::InvertRgbV1,
        )])
        .unwrap();
    let mut service = open(&doc, false);
    for (generation, alias) in [(1, "first"), (2, "different-spelling")] {
        let op = Value::object([
            ("op", Value::str("filter.duplicate")),
            (
                "layer_ref",
                Value::object([("id", Value::str(doc.roots[0].to_string()))]),
            ),
            (
                "filter_ref",
                Value::object([("id", Value::str(id.to_string()))]),
            ),
            ("ref", Value::str(alias)),
        ]);
        let req = request(&service, generation, true, op);
        if generation == 1 {
            service.prepare(&text(&req)).unwrap();
            service.cancel(1).unwrap();
            exhaust(&service.state.job);
        } else {
            assert_eq!(
                service.prepare(&text(&req)).unwrap_err().code,
                ErrorCode::ResourceLimit
            );
            assert_eq!(
                service.state.job.workspace().work_used(),
                service.state.job.workspace().work_limit()
            );
        }
    }
}

#[test]
fn equivalent_lod_aligned_view_spelling_cannot_refresh_cancelled_job() {
    let mut service = open(&fixture(128, true), false);
    let mut first = request(&service, 1, true, Value::Null);
    level(&mut first, 1);
    service.prepare(&text(&first)).unwrap();
    service.cancel(1).unwrap();
    exhaust(&service.state.job);
    let mut second = request(&service, 2, true, Value::Null);
    set(
        &mut second,
        "view",
        Value::object([
            (
                "region",
                Value::Array(vec![
                    Value::Int(1),
                    Value::Int(0),
                    Value::Int(1),
                    Value::Int(1),
                ]),
            ),
            ("level", Value::Int(1)),
        ]),
    );
    assert_eq!(
        service.prepare(&text(&second)).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        service.state.job.workspace().work_used(),
        service.state.job.workspace().work_limit()
    );
}
