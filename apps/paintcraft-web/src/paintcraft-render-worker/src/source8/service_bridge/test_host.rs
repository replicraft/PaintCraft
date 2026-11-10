use super::*;
use paintcraft_core::types::{
    Rect,
    json::{self, Value},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_render::{
    noise_work::{NoiseAdmission, NoiseJobPolicy, NoiseLimits, with_noise_workspace},
    spatial::{WorkspaceAdmission, with_workspace},
};
pub(super) enum Root {
    Base(WorkspaceAdmission),
    Noise(NoiseAdmission),
}
pub(super) struct Host {
    pub budget: SharedBudget,
    pub root: Root,
    pub p: BudgetScope,
    pub m: BudgetScope,
    pub a: BudgetScope,
    pub c: BudgetScope,
    pub k: BudgetScope,
    pub i: BudgetReservation,
    pub q: BudgetReservation,
    pub f: BudgetScope,
    pub o: BudgetScope,
    pub e: BudgetScope,
    pub h: BudgetScope,
    pub raster: paintcraft_raster::ReservedDecodeFactories,
}
impl Host {
    pub fn new(noise: bool) -> Self {
        let budget = SharedBudget::new(64 << 20);
        let scope = |n| {
            budget
                .reserve_capacity(n)
                .unwrap()
                .scope(AllocationClass::SharedBudget)
        };
        let p = scope(2 << 20);
        let m = scope(1 << 20);
        let a = scope(1 << 20);
        let c = scope(2 << 20);
        let k = scope(4096);
        let f = scope(16384);
        let o = scope(2 << 20);
        let e = scope(1 << 20);
        let h = scope(16384);
        let w = scope(4 << 20);
        let root = if noise {
            Root::Noise(
                NoiseAdmission::new_root(
                    w,
                    NoiseJobPolicy::BrowserV1,
                    NoiseLimits {
                        bytes: 16 << 20,
                        base_work: 268435456,
                        noise_work: 1048576,
                    },
                )
                .unwrap(),
            )
        } else {
            Root::Base(WorkspaceAdmission::new(w, 16 << 20, 268435456))
        };
        let i = budget.reserve_capacity(1 << 20).unwrap();
        let q = budget.reserve_capacity(1 << 20).unwrap();
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
            c,
            k,
            i,
            q,
            f,
            o,
            e,
            h,
            raster,
        }
    }
    pub fn run<T>(&self, work: impl FnOnce() -> Result<T>) -> Result<T> {
        match &self.root {
            Root::Base(b) => with_workspace(b.clone(), work),
            Root::Noise(n) => with_noise_workspace(n.clone(), work),
        }
    }
    pub fn base(&self) -> &WorkspaceAdmission {
        match &self.root {
            Root::Base(b) => b,
            Root::Noise(n) => n.workspace(),
        }
    }
    pub fn request(&self) -> Source8InspectionRequest {
        let noise = matches!(&self.root, Root::Noise(_));
        Source8InspectionRequest {
            mode: if noise {
                Source8InspectionMode::Noise
            } else {
                Source8InspectionMode::Base
            },
            caller: NoiseLimits {
                bytes: 16 << 20,
                base_work: 268435456,
                noise_work: if noise { 1048576 } else { 0 },
            },
            source: paintcraft_formats::pcnd::PortableSource8Limits {
                layers: 1024,
                tiles: 4,
                metadata_bytes: 2 << 20,
                canvas_pixels: 1 << 40,
                coverage: paintcraft_raster::coverage_mask::Limits {
                    bands: 4096,
                    runs: 65536,
                    work: 268435456,
                },
                work: 268435456,
            },
        }
    }
    pub fn realms(&self) -> Source8ReadRealms<'_> {
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
            Source8ReadCaps {
                p: 2 << 20,
                m: 1 << 20,
                a: 1 << 20,
                c: 2 << 20,
                k: 4096,
                i: 1 << 20,
                q: 1 << 20,
                staged_pixels: 262144,
            },
        )
    }
    pub fn frame(&self) -> Source8FrameRealms<'_> {
        Source8FrameRealms::new(
            Source8FrameScopes {
                control: &self.f,
                output: &self.o,
            },
            Source8FrameCaps {
                workspace: 4 << 20,
                control: 16384,
                output: 2 << 20,
            },
        )
    }
    pub fn originals(&self) -> ServiceOriginals8<'_> {
        ServiceOriginals8 {
            encoded: &self.e,
            control: &self.h,
            encoded_capacity: 1 << 20,
            control_capacity: 16384,
        }
    }
}
pub(super) fn rows() -> Vec<Value> {
    [
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/source8-display-fixtures/oracle.json"
        ))
        .as_slice(),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/source8-display-fixtures/native-extension.json"
        ))
        .as_slice(),
    ]
    .into_iter()
    .flat_map(|b| {
        json::parse(b)
            .unwrap()
            .get("cases")
            .unwrap()
            .as_array()
            .unwrap()
            .to_vec()
    })
    .collect()
}
pub(super) fn row(name: &str) -> Value {
    rows()
        .into_iter()
        .find(|r| r.get("name").unwrap().as_str() == Some(name))
        .unwrap()
}
pub(super) fn unhex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
        .collect()
}
pub(super) fn bytes(row: &Value) -> Vec<u8> {
    unhex(row.get("hex").unwrap().as_str().unwrap())
}
pub(super) fn display(row: &Value) -> Source8DisplayPatch {
    let rect = |key| {
        let a = row.get(key).unwrap().as_array().unwrap();
        Rect {
            x0: a[0].as_i64().unwrap(),
            y0: a[1].as_i64().unwrap(),
            x1: a[2].as_i64().unwrap(),
            y1: a[3].as_i64().unwrap(),
        }
    };
    Source8DisplayPatch {
        visible: rect("visible"),
        level: row.get("level").unwrap().as_i64().unwrap() as u8,
        patch: rect("patch"),
    }
}
pub(super) fn load(host: &Host, wire: &[u8]) -> Result<ServiceSource8> {
    // Check closure is stack-local and disappears before returned owned source.
    let check = || Ok(());
    let mut entry = entry::Entry8::new(host.request(), host.realms(), &check)?;
    entry.read_into_slot_checked(wire)?;
    entry.take_service_source_checked()
}
