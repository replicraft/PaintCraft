//! Independent whole-wire public consumer cases. No returned quote becomes a grant.
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    json::{self, Value},
    resources::{AllocationClass, BudgetReservation, BudgetScope, SharedBudget},
};
use paintcraft_raster::ReservedDecodeFactories;
use paintcraft_render::{
    noise_work::NoiseLimits,
    spatial::{WorkspaceAdmission, capture_current_workspace, with_workspace},
};
use paintcraft_web_render_worker::source8::*;
use sha2::{Digest, Sha256};
use std::cell::Cell;
const WORK: u64 = 268435456;
fn rows() -> Vec<Value> {
    json::parse(include_bytes!("source8-inspection-fixtures/corpus.json"))
        .unwrap()
        .get("cases")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec()
}
fn bytes(row: &Value) -> Vec<u8> {
    row.get("hex")
        .unwrap()
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn caps() -> Source8ReadCaps {
    Source8ReadCaps {
        p: 2 << 20,
        m: 1 << 20,
        a: 1 << 20,
        i: 1 << 20,
        c: 2 << 20,
        q: 1 << 20,
        k: 4096,
        staged_pixels: 262144,
    }
}
struct Host {
    budget: SharedBudget,
    p: BudgetScope,
    m: BudgetScope,
    a: BudgetScope,
    i: BudgetReservation,
    c: BudgetScope,
    q: BudgetReservation,
    k: BudgetScope,
    raster: ReservedDecodeFactories,
    caller: WorkspaceAdmission,
}
impl Host {
    fn new(cap: Source8ReadCaps, work: u64) -> Self {
        let budget = SharedBudget::new(64 << 20);
        let scope = |n| {
            budget
                .reserve_capacity(n)
                .unwrap()
                .scope(AllocationClass::SharedBudget)
        };
        let p = scope(cap.p);
        let m = scope(cap.m);
        let a = scope(cap.a);
        let c = scope(cap.c);
        let k = scope(cap.k);
        let i = budget.reserve_capacity(cap.i).unwrap();
        let q = budget.reserve_capacity(cap.q).unwrap();
        let caller = WorkspaceAdmission::new(scope(16 << 20), 16 << 20, work);
        let raster = ReservedDecodeFactories {
            selection_budget: budget.clone(),
            coverage_budget: budget.clone(),
            paint_metadata_scope: m.clone(),
        };
        Self {
            budget,
            p,
            m,
            a,
            i,
            c,
            q,
            k,
            raster,
            caller,
        }
    }
    fn realms(&self, cap: Source8ReadCaps) -> Source8ReadRealms<'_> {
        Source8ReadRealms::new(
            Source8ReadScopes {
                p: &self.p,
                m: &self.m,
                a: &self.a,
                i: &self.i,
                c: &self.c,
                q: &self.q,
                k: &self.k,
                raster: &self.raster,
            },
            cap,
        )
    }
    fn request() -> Source8InspectionRequest {
        let source = paintcraft_formats::pcnd::PortableSource8Limits {
            layers: 1024,
            tiles: 4,
            metadata_bytes: 2 << 20,
            canvas_pixels: 1 << 40,
            coverage: paintcraft_raster::coverage_mask::Limits {
                bands: 4096,
                runs: 65536,
                work: WORK,
            },
            work: WORK,
        };
        Source8InspectionRequest {
            mode: Source8InspectionMode::Base,
            caller: NoiseLimits {
                bytes: 16 << 20,
                base_work: WORK,
                noise_work: 0,
            },
            source,
        }
    }
    fn read(
        &self,
        wire: &[u8],
        cap: Source8ReadCaps,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Source8Inspection> {
        with_workspace(self.caller.clone(), || {
            inspect_portable_source8_checked(wire, Self::request(), self.realms(cap), check)
        })
    }
    fn empty(&self) {
        for s in [&self.p, &self.m, &self.a, &self.c, &self.k] {
            assert_eq!(s.footprint().live, 0);
        }
        assert_eq!(self.i.available(), self.i.capacity());
        assert_eq!(self.q.available(), self.q.capacity());
    }
}
#[test]
fn independent_whole_reader_matrix_owns_and_retires_every_source() {
    for row in rows() {
        let name = row.get("name").unwrap().as_str().unwrap();
        let wire = bytes(&row);
        let before = wire.clone();
        let host = Host::new(caps(), WORK);
        let reserved = host.budget.usage().reserved;
        let result = host.read(&wire, caps(), &|| Ok(()));
        if row.get("accept").unwrap().as_bool().unwrap() {
            let result = result.unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert_eq!(
                result.source_sha256(),
                <[u8; 32]>::from(Sha256::digest(&wire)),
                "{name}"
            );
            assert!(result.layers() > 0, "{name}");
            let scalars = json::parse(include_bytes!(
                "source8-inspection-fixtures/wire-scalars.json"
            ))
            .unwrap();
            let scalar = scalars
                .get("cases")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r.get("name").unwrap().as_str() == Some(name))
                .unwrap();
            let canvas = scalar.get("canvas").unwrap().as_array().unwrap();
            assert_eq!(
                result.canvas(),
                [
                    canvas[0].as_i64().unwrap() as u32,
                    canvas[1].as_i64().unwrap() as u32
                ],
                "{name}"
            );
            if name.starts_with("icc/") {
                let assertions = row.get("private_assertions").unwrap();
                let profile = assertions.get("profile_hex").unwrap().as_str().unwrap();
                assert_eq!(result.profile_bytes(), (profile.len() / 2) as u64);
                assert_eq!(
                    result.metadata_kind(),
                    if profile.is_empty() { 1 } else { 2 }
                );
            }
        } else {
            assert!(result.is_err(), "{name} unexpectedly accepted");
        }
        assert_eq!(wire, before, "{name}");
        host.empty();
        assert_eq!(host.budget.usage().reserved, reserved, "{name}");
        assert!(capture_current_workspace().is_none());
    }
}
fn small() -> Vec<u8> {
    bytes(
        &rows()
            .into_iter()
            .find(|r| r.get("name").unwrap().as_str() == Some("snapshot/zero-own-multiply"))
            .unwrap(),
    )
}
#[test]
fn immutable_original_capacities_cannot_understate_aggregate_union() {
    let host = Host::new(caps(), WORK);
    let wire = small();
    for which in 0..7 {
        let mut short = caps();
        match which {
            0 => short.p -= 1,
            1 => short.m -= 1,
            2 => short.a -= 1,
            3 => short.i -= 1,
            4 => short.c -= 1,
            5 => short.q -= 1,
            _ => short.k -= 1,
        }
        let checks = Cell::new(0);
        let check = || {
            checks.set(checks.get() + 1);
            Ok(())
        };
        assert_eq!(
            host.read(&wire, short, &check).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(checks.get(), 0);
        host.empty();
    }
    assert!(host.read(&wire, caps(), &|| Ok(())).is_ok());
    host.empty();
}
#[test]
fn first_callback_error_is_preserved_then_distinct_invocation_uses_original_work() {
    let wire = small();
    let host = Host::new(caps(), WORK);
    let sentinel = Error::new(ErrorCode::OperationCancelled, "diagnostic".repeat(200));
    let before = host.caller.work_used();
    let failure = host
        .read(&wire, caps(), &|| Err(sentinel.clone()))
        .unwrap_err();
    assert_eq!(failure, sentinel);
    let failed = host.caller.work_used();
    assert!(failed > before);
    host.empty();
    // This is a distinct public inspection operation, not decoder debt renewal.
    host.read(&wire, caps(), &|| Ok(())).unwrap();
    assert!(host.caller.work_used() > failed);
    host.empty();
}

#[test]
fn literal_domain_original_reservations_exact_and_minus_one() {
    let native = cfg!(target_pointer_width = "64");
    for (wire_name, field, exact) in [
        (
            "snapshot/zero-own-multiply",
            0,
            if native { 1534 } else { 1342 },
        ),
        (
            "snapshot/zero-own-multiply",
            1,
            if native { 1776 } else { 1544 },
        ),
        (
            "snapshot/zero-own-multiply",
            2,
            if native { 2992 } else { 1728 },
        ),
        ("codecs/manual-raw", 0, if native { 1890 } else { 1650 }),
        ("codecs/manual-raw", 3, 262144),
        ("icc/minimal-icc", 4, if native { 620 } else { 604 }),
        ("icc/descriptor-only", 4, if native { 488 } else { 472 }),
    ] {
        let wire = bytes(
            &rows()
                .into_iter()
                .find(|r| r.get("name").unwrap().as_str() == Some(wire_name))
                .unwrap(),
        );
        for short in [true, false] {
            let mut cap = caps();
            let value = exact - u64::from(short);
            match field {
                0 => cap.m = value,
                1 => cap.a = value,
                2 => cap.k = value,
                3 => cap.q = value,
                _ => cap.i = value,
            }
            let host = Host::new(cap, WORK);
            let result = host.read(&wire, cap, &|| Ok(()));
            if short {
                assert_eq!(
                    result.unwrap_err().code,
                    ErrorCode::ResourceLimit,
                    "{wire_name} field{field}"
                );
            } else {
                result.unwrap_or_else(|e| panic!("{wire_name} field{field}: {e:?}"));
            }
            host.empty();
        }
    }
}
