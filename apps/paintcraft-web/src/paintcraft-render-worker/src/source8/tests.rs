use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use paintcraft_render::{noise_work::NoiseLimits, spatial::WorkspaceAdmission};
pub(super) struct Host {
    pub budget: SharedBudget,
    pub root: WorkspaceAdmission,
    pub p: BudgetScope,
    pub m: BudgetScope,
    pub a: BudgetScope,
    pub c: BudgetScope,
    pub k: BudgetScope,
    pub i: BudgetReservation,
    pub q: BudgetReservation,
    pub raster: paintcraft_raster::ReservedDecodeFactories,
    pub caps: Source8ReadCaps,
}
impl Host {
    pub fn new(c: u64) -> Self {
        let budget = SharedBudget::new(64 << 20);
        let scope = |n| {
            budget
                .reserve_capacity(n)
                .unwrap()
                .scope(AllocationClass::SharedBudget)
        };
        let root = WorkspaceAdmission::new(scope(16 << 20), 16 << 20, 268435456);
        let p = scope(1 << 20);
        let m = scope(65536);
        let a = scope(65536);
        let c_scope = scope(c);
        let k = scope(4096);
        let i = budget.reserve_capacity(0).unwrap();
        let q = budget.reserve_capacity(0).unwrap();
        let raster = paintcraft_raster::ReservedDecodeFactories {
            selection_budget: budget.clone(),
            coverage_budget: budget.clone(),
            paint_metadata_scope: m.clone(),
        };
        Self {
            budget,
            root,
            p,
            m,
            a,
            c: c_scope,
            k,
            i,
            q,
            raster,
            caps: Source8ReadCaps {
                p: 1 << 20,
                m: 65536,
                a: 65536,
                i: 0,
                c,
                q: 0,
                k: 4096,
                staged_pixels: 0,
            },
        }
    }
    pub fn realms(&self, caps: Source8ReadCaps) -> Source8ReadRealms<'_> {
        Source8ReadRealms::new(
            Source8ReadScopes {
                p: &self.p,
                m: &self.m,
                a: &self.a,
                c: &self.c,
                k: &self.k,
                i: &self.i,
                q: &self.q,
                raster: &self.raster,
            },
            caps,
        )
    }
    pub fn request(&self) -> Source8InspectionRequest {
        Source8InspectionRequest {
            mode: Source8InspectionMode::Base,
            caller: NoiseLimits {
                bytes: 16 << 20,
                base_work: 268435456,
                noise_work: 0,
            },
            source: Default::default(),
        }
    }
}
pub(super) fn fixture() -> Vec<u8> {
    let v = paintcraft_core::types::json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/source8-inspection-fixtures/corpus.json"
    )))
    .unwrap();
    let r = v
        .get("cases")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("name").unwrap().as_str() == Some("snapshot/zero-own-multiply"))
        .unwrap();
    r.get("hex")
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
#[test]
fn independent_layout_profile_and_public_input_debug_are_bounded() {
    owner_ledger::validate_layouts().unwrap();
    assert_eq!(
        owner_ledger::retained_bytes().unwrap(),
        if cfg!(target_pointer_width = "64") {
            2992
        } else {
            1728
        }
    );
    let host = Host::new(1 << 20);
    let before = host.budget.usage();
    let input = host.realms(host.caps);
    assert!(format!("{input:?}").len() < 400);
    assert_eq!(host.budget.usage(), before);
    assert_eq!(host.root.work_used(), 0);
}
#[test]
fn public_entry_requires_actual_installed_caller_before_authentication() {
    let host = Host::new(1 << 20);
    let calls = std::cell::Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let error = inspect_portable_source8_checked(
        &fixture(),
        host.request(),
        host.realms(host.caps),
        &check,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert_eq!(calls.get(), 0);
    assert_eq!(host.root.work_used(), 0);
    assert_eq!(host.c.footprint().live, 0);
    assert_eq!(host.m.footprint().live, 0);
    assert_eq!(host.k.footprint().live, 0);
}
