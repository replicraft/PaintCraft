//! Frozen independent full-pixel oracles through the public Rust-only consumer.
//! No internal modules, raw documents, returned quotes as grants or test hooks.
use paintcraft_core::types::{
    ErrorCode, Rect, Result,
    json::{self, Value},
    resources::{AllocationClass, BudgetReservation, BudgetScope, SharedBudget},
};
use paintcraft_raster::ReservedDecodeFactories;
use paintcraft_render::{
    noise_work::{
        NoiseAdmission, NoiseJobPolicy, NoiseLimits, capture_current_noise_workspace,
        with_noise_workspace,
    },
    spatial::{WorkspaceAdmission, capture_current_workspace, with_workspace},
};
use paintcraft_web_render_worker::source8::*;
use sha2::{Digest, Sha256};
use std::cell::Cell;

const BASE_WORK: u64 = 268_435_456;
const NOISE_WORK: u64 = 1_000_000;
const CALLER_BYTES: u64 = 16 << 20;
const WORKSPACE_BYTES: u64 = 4 << 20;
const FRAME_BYTES: u64 = 16 << 10;
const OUTPUT_BYTES: u64 = 2 << 20;
const IMAGE_CONTROL: u64 = 112;

fn rows() -> Vec<Value> {
    [
        include_bytes!("source8-closed-frame-fixtures/full-frame.json").as_slice(),
        include_bytes!("source8-closed-frame-fixtures/branches.json").as_slice(),
    ]
    .into_iter()
    .flat_map(|bytes| {
        json::parse(bytes)
            .unwrap()
            .get("cases")
            .unwrap()
            .as_array()
            .unwrap()
            .to_vec()
    })
    .collect()
}
fn string<'a>(row: &'a Value, key: &str) -> &'a str {
    row.get(key).unwrap().as_str().unwrap()
}
fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2));
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn roi(row: &Value) -> Rect {
    let v = row.get("roi").unwrap().as_array().unwrap();
    Rect {
        x0: v[0].as_i64().unwrap(),
        y0: v[1].as_i64().unwrap(),
        x1: v[2].as_i64().unwrap(),
        y1: v[3].as_i64().unwrap(),
    }
}
fn source_caps() -> Source8ReadCaps {
    Source8ReadCaps {
        p: 2 << 20,
        m: 1 << 20,
        a: 1 << 20,
        i: 1 << 20,
        c: 2 << 20,
        q: 1 << 20,
        k: 4096,
        staged_pixels: 262_144,
    }
}
enum Caller {
    Base(WorkspaceAdmission),
    Noise(NoiseAdmission),
}
struct Host {
    budget: SharedBudget,
    p: BudgetScope,
    m: BudgetScope,
    a: BudgetScope,
    c: BudgetScope,
    k: BudgetScope,
    i: BudgetReservation,
    q: BudgetReservation,
    raster: ReservedDecodeFactories,
    w: BudgetScope,
    f: BudgetScope,
    o: BudgetScope,
    caller: Caller,
    initial_workspace_live: u64,
}
impl Host {
    fn new(noise: bool) -> Self {
        assert!(capture_current_workspace().is_none());
        assert!(capture_current_noise_workspace().is_none());
        let budget = SharedBudget::new(64 << 20);
        let scope = |n| {
            budget
                .reserve_capacity(n)
                .unwrap()
                .scope(AllocationClass::SharedBudget)
        };
        let cap = source_caps();
        let p = scope(cap.p);
        let m = scope(cap.m);
        let a = scope(cap.a);
        let c = scope(cap.c);
        let k = scope(cap.k);
        let i = budget.reserve_capacity(cap.i).unwrap();
        let q = budget.reserve_capacity(cap.q).unwrap();
        let w = scope(WORKSPACE_BYTES);
        let f = scope(FRAME_BYTES);
        let o = scope(OUTPUT_BYTES);
        let caller = if noise {
            Caller::Noise(
                NoiseAdmission::new_root(
                    w.clone(),
                    NoiseJobPolicy::BrowserV1,
                    NoiseLimits {
                        bytes: CALLER_BYTES,
                        base_work: BASE_WORK,
                        noise_work: NOISE_WORK,
                    },
                )
                .unwrap(),
            )
        } else {
            Caller::Base(WorkspaceAdmission::new(w.clone(), CALLER_BYTES, BASE_WORK))
        };
        let initial_workspace_live = w.footprint().live;
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
            c,
            k,
            i,
            q,
            raster,
            w,
            f,
            o,
            caller,
            initial_workspace_live,
        }
    }
    fn request(&self) -> Source8InspectionRequest {
        let noise = matches!(&self.caller, Caller::Noise(_));
        Source8InspectionRequest {
            mode: if noise {
                Source8InspectionMode::Noise
            } else {
                Source8InspectionMode::Base
            },
            caller: NoiseLimits {
                bytes: CALLER_BYTES,
                base_work: BASE_WORK,
                noise_work: if noise { NOISE_WORK } else { 0 },
            },
            source: paintcraft_formats::pcnd::PortableSource8Limits {
                layers: 1024,
                tiles: 4,
                metadata_bytes: 2 << 20,
                canvas_pixels: 1 << 40,
                coverage: paintcraft_raster::coverage_mask::Limits {
                    bands: 4096,
                    runs: 65_536,
                    work: BASE_WORK,
                },
                work: BASE_WORK,
            },
        }
    }
    fn render(&self, bytes: &[u8], region: Rect, checks: &Cell<u64>) -> Result<Source8RgbaImage> {
        let run = || {
            match &self.caller {
                Caller::Base(base) => {
                    assert!(capture_current_workspace().unwrap().same_job(base));
                    assert!(capture_current_noise_workspace().is_none());
                }
                Caller::Noise(pair) => {
                    assert!(
                        capture_current_workspace()
                            .unwrap()
                            .same_job(pair.workspace())
                    );
                    assert!(capture_current_noise_workspace().unwrap().same_job(pair));
                }
            }
            let realms = Source8ReadRealms::new(
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
                source_caps(),
            );
            let frame = Source8FrameRealms::new(
                Source8FrameScopes {
                    control: &self.f,
                    output: &self.o,
                },
                Source8FrameCaps {
                    workspace: WORKSPACE_BYTES,
                    control: FRAME_BYTES,
                    output: OUTPUT_BYTES,
                },
            );
            render_portable_source8_checked(bytes, self.request(), realms, &frame, region, &|| {
                checks.set(checks.get().checked_add(1).unwrap());
                Ok(())
            })
        };
        match &self.caller {
            Caller::Base(base) => with_workspace(base.clone(), run),
            Caller::Noise(pair) => with_noise_workspace(pair.clone(), run),
        }
    }
    fn assert_source_and_frame_retired(&self, name: &str) {
        for scope in [&self.p, &self.m, &self.a, &self.c, &self.k, &self.f] {
            assert_eq!(scope.footprint().live, 0, "{name}");
        }
        assert_eq!(self.i.available(), self.i.capacity(), "{name}");
        assert_eq!(self.q.available(), self.q.capacity(), "{name}");
        assert_eq!(
            self.w.footprint().live,
            self.initial_workspace_live,
            "{name}"
        );
        assert!(capture_current_workspace().is_none(), "{name}");
        assert!(capture_current_noise_workspace().is_none(), "{name}");
        match &self.caller {
            Caller::Base(base) => assert!(base.work_used() > 0, "{name}"),
            Caller::Noise(pair) => {
                let used = pair.used().unwrap();
                assert!(used.base > 0, "{name}");
                assert!(used.noise > 0, "{name}");
                assert!(used.base <= BASE_WORK && used.noise <= NOISE_WORK, "{name}");
            }
        }
    }
}

