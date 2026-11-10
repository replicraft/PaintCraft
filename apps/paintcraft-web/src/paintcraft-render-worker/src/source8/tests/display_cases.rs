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
use std::cell::{Cell, RefCell};
const C: u64 = 2 << 20;
struct Host {
    budget: SharedBudget,
    caller: Caller,
    p: BudgetScope,
    m: BudgetScope,
    a: BudgetScope,
    c: BudgetScope,
    k: BudgetScope,
    i: BudgetReservation,
    q: BudgetReservation,
    f: BudgetScope,
    o: BudgetScope,
    raster: paintcraft_raster::ReservedDecodeFactories,
}
enum Caller {
    Base(WorkspaceAdmission),
    Noise(NoiseAdmission),
}
impl Host {
    fn new(noise: bool) -> Self {
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
        let c = scope(C);
        let k = scope(4096);
        let f = scope(16384);
        let o = scope(2 << 20);
        let w = scope(4 << 20);
        let caller = if noise {
            Caller::Noise(
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
            Caller::Base(WorkspaceAdmission::new(w, 16 << 20, 268435456))
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
            caller,
            p,
            m,
            a,
            c,
            k,
            i,
            q,
            f,
            o,
            raster,
        }
    }
    fn run<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        match &self.caller {
            Caller::Base(b) => with_workspace(b.clone(), f),
            Caller::Noise(n) => with_noise_workspace(n.clone(), f),
        }
    }
    fn used(&self) -> u64 {
        match &self.caller {
            Caller::Base(b) => b.work_used(),
            Caller::Noise(n) => n.used().unwrap().base,
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
    fn realms(&self) -> Source8ReadRealms<'_> {
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
                c: C,
                k: 4096,
                i: 1 << 20,
                q: 1 << 20,
                staged_pixels: 262144,
            },
        )
    }
    fn frame(&self) -> Source8FrameRealms<'_> {
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
    fn retired(&self) {
        for scope in [&self.p, &self.m, &self.a, &self.c, &self.k, &self.f] {
            assert_eq!(scope.footprint().live, 0);
        }
        assert_eq!(self.i.available(), self.i.capacity());
        assert_eq!(self.q.available(), self.q.capacity());
    }
}
fn rows() -> Vec<Value> {
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
fn unhex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn field(row: &Value, name: &str) -> Rect {
    let a = row.get(name).unwrap().as_array().unwrap();
    Rect {
        x0: a[0].as_i64().unwrap(),
        y0: a[1].as_i64().unwrap(),
        x1: a[2].as_i64().unwrap(),
        y1: a[3].as_i64().unwrap(),
    }
}
fn display(row: &Value) -> Source8DisplayPatch {
    Source8DisplayPatch {
        visible: field(row, "visible"),
        level: row.get("level").unwrap().as_i64().unwrap() as u8,
        patch: field(row, "patch"),
    }
}
fn fixture(name: &str) -> Value {
    rows()
        .into_iter()
        .find(|r| r.get("name").unwrap().as_str() == Some(name))
        .unwrap()
}
fn source_c() -> u64 {
    // Retained decoder controls include the target's actual Mutex layout.
    // This pure quote neither reads observed usage nor creates a new grant.
    8192 + paintcraft_raster::ReservedDecodeAdmission::control_quote()
        .unwrap()
        .fixed_bytes
}
#[test]
fn independent_display_pixels_preserve_whole_grid_and_release_source_before_output() {
    for row in rows() {
        let noise = row.get("mode").and_then(Value::as_str) == Some("Noise");
        let host = Host::new(noise);
        let wire = unhex(row.get("hex").unwrap().as_str().unwrap());
        let expected = unhex(row.get("rgba_hex").unwrap().as_str().unwrap());
        let d = display(&row);
        let image = host
            .run(|| {
                render_portable_source8_display_checked(
                    &wire,
                    host.request(),
                    host.realms(),
                    &host.frame(),
                    d,
                    &|| Ok(()),
                )
            })
            .unwrap();
        assert_eq!(
            image.rgba(),
            expected.as_slice(),
            "{}",
            row.get("name").unwrap().as_str().unwrap()
        );
        assert_eq!(
            (image.width(), image.height()),
            (d.patch.width() as u32, d.patch.height() as u32)
        );
        host.retired();
        assert_eq!(host.o.footprint().live, expected.len() as u64 + 112);
        assert!(host.used() > 0);
        if let Caller::Noise(n) = &host.caller {
            assert!(n.used().unwrap().noise > 0);
        }
        let budget = host.budget.clone();
        drop(host);
        assert_eq!(budget.usage().reserved, 2097152);
        assert_eq!(image.rgba(), expected.as_slice());
        drop(image);
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn invalid_patch_and_level_keep_the_single_loaded_source_and_original_counter_debt() {
    let host = Host::new(false);
    let row = fixture("aligned-expanded-view");
    let wire = unhex(row.get("hex").unwrap().as_str().unwrap());
    host.run(|| {
        let check = || Ok(());
        let mut entry = Entry8::new(host.request(), host.realms(), &check)?;
        entry.read_into_slot_checked(&wire)?;
        let original_q = host.q.available();
        let good = display(&row);
        let mut negatives = [good; 4];
        negatives[0].level = 21;
        negatives[1].patch.x1 = 4;
        negatives[2].patch.x1 = 0;
        negatives[3].visible.x0 = -1;
        for bad in negatives {
            let before = host.used();
            assert_eq!(
                entry
                    .render_display_checked(&host.frame(), bad)
                    .unwrap_err()
                    .code,
                ErrorCode::InputSchema
            );
            assert!(host.used() > before);
            assert!(matches!(&entry.source, SourceSlot8::Loaded(_)));
            assert_eq!(host.q.available(), original_q);
            assert_eq!(host.c.footprint().live, source_c());
            assert_eq!(host.o.footprint().live, 0);
            assert_eq!(host.f.footprint().live, 0);
        }
        let image = entry.render_display_checked(&host.frame(), good)?;
        assert_eq!(
            image.rgba(),
            unhex(row.get("rgba_hex").unwrap().as_str().unwrap())
        );
        drop(image);
        drop(entry);
        Ok(())
    })
    .unwrap();
    host.retired();
}
#[test]
fn original_control_exact_storage_and_one_less_refuse_without_new_factories() {
    let row = fixture("nonzero-cell-patch");
    let wire = unhex(row.get("hex").unwrap().as_str().unwrap());
    for short in [false, true] {
        let host = Host::new(false);
        host.run(|| {
            let check = || Ok(());
            let mut entry = Entry8::new(host.request(), host.realms(), &check)?;
            entry.read_into_slot_checked(&wire)?;
            assert_eq!(host.c.footprint().live, source_c());
            let mut held = host.c.account();
            held.resize(C - source_c() - 4096 - 80 + u64::from(short))?;
            let result = entry.render_display_checked(&host.frame(), display(&row));
            if short {
                let error = result.unwrap_err();
                assert_eq!(error.code, ErrorCode::ResourceLimit);
                assert!(error.allocation_denial().is_some());
                assert_eq!(host.o.footprint().live, 0);
            } else {
                let image = result?;
                assert_eq!(
                    image.rgba(),
                    unhex(row.get("rgba_hex").unwrap().as_str().unwrap())
                );
                drop(image);
            }
            assert_eq!(host.c.footprint().live, held.bytes() + source_c());
            drop(held);
            assert_eq!(host.c.footprint().live, source_c());
            drop(entry);
            Ok(())
        })
        .unwrap();
        host.retired();
    }
}
#[test]
fn first_display_callback_error_preserves_exact_prefix_and_retires_outer_storage() {
    let host = Host::new(false);
    let row = fixture("aligned-expanded-view");
    let wire = unhex(row.get("hex").unwrap().as_str().unwrap());
    let sentinel = Error::new(
        ErrorCode::OperationCancelled,
        "display original diagnostic".repeat(41),
    )
    .at(41);
    let pointer = sentinel.message.as_ptr();
    let slot = RefCell::new(Some(sentinel));
    let reached = Cell::new(false);
    let check = || {
        if host.o.footprint().live != 0 {
            reached.set(true);
            assert_eq!(host.o.footprint().live, 136);
            assert_eq!(host.c.footprint().live, source_c() + 4096 + 240);
            if let Some(error) = slot.borrow_mut().take() {
                return Err(error);
            }
        }
        Ok(())
    };
    host.run(|| {
        let mut entry = Entry8::new(host.request(), host.realms(), &check)?;
        entry.read_into_slot_checked(&wire)?;
        let before = host.used();
        let error = entry
            .render_display_checked(&host.frame(), display(&row))
            .unwrap_err();
        assert!(reached.get());
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.operation_index, Some(41));
        assert_eq!(error.message.as_ptr(), pointer);
        assert_eq!(
            host.used() - before,
            if cfg!(target_pointer_width = "64") {
                43364
            } else {
                43356
            }
        );
        assert_eq!(host.c.footprint().live, source_c());
        assert_eq!(host.o.footprint().live, 0);
        assert_eq!(host.f.footprint().live, 0);
        assert!(host.m.footprint().live > 0);
        drop(entry);
        Ok(())
    })
    .unwrap();
    host.retired();
}

#[test]
fn private_original_control_group_preserves_scope_and_target_movement_literals() {
    use crate::source8::display_public::{ORIGINAL_CONTROL_MOVE_WORK, OriginalControl8};
    assert_eq!(std::mem::size_of::<OriginalControl8<'static>>(), 16);
    assert_eq!(std::mem::align_of::<OriginalControl8<'static>>(), 8);
    assert_eq!(
        ORIGINAL_CONTROL_MOVE_WORK,
        if cfg!(target_pointer_width = "64") {
            96
        } else {
            88
        }
    );
    let host = Host::new(false);
    let original = OriginalControl8 {
        scope: &host.c,
        capacity: C,
    };
    assert!(std::ptr::eq(original.scope, &host.c));
    assert_eq!(original.capacity, C);
    // No new scope, reservation, counter or account is made by the grouping.
    host.retired();
    let budget = host.budget.clone();
    drop(host);
    assert_eq!(budget.usage().reserved, 0);
}
