use super::*;
use paintcraft_core::types::{
    LayerBlendMode, Rgba, json,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::Document;
use paintcraft_formats::pcnd::{self, count_portable_blend_snapshot_rooted_checked};
use paintcraft_render::{blend::metadata::quote_unbound_rooted_frame_workspace_checked, spatial};
fn source(name: &str) -> Document {
    let corpus = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../crates/services/paintcraft-formats/tests/retained-source8-layout-fixtures/oracle.json"
    ))).unwrap();
    let row = corpus
        .get("cases")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r.get("name").unwrap().as_str() == Some(name))
        .unwrap();
    let wire: Vec<u8> = row
        .get("hex")
        .unwrap()
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
        .collect();
    let budget = SharedBudget::new(32 << 20);
    let mut doc = pcnd::read_with_admission_checked(
        &wire,
        1 << 40,
        1 << 20,
        1024,
        pcnd::ReadAdmission {
            metadata: pcnd::MetadataAdmission {
                bytes: 8 << 20,
                budget: &budget,
            },
            coverage: pcnd::CoverageAdmission {
                bytes: 8 << 20,
                budget: &budget,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap_or_else(|error| panic!("materialize source fixture {name}: {error}"));
    doc.layers.get_mut(&doc.roots[0]).unwrap().blend_mode = LayerBlendMode::Multiply;
    doc
}
fn basic() -> Document {
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([7, 9, 11, 0])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().blend_mode = LayerBlendMode::Multiply;
    doc
}
fn demand(doc: &Document) -> (Vec<u8>, BlendSourceDemand, UnboundRootedFrameWorkspace) {
    let budget = SharedBudget::new(64 << 20);
    let scope = budget
        .reserve_capacity(64 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let work = spatial::WorkspaceAdmission::new(scope.clone(), 64 << 20, 268435456);
    let spend = |n| work.reserve_work(n);
    let check = || Ok(());
    let counted =
        count_portable_blend_snapshot_rooted_checked(doc, &scope, &check, &spend).unwrap();
    let (bytes, source) = counted.encode_with_source_forecast_checked().unwrap();
    let w = quote_unbound_rooted_frame_workspace_checked(doc, &check, &spend).unwrap();
    (bytes.as_bytes().to_vec(), source, w)
}
#[test]
fn t0_target_wrappers_and_compiled_roles_are_defining_not_fixture_grants() {
    let (bytes, d, w) = demand(&basic());
    let q = source8_role_demand_checked(&d, &w).unwrap();
    assert_eq!(q.roles[0], bytes.len() as u64);
    assert_eq!(q.roles[1], d.p_peak);
    assert_ne!(q.roles[1], 2 << 20);
    assert_eq!(q.roles[3], if usize::BITS == 64 { 1776 } else { 1544 });
    assert_eq!(q.roles[4], 0);
    assert_eq!(q.display_control_bytes, 176128);
    assert_eq!(
        q.roles[5],
        q.read_control_bytes.max(q.display_control_bytes)
    );
    assert_eq!(q.roles[6], 0);
    assert_eq!(q.roles[7], if usize::BITS == 64 { 2992 } else { 1728 });
    assert_eq!(q.roles[8], w.workspace_bytes);
    assert_eq!((q.roles[9], q.roles[10]), (16384, 32992));
    assert_eq!(
        q.roles[11],
        LayerBlendService::control_quote()
            .unwrap()
            .into_iter()
            .sum::<u64>()
    );
    assert_eq!(
        q.compiled_profile,
        LayerBlendService::compiled_profile().unwrap()
    );
    assert_eq!(
        q.total_bytes,
        q.roles.iter().sum::<u64>() + q.paired_noise_bytes
    );
    assert_eq!(size_of::<Source8RoleDemand>(), 168);
}
#[test]
fn real_counted_corpus_preserves_empty_masks_icc_and_pixel_phase_overlap() {
    for name in [
        "baseline",
        "default-raw",
        "empty-own",
        "nonempty-disabled-own",
        "global-two-masks",
        "mixed-duplicate-masks",
        "icc-profile-none",
    ] {
        let (bytes, d, w) = demand(&source(name));
        let s = &d.retained;
        let q = source8_role_demand_checked(&d, &w).unwrap();
        let wrapper = if usize::BITS == 64 { 1776 } else { 1544 };
        assert_eq!(
            q.roles[3],
            s.a_retained() + wrapper + if s.mask_present() > 0 { 4096 } else { 0 }
        );
        assert_eq!(q.roles[4], s.icc_retained());
        if name == "icc-profile-none" {
            assert_eq!(s.profile_bytes(), 0);
            assert!(q.roles[4] > 192);
        }
        if name == "empty-own" {
            assert_eq!(s.mask_nonempty(), 0);
            assert!(s.mask_present() > 0);
        }
        let pixel = if s.tiles() == 0 {
            0
        } else {
            s.q_fixed() + if usize::BITS == 64 { 408 } else { 300 } + 4096 + s.scratch_bytes()
        };
        let adoption =
            s.q_fixed() + layer_blend_appearance::transient_quote(s.layers() as usize).unwrap();
        let icc = if s.attachment_count() == 0 { 0 } else { 4096 };
        assert_eq!(q.read_control_bytes, 12288 + pixel.max(adoption).max(icc));
        assert_eq!(q.display_control_bytes, 176128 + s.q_fixed());
        assert_eq!(q.roles[6], s.tiles() * 262144);
        assert_eq!(q.staged_pixels, s.tiles() * 65536);
        assert_eq!(q.roles[0], bytes.len() as u64);
    }
}
#[test]
fn query_is_pure_preserves_existing_original_counter_source_and_budget_activity() {
    let doc = basic();
    let snapshot = doc.clone();
    let (_, d, w) = demand(&doc);
    let budget = SharedBudget::new(1 << 20);
    let scope = budget
        .reserve_capacity(1 << 20)
        .unwrap()
        .scope(AllocationClass::PaintMetadata);
    let original = spatial::WorkspaceAdmission::new(scope.clone(), 1 << 20, 268435456);
    original.reserve_work(17).unwrap();
    let before = (
        budget.usage(),
        budget.activity(),
        scope.footprint(),
        original.work_used(),
    );
    let cost = source8_role_query_cost().unwrap();
    assert_eq!(cost.formation_work, 2 * cost.control_bytes + 16384);
    assert!(cost.control_bytes >= 4 * size_of::<Source8RoleDemand>() as u64);
    let q = source8_role_demand_checked(&d, &w).unwrap();
    assert!(q.total_bytes <= 64 << 20);
    assert_eq!(
        (
            budget.usage(),
            budget.activity(),
            scope.footprint(),
            original.work_used()
        ),
        before
    );
    assert!(doc.same_semantics(&snapshot));
}
#[test]
fn incompatible_target_noise_geometry_and_oversized_union_are_typed_refusals() {
    let (_, d, w) = demand(&basic());
    let mut cases = Vec::new();
    let mut x = d;
    x.pointer_bytes = if usize::BITS == 64 { 4 } else { 8 };
    cases.push(x);
    let mut x = d;
    x.stored_noise = true;
    cases.push(x);
    let mut x = d;
    x.canvas[0] = 0;
    cases.push(x);
    let mut x = d;
    x.encoded_bytes = 64 << 20;
    cases.push(x);
    let mut x = d;
    x.encoded_bytes = u64::MAX;
    cases.push(x);
    for x in cases {
        assert_eq!(
            source8_role_demand_checked(&x, &w).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
    }
    for kind in 0..3 {
        let mut bad = w;
        if kind == 0 {
            bad.workspace_bytes += 1;
        }
        if kind == 1 {
            bad.display_cells += 1;
        }
        if kind == 2 {
            bad.noise_sidecar_bytes = 1;
        }
        assert!(source8_role_demand_checked(&d, &bad).is_err());
    }
}
#[test]
fn pure_query_does_not_capture_or_renew_installed_authority() {
    let (_, d, w) = demand(&basic());
    let budget = SharedBudget::new(1 << 20);
    let root = spatial::WorkspaceAdmission::new(
        budget.scope(AllocationClass::PaintMetadata),
        1 << 20,
        100,
    );
    root.reserve_work(3).unwrap();
    spatial::with_workspace(root.clone(), || {
        source8_role_demand_checked(&d, &w)?;
        spatial::capture_current_workspace()
            .unwrap()
            .reserve_work(2)?;
        Ok(())
    })
    .unwrap();
    assert_eq!(root.work_used(), 5);
}
#[test]
fn actual_reader_exact_a_and_k_and_minus_one_use_original_factories() {
    use crate::source8::{Source8ReadCaps, entry::Entry8, tests::Host};
    let (wire, d, w) = demand(&basic());
    let q = source8_role_demand_checked(&d, &w).unwrap();
    for short_role in [None, Some(3), Some(7)] {
        let mut host = Host::new(q.roles[5]);
        let r = q.roles;
        let scope = |n| {
            host.budget
                .reserve_capacity(n)
                .unwrap()
                .scope(AllocationClass::PaintMetadata)
        };
        host.p = scope(r[1]);
        host.m = scope(r[2]);
        host.a = scope(r[3] - u64::from(short_role == Some(3)));
        host.k = scope(r[7] - u64::from(short_role == Some(7)));
        host.caps = Source8ReadCaps {
            p: r[1],
            m: r[2],
            a: r[3] - u64::from(short_role == Some(3)),
            c: r[5],
            i: 0,
            q: 0,
            k: r[7] - u64::from(short_role == Some(7)),
            staged_pixels: 0,
        };
        host.raster.paint_metadata_scope = host.m.clone();
        let result = spatial::with_workspace(host.root.clone(), || {
            let check = || Ok(());
            let mut entry = Entry8::new(host.request(), host.realms(host.caps), &check)?;
            entry.read_into_slot_checked(&wire)
        });
        if short_role.is_none() {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err().code, ErrorCode::ResourceLimit);
        }
        assert_eq!(host.m.footprint().live, 0);
        assert_eq!(host.c.footprint().live, 0);
        assert_eq!(host.a.footprint().live, 0);
        assert_eq!(host.k.footprint().live, 0);
    }
}

#[test]
fn real_icc_payload_and_disabled_stored_noise_use_defining_retention_and_paired_overlap() {
    use paintcraft_core::types::{
        filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution},
        ids::deterministic_id,
    };
    use paintcraft_document::{FilterInstance, FilterStack};
    let mut doc = source("icc-profile-none");
    let profile = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../assets/profiles/sRGB2014.icc"
    ));
    let budget = SharedBudget::new(1 << 20);
    let descriptor = *doc.source_metadata.as_ref().unwrap().descriptor();
    doc.source_metadata = Some(
        SourceMetadata::capture(descriptor, Some(profile), 1 << 20, &budget, &|| Ok(())).unwrap(),
    );
    let value = Descriptor::AddNoiseRgbV1(
        AddNoiseRgbV1::new(100, 17, false, NoiseDistribution::Uniform).unwrap(),
    );
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            deterministic_id(&["role-query", "disabled-noise"]),
            false,
            value,
        )])
        .unwrap();
    let (_, d, w) = demand(&doc);
    let q = source8_role_demand_checked(&d, &w).unwrap();
    assert!(d.stored_noise);
    assert_eq!(d.retained.profile_bytes(), profile.len() as u64);
    assert_eq!(
        q.roles[4],
        SourceMetadata::reserved_retained_quote(profile.len() as u64).unwrap()
    );
    assert_eq!(
        q.paired_noise_bytes,
        NoiseAdmission::control_quote().unwrap().counter_bytes
    );
    assert_eq!(w.noise_sidecar_bytes, q.paired_noise_bytes);
    assert_eq!(
        q.total_bytes,
        q.roles.iter().sum::<u64>() + q.paired_noise_bytes
    );
    assert!(q.roles[8] >= q.paired_noise_bytes);
}