#[test]
fn independent_visible_bytes_and_all_source_roles_retire_before_image() {
    let mut accepted = 0;
    for row in rows() {
        if row.get("expected_error").is_some() {
            continue;
        }
        let name = string(&row, "name");
        let wire = unhex(string(&row, "hex"));
        assert_eq!(digest(&wire), string(&row, "sha256"), "{name}");
        let source_before = wire.clone();
        let expected = unhex(string(&row, "rgba_hex"));
        assert_eq!(digest(&expected), string(&row, "rgba_sha256"), "{name}");
        let region = roi(&row);
        let host = Host::new(row.get("mode").and_then(Value::as_str) == Some("Noise"));
        let reserved = host.budget.usage().reserved;
        let checks = Cell::new(0u64);
        let image = host
            .render(&wire, region, &checks)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert_eq!(image.width(), region.width() as u32, "{name}");
        assert_eq!(image.height(), region.height() as u32, "{name}");
        assert_eq!(image.byte_len(), expected.len(), "{name}");
        assert_eq!(image.rgba(), expected.as_slice(), "{name}");
        assert_eq!(wire, source_before, "{name}");
        assert!(checks.get() > 0, "{name}");
        host.assert_source_and_frame_retired(name);
        assert_eq!(
            host.o.footprint().live,
            expected.len() as u64 + IMAGE_CONTROL,
            "{name}"
        );
        assert_eq!(host.budget.usage().reserved, reserved, "{name}");
        let budget = host.budget.clone();
        let output = host.o.clone();
        drop(host);
        drop(wire);
        drop(source_before);
        assert_eq!(budget.usage().reserved, OUTPUT_BYTES, "{name}");
        assert_eq!(image.rgba(), expected.as_slice(), "{name}");
        assert_eq!(
            output.footprint().live,
            expected.len() as u64 + IMAGE_CONTROL,
            "{name}"
        );
        drop(image);
        assert_eq!(output.footprint().live, 0, "{name}");
        assert_eq!(output.capacity_snapshot().available, OUTPUT_BYTES, "{name}");
        drop(output);
        assert_eq!(budget.usage().reserved, 0, "{name}");
        accepted += 1;
    }
    assert_eq!(accepted, 22);
}

