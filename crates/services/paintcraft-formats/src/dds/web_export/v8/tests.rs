//! Authored actual consumer controls; expected pixels come from frozen math data.
use super::*;
use paintcraft_core::types::filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution};
use paintcraft_document::{FilterInstance, FilterStack};
fn doc(distribution: NoiseDistribution, enabled: bool, amount: i16) -> Document {
    let mut doc =
        paintcraft_document::import_image(1, 1, &[64, 16, 128, 127], &"13".repeat(32)).unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(712),
            enabled,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(amount, 1, true, distribution).unwrap()),
        )])
        .unwrap();
    doc
}
fn options() -> TextureOptions {
    TextureOptions {
        encoding: Encoding::Rgba8,
        transfer: Transfer::Srgb,
        alpha: SourceAlpha::Straight,
        mips: MipCount::Levels(1),
        filter: MipFilter::Nearest,
        quality: TextureQuality::Normal,
    }
}
fn plan(doc: &Document, workload: Workload) -> NoiseSourcePlan {
    let target = export::metadata_policy::Target::new(
        crate::registry::Family::Dds,
        crate::registry::Operation::ExportFlat,
    );
    let bake = export::filter_bake::Policy::capture(
        doc,
        target,
        export::filter_bake::Intent::Bake,
        &|| Ok(()),
    )
    .unwrap();
    NoiseSourcePlan::capture_with_policies_checked(
        doc,
        options(),
        export::CopyPolicies {
            source_metadata: Default::default(),
            filter_bake: bake,
        },
        workload,
        &|| Ok(()),
    )
    .unwrap()
}
fn artifact(doc: &Document) -> (Header, Vec<u8>) {
    let plan = plan(
        doc,
        Workload::browser(NoiseJobPolicy::BrowserV1.max_limits()).unwrap(),
    );
    let source = plan.snapshot(doc).unwrap();
    let owner = Owner {
        session: doc.document_id,
        state: doc.state_id,
        epoch: 0,
        frontend: 3,
    };
    let header = Header::capture(&plan, &source, owner, doc.document_id).unwrap();
    let mut request = header.encode().unwrap().to_vec();
    request.extend(source);
    (header, request)
}
#[test]
fn actual_uniform_gaussian_dds_bytes_and_preview_bind_independent_literals() {
    // Frozen Effects oracle afc142: coordinate0,0 seed1 amount10 mono, RGBA64/16/128/127.
    for (mode, pixel) in [
        (NoiseDistribution::Uniform, [86, 38, 150, 127]),
        (NoiseDistribution::Gaussian, [70, 22, 134, 127]),
    ] {
        let doc = doc(mode, true, 10);
        let before = pcnd::write(&doc).unwrap();
        let (h, request) = artifact(&doc);
        let response = execute(&request, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
        let Frame::Success {
            encoded, preview, ..
        } = decode_response(&response, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap()
        else {
            panic!("expected actual DDS8 success")
        };
        assert_eq!(preview, pixel);
        assert_eq!(
            decode_top_mip_checked(encoded, PIXELS_MAX, &|| Ok(()))
                .unwrap()
                .rgba,
            pixel
        );
        assert_eq!(pcnd::write(&doc).unwrap(), before);
    }
}
#[test]
fn disabled_and_default_stored_intent_require8_and_legacy7_stays_closed() {
    for (mode, enabled, amount) in [
        (NoiseDistribution::Uniform, false, 0),
        (NoiseDistribution::Gaussian, false, 10),
        (NoiseDistribution::Gaussian, true, 0),
    ] {
        let doc = doc(mode, enabled, amount);
        let (h, request) = artifact(&doc);
        assert_eq!(h.spatial.stored_noise(), 1);
        assert!(SourcePlan::capture(&doc, options()).is_err());
        let response = execute(&request, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
        let Frame::Success { preview, .. } =
            decode_response(&response, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap()
        else {
            panic!("identity stored intent must be admitted")
        };
        assert_eq!(preview, [64, 16, 128, 127]);
    }
}
#[test]
fn each_context_workload_source_and_response_echo_is_strict() {
    let (h, request) = artifact(&doc(NoiseDistribution::Uniform, true, 10));
    for at in [
        8, 10, 123, 256, 258, 260, 262, 368, 384, 386, 388, 390, 608, 640, 664, 672,
    ] {
        let mut bad = request.clone();
        bad[at] ^= 1;
        assert!(
            Header::decode(&bad[..REQUEST_HEADER]).is_err(),
            "mutated offset{at}"
        );
    }
    let mut bad = request.clone();
    *bad.last_mut().unwrap() ^= 1;
    let refused = execute(&bad, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
    assert!(matches!(
        decode_response(&refused, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap(),
        Frame::Refused(_)
    ));
    let response = execute(&request, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
    for at in [8, 32, 73, 89, 121, 265, 288, 320, 384, 416, 544, 800] {
        let mut b = response.clone();
        b[at] ^= 1;
        assert!(decode_response(&b, &h, h.owner, env!("CARGO_PKG_VERSION")).is_err());
    }
}
#[test]
fn low_component_quotes_and_missing_bake_refuse_without_new_caps() {
    let d = doc(NoiseDistribution::Gaussian, true, 10);
    let full = Workload::browser(NoiseJobPolicy::BrowserV1.max_limits()).unwrap();
    let p = plan(&d, full);
    for limits in [
        NoiseLimits {
            base_work: 0,
            ..full.limits()
        },
        NoiseLimits {
            noise_work: 1,
            ..full.limits()
        },
    ] {
        let target = export::metadata_policy::Target::new(
            crate::registry::Family::Dds,
            crate::registry::Operation::ExportFlat,
        );
        let bake = export::filter_bake::Policy::capture(
            &d,
            target,
            export::filter_bake::Intent::Bake,
            &|| Ok(()),
        )
        .unwrap();
        assert!(
            NoiseSourcePlan::capture_with_policies_checked(
                &d,
                options(),
                export::CopyPolicies {
                    source_metadata: Default::default(),
                    filter_bake: bake
                },
                Workload::browser(limits).unwrap(),
                &|| Ok(())
            )
            .is_err()
        );
    }
    assert!(
        NoiseSourcePlan::capture_with_policies_checked(
            &d,
            options(),
            Default::default(),
            full,
            &|| Ok(())
        )
        .is_err()
    );
    assert_eq!(
        p.workload().limits(),
        NoiseJobPolicy::BrowserV1.max_limits()
    );
}
#[test]
fn exact_source_residency_and_stack_capacity_do_not_change_producer_consumer_facts() {
    let mut d = doc(NoiseDistribution::Uniform, true, 10);
    let f = d
        .layers
        .get(&d.roots[0])
        .unwrap()
        .filters
        .iter()
        .next()
        .unwrap()
        .clone();
    let mut v = Vec::with_capacity(8);
    v.push(f);
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(v).unwrap();
    let (h, request) = artifact(&d);
    let source = &request[REQUEST_HEADER..];
    let read_budget = paintcraft_core::types::resources::SharedBudget::new(4 << 20);
    let decoded = pcnd::read_with_admission_checked(
        source,
        PIXELS_MAX,
        STAGED_LAYER_PIXELS_MAX,
        LAYERS_MAX,
        pcnd::ReadAdmission {
            metadata: pcnd::MetadataAdmission {
                bytes: 1 << 20,
                budget: &read_budget,
            },
            coverage: pcnd::CoverageAdmission {
                bytes: 1 << 20,
                budget: &read_budget,
                limits: Default::default(),
            },
        },
        &|| Ok(()),
    )
    .unwrap();
    let p = plan(&decoded, h.workload);
    let (a, b, _) = p.snapshot_contexts(source, &|| Ok(())).unwrap();
    assert_eq!(a, h.spatial);
    assert_eq!(b, h.programs);
}

#[test]
fn installed_legacy_parent_cannot_be_upgraded_by_the_isolated_endpoint() {
    let (h, request) = artifact(&doc(NoiseDistribution::Uniform, true, 10));
    let budget = paintcraft_core::types::resources::SharedBudget::new(64 << 20);
    let reservation = budget.reserve_capacity(64 << 20).unwrap();
    let scope = reservation.scope(paintcraft_core::types::resources::AllocationClass::SharedBudget);
    let parent = paintcraft_render::spatial::WorkspaceAdmission::new(scope, 64 << 20, 100);
    let response = paintcraft_render::spatial::with_workspace(parent, || {
        execute(&request, env!("CARGO_PKG_VERSION"), &|| Ok(()))
    })
    .unwrap();
    let Frame::Refused(error) =
        decode_response(&response, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap()
    else {
        panic!("legacy caller must refuse")
    };
    assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
}
#[test]
fn cancellation_preserves_frozen_source_and_never_claims_encoded_success() {
    let (h, request) = artifact(&doc(NoiseDistribution::Gaussian, true, 10));
    let before = request.clone();
    let cancelled = || {
        Err(Error::new(
            ErrorCode::OperationCancelled,
            "test cancellation",
        ))
    };
    assert_eq!(
        execute(&request, env!("CARGO_PKG_VERSION"), &cancelled)
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    let calls = std::cell::Cell::new(0);
    let response = execute(&request, env!("CARGO_PKG_VERSION"), &|| {
        calls.set(calls.get() + 1);
        if calls.get() >= 10 {
            cancelled()
        } else {
            Ok(())
        }
    })
    .unwrap();
    assert!(matches!(
        decode_response(&response, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap(),
        Frame::Refused(Error {
            code: ErrorCode::OperationCancelled,
            ..
        })
    ));
    assert_eq!(request, before);
}
