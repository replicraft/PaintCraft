use super::*;
use paintcraft_core::types::{
    Rect,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_render::{
    noise_work::NoiseLimits,
    spatial::{WorkspaceAdmission, with_workspace},
};
use std::cell::{Cell, RefCell};

struct Host {
    budget: SharedBudget,
    root: WorkspaceAdmission,
    p: BudgetScope,
    m: BudgetScope,
    a: BudgetScope,
    c: BudgetScope,
    k: BudgetScope,
    i: BudgetReservation,
    q: BudgetReservation,
    raster: paintcraft_raster::ReservedDecodeFactories,
    f: BudgetScope,
    o: BudgetScope,
    output: u64,
}
impl Host {
    fn new(output: u64) -> Self {
        let budget = SharedBudget::new(64 << 20);
        let scope = |n| {
            budget
                .reserve_capacity(n)
                .unwrap()
                .scope(AllocationClass::SharedBudget)
        };
        let root = WorkspaceAdmission::new(scope(4 << 20), 16 << 20, 268435456);
        let p = scope(2 << 20);
        let m = scope(1 << 20);
        let a = scope(1 << 20);
        let c = scope(2 << 20);
        let k = scope(4096);
        let f = scope(16384);
        let o = scope(output);
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
            raster,
            f,
            o,
            output,
        }
    }
    fn caps(&self) -> Source8ReadCaps {
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
    fn realms(&self) -> Source8ReadRealms<'_> {
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
            self.caps(),
        )
    }
    fn request(&self) -> Source8InspectionRequest {
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
    fn frame(&self) -> frame_inputs::Source8FrameRealms<'_> {
        frame_inputs::Source8FrameRealms::new(
            frame_inputs::Source8FrameScopes {
                control: &self.f,
                output: &self.o,
            },
            frame_inputs::Source8FrameCaps {
                workspace: 4 << 20,
                control: 16384,
                output: self.output,
            },
        )
    }
    fn assert_source_retired(&self) {
        for scope in [&self.p, &self.m, &self.a, &self.c, &self.k, &self.f] {
            assert_eq!(scope.footprint().live, 0);
        }
        assert_eq!(self.i.available(), self.i.capacity());
        assert_eq!(self.q.available(), self.q.capacity());
    }
}
fn rect() -> Rect {
    Rect::new(0, 0, 1, 1).unwrap()
}