#[test]
fn independent_offcanvas_refusal_returns_no_image_and_retires_all_roles() {
    let cases = rows();
    let row = cases
        .iter()
        .find(|row| row.get("expected_error").is_some())
        .unwrap();
    let wire = unhex(string(row, "hex"));
    let source_before = wire.clone();
    let host = Host::new(false);
    let checks = Cell::new(0u64);
    let error = host.render(&wire, roi(row), &checks).unwrap_err();
    assert_eq!(error.code, ErrorCode::InputSchema);
    assert_eq!(error.message, "CPU blend quote region");
    assert_eq!(wire, source_before);
    host.assert_source_and_frame_retired(string(row, "name"));
    assert_eq!(host.o.footprint().live, 0);
    let budget = host.budget.clone();
    drop(error);
    drop(host);
    assert_eq!(budget.usage().reserved, 0);
}

#[test]
fn image_alone_keeps_original_output_capacity_after_all_host_factories_die() {
    let cases = rows();
    let row = cases
        .iter()
        .find(|row| row.get("name").and_then(Value::as_str) == Some("mask-then-opacity"))
        .unwrap();
    let wire = unhex(string(row, "hex"));
    let expected = unhex(string(row, "rgba_hex"));
    let host = Host::new(false);
    let image = host.render(&wire, roi(row), &Cell::new(0)).unwrap();
    host.assert_source_and_frame_retired("sole output factory");
    let budget = host.budget.clone();
    drop(host);
    drop(wire);
    // No output observer/factory survives here; only the image keeps original O.
    assert_eq!(budget.usage().reserved, OUTPUT_BYTES);
    assert_eq!(image.rgba(), expected.as_slice());
    drop(image);
    assert_eq!(budget.usage().reserved, 0);
}
