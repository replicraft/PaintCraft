//! Entry descendant: actual closed-source assertions and causal helper refusals.
use super::super::{assembly_ledger as work, metadata, owner, tests::Host};
use super::*;
use paintcraft_core::types::json::{self, Value};
use paintcraft_render::spatial::{self, WorkspaceAdmission};
fn raw(v: &Value) -> Vec<u8> {
    v.as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
#[test]
fn actual_private_payload_domains_match_independent_physical_oracles() {
    let corpus = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/source8-inspection-fixtures/corpus.json"
    )))
    .unwrap();
    let proofs = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/source8-inspection-fixtures/domain-proof.json"
    )))
    .unwrap();
    for proof in proofs.get("cases").unwrap().as_array().unwrap() {
        let name = proof.get("name").unwrap().as_str().unwrap();
        let row = corpus
            .get("cases")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r.get("name").unwrap().as_str() == Some(name))
            .unwrap();
        let bytes = raw(row.get("hex").unwrap());
        let before = bytes.clone();
        let mut host = Host::new(2 << 20);
        // Select the original whole-corpus profile, not the owner's T0-only host.
        let scope = |n| {
            host.budget
                .reserve_capacity(n)
                .unwrap()
                .scope(paintcraft_core::types::resources::AllocationClass::SharedBudget)
        };
        host.p = scope(2 << 20);
        host.m = scope(1 << 20);
        host.a = scope(1 << 20);
        host.raster.paint_metadata_scope = host.m.clone();
        host.caps.p = 2 << 20;
        host.caps.m = 1 << 20;
        host.caps.a = 1 << 20;
        host.i = host.budget.reserve_capacity(1 << 20).unwrap();
        host.q = host.budget.reserve_capacity(1 << 20).unwrap();
        host.caps.i = 1 << 20;
        host.caps.q = 1 << 20;
        host.caps.staged_pixels = 262144;
        spatial::with_workspace(host.root.clone(), || {
            let check = || Ok(());
            let mut entry = Entry8::new(host.request(), host.realms(host.caps), &check)?;
            entry.read_into_slot_checked(&bytes)?;
            let SourceSlot8::Loaded(source) = &entry.source else {
                panic!("not loaded")
            };
            owner::payload_cases::assert_closed(source, proof.get("fields").unwrap());
            drop(entry);
            Ok(())
        })
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(bytes, before);
        for scope in [&host.p, &host.m, &host.a, &host.c, &host.k] {
            assert_eq!(scope.footprint().live, 0, "{name}");
        }
        assert_eq!(host.i.available(), host.i.capacity());
        assert_eq!(host.q.available(), host.q.capacity());
    }
}
fn helper(work_limit: u64, f: impl FnOnce(&EntryAuth8<'_>) -> Result<()>) -> (Result<()>, u64) {
    let host = Host::new(1 << 20);
    let root = WorkspaceAdmission::new(host.root.scope().clone(), 16 << 20, work_limit);
    let result = spatial::with_workspace(root.clone(), || {
        let check = || Ok(());
        let mut request = host.request();
        request.caller.base_work = work_limit;
        let auth = EntryAuth8 {
            caller: caller::RequiredCaller8::capture(&request)?,
            raw_check: &check,
        };
        f(&auth)
    });
    (result, root.work_used())
}
#[test]
fn stack_hash_and_string_helpers_refuse_before_their_uninstrumented_bodies() {
    for (ceiling, paid, success) in [(63, 0, false), (3055, 64, false), (3056, 3056, true)] {
        let (result, used) = helper(ceiling, |auth| {
            metadata::StackFacts::default()
                .finish(Vec::new(), auth)
                .map(|_| ())
        });
        assert_eq!(result.is_ok(), success);
        assert_eq!(used, paid);
    }
    for (ceiling, paid, success) in [(63, 0, false), (67, 64, false), (68, 68, true)] {
        let (result, used) = helper(ceiling, |auth| {
            metadata::string("ab", auth).map(|s| assert_eq!(s, "ab"))
        });
        assert_eq!(result.is_ok(), success);
        assert_eq!(used, paid);
    }
}
#[test]
fn frozen_adoption_registry_and_cleanup_math_match_literal_target_policy() {
    let native = cfg!(target_pointer_width = "64");
    assert_eq!(work::stack(0, 29).unwrap(), 3056);
    assert_eq!(work::stack(1, 61).unwrap(), 3760);
    assert_eq!(
        work::retire(1, 0, 0).unwrap(),
        if native { 480 } else { 380 }
    );
    assert_eq!(
        work::adoption(1, 4, &work::AdoptionFacts::default()).unwrap(),
        if native { 362809 } else { 280049 }
    );
    for (n, expected) in [0, 0, 7, 11, 23].into_iter().enumerate() {
        assert_eq!(work::bare(n as u64).unwrap(), expected);
    }
}

#[test]
fn fresh_map_allocation_follows_paid_math_and_original_metadata_guard() {
    let native = cfg!(target_pointer_width = "64");
    // Four fresh buckets now contain the larger Layer stride. The fixed
    //144-action prefix and original2000-byte metadata guard are unchanged.
    let exact = if native { 1636 + 4 * 56 } else { 1444 + 4 * 40 };
    for (ceiling, paid, success) in [(exact - 1, 144, false), (exact, exact, true)] {
        let host = Host::new(1 << 20);
        let mut guard = host.m.account();
        guard.resize(2000).unwrap();
        let factories = owner::RetainedFactories8 {
            _m: host.m.clone(),
            _a: host.a.clone(),
            _c: host.c.clone(),
            _i: host.i.clone(),
            _q: host.q.clone(),
            _raster: host.raster.clone(),
        };
        let header = paintcraft_formats::pcnd::PortableHeaderView {
            document_id: uuid::Uuid::from_bytes([16; 16]),
            state_id: uuid::Uuid::from_bytes([17; 16]),
            canvas: (2, 1),
            title: None,
            source_metadata: None,
            layers: 1,
        };
        let (result, used) = helper(ceiling, |auth| {
            metadata::GuardedBuild8::new(header, guard, factories, auth).map(drop)
        });
        assert_eq!(result.is_ok(), success);
        assert_eq!(used, paid);
        assert_eq!(host.m.footprint().live, 0);
    }
}

#[test]
fn new_probe_supplement_refuses_before_helper_and_keeps_original_paid_debt() {
    // Read-only unit preparation obtains an actual opaque SAME Plan summary.
    // It does not claim to qualify that Plan's already-tested construction ledger.
    let host = Host::new(1 << 20);
    let wire = super::super::tests::fixture();
    let checked = paintcraft_formats::pcnd::CheckedContainer::verify(&wire, &|| Ok(())).unwrap();
    let check = || Ok(());
    let spend = |_| Ok(());
    let plan = checked
        .portable_source8_plan_checked(Default::default(), &host.p, &check, &spend)
        .unwrap();
    let layout = plan.retained_source_layout_checked().unwrap();
    for (ceiling, expected) in [(63, 0), (65, 64)] {
        let (result, paid) = helper(ceiling, |auth| {
            super::super::probe_ledger::prepay(layout.summary(), auth)
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
        assert_eq!(paid, expected);
    }
    let (result, paid) = helper(65, |auth| {
        assert_eq!(
            super::super::probe_ledger::prepay(layout.summary(), auth)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(
            super::super::probe_ledger::prepay(layout.summary(), auth)
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        Ok(())
    });
    result.unwrap();
    assert_eq!(paid, 64);
    let (result, paid) = helper(268435456, |auth| {
        super::super::probe_ledger::prepay(layout.summary(), auth)
    });
    result.unwrap();
    assert_eq!(
        paid,
        64 + 16 * paintcraft_formats::pcnd::layer_blend_appearance::CONTROL_BYTES
            + if cfg!(target_pointer_width = "64") {
                397986
            } else {
                322722
            }
    );
}
