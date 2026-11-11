use super::*;
fn available(n: u8) -> Observation {
    Observation::Available(Identity {
        helper: [n; 32],
        manifest: [n + 1; 32],
    })
}
#[test]
fn unchanged_refresh_and_observed_aba_are_distinct() {
    let mut g = Generation::default();
    let r = g.begin_refresh().unwrap();
    let a = g.complete(r, available(1)).unwrap();
    let r = g.begin_refresh().unwrap();
    assert!(g.require_current(a).is_err());
    assert_eq!(g.complete(r, available(1)).unwrap(), a);
    g.require_current(a).unwrap();
    let r = g.begin_refresh().unwrap();
    assert!(g.complete(r, Observation::Unavailable).is_err());
    assert!(g.require_current(a).is_err());
    let r = g.begin_refresh().unwrap();
    let b = g.complete(r, available(1)).unwrap();
    assert_eq!(b.generation(), a.generation() + 2);
    assert!(g.require_current(a).is_err());
    g.require_current(b).unwrap();
}
#[test]
fn late_and_duplicate_completions_cannot_roll_back_observation() {
    let mut g = Generation::default();
    let old = g.begin_refresh().unwrap();
    let new = g.begin_refresh().unwrap();
    assert!(g.complete(old, available(1)).is_err());
    let t = g.complete(new, available(2)).unwrap();
    assert!(g.complete(new, available(1)).is_err());
    g.require_current(t).unwrap();
    let r = g.begin_refresh().unwrap();
    let changed = g.complete(r, available(3)).unwrap();
    assert!(g.require_current(t).is_err());
    g.require_current(changed).unwrap();
}
#[test]
fn exhaustion_refuses_without_recycling_epoch_or_changing_old_observation() {
    let mut g = Generation {
        requested: u64::MAX,
        ..Default::default()
    };
    assert!(g.begin_refresh().is_err());
    let mut g = Generation {
        epoch: u64::MAX,
        ..Default::default()
    };
    let r = g.begin_refresh().unwrap();
    assert!(g.complete(r, available(1)).is_err());
    assert_eq!(g.observation, None);
}
#[test]
fn scoped_projection_does_not_grant_qualification_and_preserves_cancel_error() {
    let a = available(1).to_value();
    assert_eq!(a.get("schema").and_then(Value::as_str), Some(PROFILE));
    assert_eq!(a.get("release_qualified"), Some(&Value::Bool(false)));
    assert_eq!(
        Observation::Unavailable.to_value().get("helper_sha256"),
        Some(&Value::Null)
    );
    let e = Error::new(ErrorCode::ResourceLimit, "causal observation refusal");
    let actual = observe(&|| Err(e.clone())).unwrap_err();
    assert_eq!(actual.code, e.code);
    assert_eq!(actual.message, e.message);
}

#[test]
fn same_executable_bytes_with_different_verified_manifest_invalidates_ticket() {
    let mut g = Generation::default();
    let r = g.begin_refresh().unwrap();
    let t = g.complete(r, available(1)).unwrap();
    let changed = Observation::Available(Identity {
        helper: [1; 32],
        manifest: [42; 32],
    });
    let r = g.begin_refresh().unwrap();
    let u = g.complete(r, changed).unwrap();
    assert_ne!(u, t);
    assert!(g.require_current(t).is_err());
    g.require_current(u).unwrap();
}
#[test]
#[ignore = "requires copied real writer installation; use serial installed qualification"]
fn actual_selected_manifest_mismatch_refuses_before_helper_launch_and_releases_lease() {
    use super::super::jxr_encode::JxrEncodeJob;
    use paintcraft_core::types::resources::{ResourceLease, SharedBudget};
    let actual = observe(&|| Ok(())).unwrap().identity().unwrap();
    let mut wrong = actual;
    wrong.manifest[0] ^= 1;
    let layout = super::super::super::jxr_encode::Layout::new(2, 1, 1024).unwrap();
    let quote = layout.reservation_bytes().unwrap();
    let budget = SharedBudget::new(quote);
    let owner = Owner {
        session: Uuid::new_v4(),
        state: Uuid::new_v4(),
        epoch: 0,
        frontend: 3,
    };
    let job = JxrEncodeJob {
        layout,
        owner,
        deadline: Duration::from_secs(5),
    };
    let error = job
        .prepare_with_installation(
            Arc::new(vec![0; 6]),
            ResourceLease::new(budget.reserve(quote).unwrap()),
            &|| Ok(()),
            &|| Ok(owner),
            (Some(wrong), |_, _| Ok(())),
            |_, _| panic!("manifest mismatch must not launch"),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ConflictStaleBase);
    assert_eq!(budget.usage().reserved, 0);
}