#[test]
fn output_exact_boundary_and_independent_bytes_outlive_all_original_host_factories() {
    for output in [115, 116] {
        let host = Host::new(output);
        let budget = host.budget.clone();
        let image = with_workspace(host.root.clone(), || {
            let check = || Ok(());
            let entry = Entry8::new(host.request(), host.realms(), &check)?;
            let frame = host.frame();
            let _control = frame.admit_control(&entry.auth)?;
            let result = frame_output::PendingOutput8::admit(1, 1, &frame, &entry.auth);
            if output == 115 {
                let error = result.err().unwrap();
                assert_eq!(error.code, ErrorCode::ResourceLimit);
                assert_eq!(host.o.footprint().live, 0);
                Ok(None)
            } else {
                assert_eq!(host.o.footprint().live, 116);
                let image = result.unwrap().finish(vec![17, 29, 41, 53], &entry.auth)?;
                assert_eq!(image.rgba(), &[17, 29, 41, 53]);
                assert_eq!((image.width(), image.height(), image.byte_len()), (1, 1, 4));
                Ok(Some(image))
            }
        })
        .unwrap();
        host.assert_source_retired();
        drop(host);
        if let Some(image) = image {
            assert_eq!(image.rgba(), &[17, 29, 41, 53]);
            assert_eq!(budget.usage().reserved, 116);
            drop(image);
        }
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn finish_denial_keeps_pixels_paired_and_preserves_original_spent_work() {
    let host = Host::new(116);
    with_workspace(host.root.clone(), || {
        let check = || Ok(());
        let entry = Entry8::new(host.request(), host.realms(), &check)?;
        let frame = host.frame();
        let _control = frame.admit_control(&entry.auth)?;
        let pending = frame_output::PendingOutput8::admit(1, 1, &frame, &entry.auth)?;
        host.root.reserve_work(268435456 - host.root.work_used())?;
        let error = pending.finish(vec![1, 2, 3, 4], &entry.auth).unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(host.root.work_used(), 268435456);
        assert_eq!(host.o.footprint().live, 0);
        Ok(())
    })
    .unwrap();
    host.assert_source_retired();
}

#[test]
fn original_callback_diagnostic_after_output_admission_moves_without_replacement() {
    let host = Host::new(2 << 20);
    let original = Error::new(
        ErrorCode::OperationCancelled,
        "frame diagnostic".repeat(257),
    )
    .at(31);
    let pointer = original.message.as_ptr();
    let error_slot = RefCell::new(Some(original));
    let observed = Cell::new(false);
    let check = || {
        if host.o.footprint().live != 0 {
            observed.set(true);
            if let Some(error) = error_slot.borrow_mut().take() {
                return Err(error);
            }
        }
        Ok(())
    };
    with_workspace(host.root.clone(), || {
        let mut entry = Entry8::new(host.request(), host.realms(), &check)?;
        entry.read_into_slot_checked(&crate::source8::tests::fixture())?;
        let frame = host.frame();
        let error = entry.render_closed_checked(&frame, rect()).unwrap_err();
        assert!(observed.get());
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.operation_index, Some(31));
        assert_eq!(error.message.as_ptr(), pointer);
        assert_eq!(host.o.footprint().live, 0);
        assert_eq!(host.f.footprint().live, 0);
        assert!(host.m.footprint().live > 0);
        drop(entry);
        Ok(())
    })
    .unwrap();
    host.assert_source_retired();
}

#[test]
fn wrong_declared_byte_union_refuses_before_even_attempting_full_f_account() {
    let host = Host::new(2 << 20);
    let mut held = host.f.account();
    held.resize(16384).unwrap();
    with_workspace(host.root.clone(), || {
        let mut request = host.request();
        request.caller.bytes = 14 << 20;
        let check = || Ok(());
        let mut entry = Entry8::new(request, host.realms(), &check)?;
        entry.read_into_slot_checked(&crate::source8::tests::fixture())?;
        let before = host.root.work_used();
        let frame = host.frame();
        let error = entry.render_closed_checked(&frame, rect()).unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(error.message, "Source8 frame original declaration limit");
        assert_eq!(host.root.work_used() - before, 4160);
        assert_eq!(host.f.footprint().live, 16384);
        assert_eq!(host.f.footprint().peak, 16384);
        assert_eq!(host.o.footprint().live, 0);
        Ok(())
    })
    .unwrap();
    drop(held);
    host.assert_source_retired();
}

#[test]
fn missing_original_caller_has_no_decode_frame_or_callback_side_effects() {
    let host = Host::new(2 << 20);
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let frame = host.frame();
    let error = frame_public::render_portable_source8_checked(
        &crate::source8::tests::fixture(),
        host.request(),
        host.realms(),
        &frame,
        rect(),
        &check,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
    assert_eq!(calls.get(), 0);
    assert_eq!(host.root.work_used(), 0);
    assert_eq!(host.o.footprint().live, 0);
    host.assert_source_retired();
}

#[test]
fn original_allocation_denial_after_output_admission_keeps_its_diagnostic_realm() {
    let host = Host::new(2 << 20);
    let diagnostic = SharedBudget::new(1);
    let original = diagnostic.reserve(2).unwrap_err().at(23);
    let pointer = original.message.as_ptr();
    let slot = RefCell::new(Some(original));
    let reached = Cell::new(false);
    let check = || {
        if host.o.footprint().live != 0 {
            reached.set(true);
            if let Some(error) = slot.borrow_mut().take() {
                return Err(error);
            }
        }
        Ok(())
    };
    with_workspace(host.root.clone(), || {
        let mut entry = Entry8::new(host.request(), host.realms(), &check)?;
        entry.read_into_slot_checked(&crate::source8::tests::fixture())?;
        let frame = host.frame();
        let error = entry.render_closed_checked(&frame, rect()).unwrap_err();
        assert!(reached.get());
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(error.operation_index, Some(23));
        assert_eq!(error.message.as_ptr(), pointer);
        let denial = error.allocation_denial().unwrap();
        assert_eq!(denial.required_bytes(), 2);
        assert_eq!(denial.budget().usage(), diagnostic.usage());
        assert_eq!(denial.budget().usage().limit, 1);
        let original_lease = diagnostic.reserve(1).unwrap();
        assert_eq!(denial.budget().usage().reserved, 1);
        drop(original_lease);
        assert_eq!(denial.budget().usage().reserved, 0);
        assert_eq!(host.o.footprint().live, 0);
        assert_eq!(host.f.footprint().live, 0);
        drop(entry);
        Ok(())
    })
    .unwrap();
    host.assert_source_retired();
}

fn prepared_fixture(keys: &[[u8; 5]]) -> paintcraft_document::Document {
    use paintcraft_core::types::{
        Rgba,
        filters::{CurvesRgbV1, Descriptor},
        ids::deterministic_id,
    };
    let mut doc = paintcraft_document::new_document(
        1,
        1,
        Some(Rgba([0, 64, 128, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters = paintcraft_document::FilterStack::new(
        keys.iter()
            .enumerate()
            .map(|(i, values)| {
                paintcraft_document::FilterInstance::new(
                    deterministic_id(&["closed-frame-existing-key", &i.to_string()]),
                    true,
                    Descriptor::CurvesRgbV1(CurvesRgbV1::new(*values)),
                )
            })
            .collect(),
    )
    .unwrap();
    doc
}
fn curves_k_wire() -> Vec<u8> {
    const HEX: &str = "50434e44080d0a1a19010d6375727665732d7267622d763109706172616d732d76311010101010101010101010101010101011111111111111111111111111111111010100000212121212121212121212121212121212054c6179657201ff0000000101004080ff0128282828282828282828282828282828010005004080c0ff00000013131313131313131313131313131313054c6179657200ff0100000101ffffffff000000d2de1b755c68d269176fed2d25a0b5470d184ed7019e6a1e07bc612b18dc9dc4";
    HEX.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
#[test]
fn original_ambient_and_selected_prepared_key_matrix_and_alias_retirement() {
    use paintcraft_render::{
        noise_work::{NoiseAdmission, NoiseJobPolicy, with_noise_workspace},
        prepared::prepare_document_programs,
    };
    const K: [u8; 5] = [0, 64, 128, 192, 255];
    const J: [u8; 5] = [255, 192, 128, 64, 0];
    for case in 0..9 {
        let mut host = Host::new(2 << 20);
        let root = NoiseAdmission::new_root(
            host.root.scope().clone(),
            NoiseJobPolicy::BrowserV1,
            NoiseLimits {
                bytes: 16 << 20,
                base_work: 268435456,
                noise_work: 1048576,
            },
        )
        .unwrap();
        host.root = root.workspace().clone();
        let ambient = match case {
            0 | 4 | 5 | 7 => root
                .prepare_document(&prepared_fixture(&[K]), &|| Ok(()))
                .unwrap(),
            1 | 6 => root
                .prepare_document(&prepared_fixture(&[J]), &|| Ok(()))
                .unwrap(),
            8 => root
                .prepare_document(&prepared_fixture(&[K, J]), &|| Ok(()))
                .unwrap(),
            _ => root.clone(),
        };
        let selected = match case {
            2 | 4 | 6 => prepare_document_programs(
                &prepared_fixture(&[K]),
                root.workspace().clone(),
                &|| Ok(()),
            )
            .unwrap(),
            3 | 7 => prepare_document_programs(
                &prepared_fixture(&[J]),
                root.workspace().clone(),
                &|| Ok(()),
            )
            .unwrap(),
            5 | 8 => ambient.workspace().clone(),
            _ => root.workspace().clone(),
        };
        let union = selected
            .prepared_owner_union_bytes(Some(ambient.workspace()))
            .unwrap();
        if case == 4 {
            let one = selected.prepared_owner_union_bytes(None).unwrap();
            assert!(one > 0);
            assert_eq!(union, 2 * one);
        }
        if case == 5 || case == 8 {
            assert_eq!(union, selected.prepared_owner_union_bytes(None).unwrap());
        }
        if case == 8 {
            assert_eq!(selected.prepared_curves().unwrap().len(), 2);
        }
        let original_w_live = host.root.scope().footprint().live;
        let before = root.used().unwrap();
        let result = with_noise_workspace(ambient.clone(), || {
            with_workspace(selected.clone(), || {
                let mut request = host.request();
                request.mode = Source8InspectionMode::Noise;
                request.caller.noise_work = 1048576;
                frame_public::render_portable_source8_checked(
                    &curves_k_wire(),
                    request,
                    host.realms(),
                    &host.frame(),
                    rect(),
                    &|| Ok(()),
                )
            })
        });
        if matches!(case, 0 | 2 | 4 | 5 | 8) {
            let image = result.unwrap_or_else(|e| panic!("matrix {case}: {e:?}"));
            assert_eq!(image.rgba(), &[0, 64, 128, 255]);
            drop(image);
        } else {
            assert_eq!(
                result.unwrap_err().code,
                ErrorCode::CapabilityUnsupported,
                "matrix {case}"
            );
        }
        let after = root.used().unwrap();
        assert!(after.base > before.base);
        assert_eq!(after.noise, before.noise);
        assert_eq!(host.root.scope().footprint().live, original_w_live);
        assert_eq!(host.o.footprint().live, 0);
        host.assert_source_retired();
        let surviving = if selected.has_prepared_programs() {
            selected.clone()
        } else {
            ambient.workspace().clone()
        };
        let retained = surviving.prepared_retained_bytes();
        assert!(retained > 0);
        drop(selected);
        drop(ambient);
        drop(root);
        // Existing complete programs retain W payload only. No source or F alias
        // is present in their graph; all source roles were already checked above.
        assert_eq!(host.root.scope().footprint().live, retained);
        host.assert_source_retired();
        drop(surviving);
        assert_eq!(host.root.scope().footprint().live, 0);
    }
}

#[test]
fn actual_render_callback_captures_retain_only_original_workspace_after_source_and_host_retire() {
    use paintcraft_render::{
        noise_work::{
            NoiseAdmission, NoiseJobPolicy, capture_current_noise_workspace, with_noise_workspace,
        },
        spatial::capture_current_workspace,
    };
    for noise_first in [false, true] {
        let mut host = Host::new(2 << 20);
        let root = NoiseAdmission::new_root(
            host.root.scope().clone(),
            NoiseJobPolicy::BrowserV1,
            NoiseLimits {
                bytes: 16 << 20,
                base_work: 268435456,
                noise_work: 1048576,
            },
        )
        .unwrap();
        host.root = root.workspace().clone();
        assert!(!host.root.has_prepared_programs());
        let saved = RefCell::new(None);
        let captured_while_source_alive = Cell::new(false);
        let check = || {
            let base = capture_current_workspace().unwrap();
            if host.o.footprint().live > 0
                && base.has_prepared_programs()
                && saved.borrow().is_none()
            {
                let noise = capture_current_noise_workspace().unwrap();
                assert!(base.same_job(root.workspace()));
                assert!(noise.same_job(&root));
                assert!(host.m.footprint().live > 0);
                assert!(host.f.footprint().live > 0);
                assert!(base.prepared_retained_bytes() > 0);
                captured_while_source_alive.set(true);
                *saved.borrow_mut() = Some((base, noise));
            }
            Ok(())
        };
        let image = with_noise_workspace(root.clone(), || {
            let mut request = host.request();
            request.mode = Source8InspectionMode::Noise;
            request.caller.noise_work = 1048576;
            frame_public::render_portable_source8_checked(
                &curves_k_wire(),
                request,
                host.realms(),
                &host.frame(),
                rect(),
                &check,
            )
        })
        .unwrap();
        assert!(captured_while_source_alive.get());
        let (base, noise) = saved.into_inner().unwrap();
        assert_eq!(base.work_used(), root.used().unwrap().base);
        assert_eq!(noise.used().unwrap().base, base.work_used());
        assert_eq!(noise.used().unwrap().noise, 0);
        assert!(base.prepared_retained_bytes() > 0);
        host.assert_source_retired();
        assert_eq!(host.o.footprint().live, 116);
        assert_eq!(image.rgba(), &[0, 64, 128, 255]);
        let budget = host.budget.clone();
        drop(root);
        drop(host);
        assert_eq!(budget.usage().reserved, 6291456);
        if noise_first {
            drop(noise);
            assert_eq!(
                base.scope().footprint().live,
                base.prepared_retained_bytes()
            );
            assert_eq!(budget.usage().reserved, 6291456);
            drop(base);
        } else {
            drop(base);
            assert_eq!(budget.usage().reserved, 6291456);
            drop(noise);
        }
        assert_eq!(budget.usage().reserved, 2097152);
        assert_eq!(image.rgba(), &[0, 64, 128, 255]);
        drop(image);
        assert_eq!(budget.usage().reserved, 0);
    }
}
