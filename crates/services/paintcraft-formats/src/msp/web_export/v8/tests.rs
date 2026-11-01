//! Source-authored consumer tests. No product encoder produces expected bytes.
use super::*;
use paintcraft_core::types::filters::{
    AddNoiseRgbV1, Descriptor, NoiseDistribution, ThresholdLumaV1,
};
use paintcraft_document::{FilterInstance, FilterStack};
const VERSIONS: [Version; 2] = [Version::UncompressedV1, Version::RowRleV2];
fn doc(
    mode: NoiseDistribution,
    enabled: bool,
    amount: i16,
    threshold: Option<i16>,
    alpha: u8,
) -> Document {
    let mut d = paintcraft_document::import_image(
        2,
        1,
        &[64, 16, 128, alpha, 64, 16, 128, alpha],
        &"13".repeat(32),
    )
    .unwrap();
    let mut filters = vec![FilterInstance::new(
        Uuid::from_u128(712),
        enabled,
        Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(amount, 65535, false, mode).unwrap()),
    )];
    if let Some(level) = threshold {
        filters.push(FilterInstance::new(
            Uuid::from_u128(713),
            true,
            Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(level).unwrap()),
        ));
    }
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(filters).unwrap();
    d
}
fn policies(d: &Document) -> export::CopyPolicies {
    let target = export::metadata_policy::Target::new(
        crate::registry::Family::Msp,
        crate::registry::Operation::ExportFlat,
    );
    export::CopyPolicies {
        source_metadata: export::metadata_policy::Policy::capture(
            d,
            target,
            if d.source_metadata.is_some() {
                export::metadata_policy::Intent::Omit
            } else {
                export::metadata_policy::Intent::Refuse
            },
        )
        .unwrap(),
        filter_bake: export::filter_bake::Policy::capture(
            d,
            target,
            export::filter_bake::Intent::Bake,
            &|| Ok(()),
        )
        .unwrap(),
    }
}
fn plan(d: &Document, v: Version, w: Workload) -> NoiseSourcePlan {
    NoiseSourcePlan::capture_with_policies_checked(d, v, policies(d), w, &|| Ok(())).unwrap()
}
fn artifact(d: &Document, v: Version) -> (Header, Vec<u8>) {
    let p = plan(d, v, default_workload().unwrap());
    let source = p.snapshot(d).unwrap();
    let owner = Owner {
        session: d.document_id,
        state: d.state_id,
        epoch: 0,
        frontend: 3,
    };
    let h = Header::capture(&p, &source, owner, d.document_id).unwrap();
    let mut bytes = h.encode().unwrap().to_vec();
    bytes.extend(source);
    (h, bytes)
}
fn response(h: &Header, request: &[u8]) -> Vec<u8> {
    let bytes = execute(request, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
    // Force binding validation for every successful helper invocation.
    decode_response(&bytes, h, h.owner, env!("CARGO_PKG_VERSION")).unwrap();
    bytes
}
#[test]
fn default1_and_explicit2_actual_noise_then_threshold_match_independent_msp_bytes() {
    // Qualified independent Noise literals: Uniform100/seed65535/color gives
    // [255,211,0] and [51,0,97]. Threshold128 => white/black. Gaussian gives
    // [47,139,0] and [0,4,46]; Threshold64 => white/black. Raw source yields
    // black/black under either threshold, so white is causal, not identity.
    for (mode, level) in [
        (NoiseDistribution::Uniform, 128),
        (NoiseDistribution::Gaussian, 64),
    ] {
        let d = doc(mode, true, 100, Some(level), 255);
        let before = pcnd::write(&d).unwrap();
        for v in VERSIONS {
            let (h, r) = artifact(&d, v);
            let bytes = response(&h, &r);
            let Frame::Success {
                encoded, preview, ..
            } = decode_response(&bytes, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap()
            else {
                panic!("MSP8 must succeed")
            };
            let expected: &[u8] = match v {
                Version::UncompressedV1 => {
                    include_bytes!("../../../../tests/msp-noise-v8-fixtures/white-black-v1.msp")
                }
                Version::RowRleV2 => {
                    include_bytes!("../../../../tests/msp-noise-v8-fixtures/white-black-v2.msp")
                }
            };
            assert_eq!(encoded, expected);
            assert_eq!(preview, [255, 255, 255, 255, 0, 0, 0, 255]);
            assert_eq!(
                crate::msp::decode(encoded, &|| Ok(()))
                    .unwrap()
                    .rgba(&|| Ok(()))
                    .unwrap(),
                preview
            );
            assert_eq!(pcnd::write(&d).unwrap(), before);
        }
    }
}
#[test]
fn chromatic_and_nonopaque_sources_refuse_without_implicit_threshold_or_matte() {
    for (threshold, alpha) in [(None, 255), (Some(128), 127)] {
        let d = doc(NoiseDistribution::Uniform, true, 100, threshold, alpha);
        for v in VERSIONS {
            let (h, r) = artifact(&d, v);
            let b = response(&h, &r);
            let Frame::Refused(e) =
                decode_response(&b, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap()
            else {
                panic!("BW must be explicit")
            };
            assert_eq!(e.code, ErrorCode::CapabilityUnsupported);
            assert!(e.message.contains("opaque black/white"));
        }
    }
}
#[test]
fn disabled_and_zero_stored_noise_use8_while_legacy_plan_stays_closed() {
    for (enabled, amount) in [(false, 100), (true, 0), (false, 0)] {
        let d = doc(NoiseDistribution::Uniform, enabled, amount, Some(128), 255);
        assert!(SourcePlan::capture(&d).is_err());
        for v in VERSIONS {
            let (h, r) = artifact(&d, v);
            assert_eq!(h.spatial.stored_noise(), 1);
            let b = response(&h, &r);
            let Frame::Success { preview, .. } =
                decode_response(&b, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap()
            else {
                panic!("stored identity must succeed")
            };
            assert_eq!(preview, [0, 0, 0, 255, 0, 0, 0, 255]);
        }
    }
}
#[test]
fn selected_version_settings_and_response_magic_are_exact_and_cross_replays_refuse() {
    let d = doc(NoiseDistribution::Uniform, true, 100, Some(128), 255);
    let (a, ra) = artifact(&d, Version::UncompressedV1);
    let (b, rb) = artifact(&d, Version::RowRleV2);
    assert_eq!(a.settings(), [1, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(b.settings(), [2, 0, 0, 0, 0, 0, 0, 0]);
    assert_ne!(export_variant(a.version), export_variant(b.version));
    let pa = plan(&d, a.version, a.workload);
    assert!(!pa.matches_context(&b));
    let ba = response(&a, &ra);
    let bb = response(&b, &rb);
    assert!(decode_response(&ba, &b, b.owner, env!("CARGO_PKG_VERSION")).is_err());
    assert!(decode_response(&bb, &a, a.owner, env!("CARGO_PKG_VERSION")).is_err());
    for version in [0, 3, 255] {
        let mut raw = ra.clone();
        raw[113] = version;
        assert!(Header::decode(&raw[..REQUEST_HEADER]).is_err());
    }
    for at in [114, 115, 116, 117, 118, 119, 120, 123, 127] {
        let mut raw = ra.clone();
        raw[at] = 1;
        assert!(Header::decode(&raw[..REQUEST_HEADER]).is_err());
    }
    let mut forged = ba.clone();
    forged[121] = 2;
    assert!(decode_response(&forged, &a, a.owner, env!("CARGO_PKG_VERSION")).is_err());
}
#[test]
fn contexts_source_and_abi_quotes_are_checked_before_encoded_publication() {
    let d = doc(NoiseDistribution::Uniform, true, 100, Some(128), 255);
    let (h, r) = artifact(&d, VERSIONS[0]);
    for at in [
        8, 10, 256, 258, 260, 262, 368, 384, 386, 388, 390, 608, 640, 664, 672,
    ] {
        let mut bad = r.clone();
        bad[at] ^= 1;
        assert!(
            Header::decode(&bad[..REQUEST_HEADER]).is_err(),
            "offset {at}"
        );
    }
    let mut bad = r.clone();
    *bad.last_mut().unwrap() ^= 1;
    let out = response(&h, &bad);
    assert!(matches!(
        decode_response(&out, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap(),
        Frame::Refused(_)
    ));
    let good = response(&h, &r);
    for at in [32, 73, 89, 121, 265, 266, 288, 320, 384, 416, 544, 800] {
        let mut bad = good.clone();
        bad[at] ^= 1;
        assert!(decode_response(&bad, &h, h.owner, env!("CARGO_PKG_VERSION")).is_err());
    }
}
#[test]
fn version_specific_phase_quotes_use_max_not_sum_and_actual_encoded_extent() {
    let d = doc(NoiseDistribution::Uniform, true, 100, Some(128), 255);
    for v in VERSIONS {
        let (h, _) = artifact(&d, v);
        let extra = encode_extra(v, h.size).unwrap();
        assert_eq!(
            extra,
            match v {
                Version::UncompressedV1 => 34,
                Version::RowRleV2 => 1 + ENCODED_BYTES_MAX as u64,
            }
        );
        let render = h
            .programs
            .phase_payload(h.spatial, h.workload, 0, WORKER_PAYLOAD_BYTES_MAX)
            .unwrap();
        let encode = h
            .programs
            .phase_payload(h.spatial, h.workload, extra, WORKER_PAYLOAD_BYTES_MAX)
            .unwrap();
        assert!(encode >= render);
        assert!(encode <= WORKER_PAYLOAD_BYTES_MAX);
        assert!(
            h.programs
                .phase_payload(h.spatial, h.workload, extra, encode - 1)
                .is_err()
        );
        assert!(
            h.programs
                .phase_payload(h.spatial, h.workload, extra, encode)
                .is_ok()
        );
    }
    assert!(decode_settings([1, 1, 0, 0, 0, 0, 0, 0], (2, 1)).is_err());
    assert!(RgbaLayout::new(Version::UncompressedV1, 65536, 1).is_err());
    assert_eq!(WORKER_PAYLOAD_BYTES_MAX, 160 << 20);
    assert_eq!(RESERVATION_BYTES, 512 << 20);
}
#[test]
fn cumulative_work_limits_bake_and_installed_parent_cannot_be_bypassed() {
    let d = doc(NoiseDistribution::Uniform, true, 100, Some(128), 255);
    let w = default_workload().unwrap();
    for limits in [
        NoiseLimits {
            base_work: 0,
            ..w.limits()
        },
        NoiseLimits {
            noise_work: 1,
            ..w.limits()
        },
    ] {
        assert!(
            NoiseSourcePlan::capture_with_policies_checked(
                &d,
                VERSIONS[0],
                policies(&d),
                Workload::browser(limits).unwrap(),
                &|| Ok(())
            )
            .is_err()
        );
    }
    assert!(
        NoiseSourcePlan::capture_with_policies_checked(
            &d,
            VERSIONS[0],
            Default::default(),
            w,
            &|| Ok(())
        )
        .is_err()
    );
    let (h, r) = artifact(&d, VERSIONS[0]);
    let budget = paintcraft_core::types::resources::SharedBudget::new(64 << 20);
    let reservation = budget.reserve_capacity(64 << 20).unwrap();
    let scope = reservation.scope(paintcraft_core::types::resources::AllocationClass::SharedBudget);
    let parent = paintcraft_render::spatial::WorkspaceAdmission::new(scope, 64 << 20, 100);
    let out = paintcraft_render::spatial::with_workspace(parent, || {
        execute(&r, env!("CARGO_PKG_VERSION"), &|| Ok(()))
    })
    .unwrap();
    let Frame::Refused(e) = decode_response(&out, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap()
    else {
        panic!("nested caller must refuse")
    };
    assert_eq!(e.code, ErrorCode::CapabilityUnsupported);
}
#[test]
fn cancellation_releases_job_without_mutating_the_frozen_source() {
    let d = doc(NoiseDistribution::Gaussian, true, 100, Some(64), 255);
    let (h, r) = artifact(&d, VERSIONS[1]);
    let before = r.clone();
    let calls = std::cell::Cell::new(0);
    let out = execute(&r, env!("CARGO_PKG_VERSION"), &|| {
        calls.set(calls.get() + 1);
        if calls.get() >= 10 {
            Err(Error::new(ErrorCode::OperationCancelled, "cancel MSP8"))
        } else {
            Ok(())
        }
    })
    .unwrap();
    let Frame::Refused(e) = decode_response(&out, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap()
    else {
        panic!("cancel must not publish")
    };
    assert_eq!(e.code, ErrorCode::OperationCancelled);
    assert_eq!(r, before);
}
#[test]
fn real_spatial_workspace_and_codec_extra_are_disjoint_not_added_twice() {
    use paintcraft_core::types::filters::BoxBlurV1;
    let mut d = doc(NoiseDistribution::Uniform, true, 100, Some(128), 255);
    let layer = d.layers.get_mut(&d.roots[0]).unwrap();
    let mut f = layer.filters.iter().cloned().collect::<Vec<_>>();
    f.insert(
        1,
        FilterInstance::new(
            Uuid::from_u128(714),
            true,
            Descriptor::BoxBlurV1(BoxBlurV1::new(1).unwrap()),
        ),
    );
    layer.filters = FilterStack::new(f).unwrap();
    let (h, r) = artifact(&d, Version::UncompressedV1);
    let extra = encode_extra(h.version, h.size).unwrap();
    assert!(h.spatial.workspace_bytes() >= extra);
    assert_eq!(
        h.programs
            .phase_payload(h.spatial, h.workload, 0, WORKER_PAYLOAD_BYTES_MAX)
            .unwrap(),
        h.programs
            .phase_payload(h.spatial, h.workload, extra, WORKER_PAYLOAD_BYTES_MAX)
            .unwrap()
    );
    let out = response(&h, &r);
    assert!(matches!(
        decode_response(&out, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap(),
        Frame::Success { .. }
    ));
}
#[test]
fn both_registry_variants_are_explicit_web_studio_copies_without_legacy_widening() {
    use crate::registry::{self, Family, Frontend, Operation, Platform, Request};
    for v in VERSIONS {
        let mut r = Request::new(
            Frontend::Studio,
            Operation::ExportFlat,
            Family::Msp,
            Platform::Web,
        );
        r.extension = Some("msp");
        r.variant = Some(export_variant(v));
        assert!(registry::admission(r).available());
        for denied in [
            Request {
                frontend: Frontend::ClassicV11,
                ..r
            },
            Request {
                operation: Operation::SavePreserving,
                ..r
            },
            Request {
                variant: Some("web-msp-unknown-worker-v8"),
                ..r
            },
        ] {
            assert!(!registry::admission(denied).available());
        }
    }
}
#[test]
fn real_default_tile_elision_and_spare_filter_capacity_use_authenticated_snapshot_shape() {
    use paintcraft_core::types::{Rgba, resources::SharedBudget};
    use paintcraft_raster::{Admission, RasterLayer};
    let mut d = doc(NoiseDistribution::Uniform, false, 0, None, 255);
    d.canvas = (128, 128);
    let id = d.roots[0];
    let mut raster = RasterLayer::new(128, 128, Rgba([255; 4]));
    raster
        .set_block(
            0,
            0,
            &[255u8; 4].repeat(128 * 128),
            &mut Admission::new(65_536),
        )
        .unwrap();
    assert_eq!(raster.resident_tiles(), 1);
    let layer = d.layers.get_mut(&id).unwrap();
    layer.raster = raster;
    let mut spare = Vec::with_capacity(8);
    spare.extend(layer.filters.iter().cloned());
    layer.filters = FilterStack::new(spare).unwrap();
    for v in VERSIONS {
        let (h, request) = artifact(&d, v);
        let source = &request[REQUEST_HEADER..];
        let budget = SharedBudget::new(4 << 20);
        let restored = pcnd::read_with_admission_checked(
            source,
            PIXELS_MAX,
            STAGED_LAYER_PIXELS_MAX,
            LAYERS_MAX,
            pcnd::ReadAdmission {
                metadata: pcnd::MetadataAdmission {
                    bytes: 1 << 20,
                    budget: &budget,
                },
                coverage: pcnd::CoverageAdmission {
                    bytes: 1 << 20,
                    budget: &budget,
                    limits: Default::default(),
                },
            },
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(restored.layers[&id].raster.resident_tiles(), 0);
        let p = plan(&restored, v, h.workload);
        let (a, b, _) = p.snapshot_contexts(source, &|| Ok(())).unwrap();
        assert_eq!(a, h.spatial);
        assert_eq!(b, h.programs);
        let out = response(&h, &request);
        assert!(matches!(
            decode_response(&out, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap(),
            Frame::Success { .. }
        ));
        assert_eq!(d.layers[&id].raster.resident_tiles(), 1);
    }
}
#[test]
fn actual_masks_apply_once_before_explicit_threshold_and_metadata_omit_is_separate() {
    use paintcraft_core::types::{Rect, resources::SharedBudget};
    use paintcraft_document::FilterMask;
    use paintcraft_raster::coverage_mask::A8Mask;
    for (coverage, enabled, white) in [
        (0, true, false),
        (127, true, false),
        (255, true, true),
        (127, false, true),
    ] {
        let mut d = doc(NoiseDistribution::Uniform, true, 100, Some(128), 255);
        d.source_metadata = pcnd::read(include_bytes!(
            "../../../../tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
        ))
        .unwrap()
        .source_metadata;
        let mask = A8Mask::uniform(
            Rect::new(0, 0, 2, 1).unwrap(),
            coverage,
            &SharedBudget::new(8192),
            Default::default(),
            &|| Ok(()),
        )
        .unwrap();
        let l = d.layers.get_mut(&d.roots[0]).unwrap();
        let mut f = l.filters.iter().cloned().collect::<Vec<_>>();
        f[0] = f[0].with_mask(Some(FilterMask::new(enabled, mask)));
        l.filters = FilterStack::new(f).unwrap();
        let before = pcnd::write(&d).unwrap();
        for v in VERSIONS {
            let (h, r) = artifact(&d, v);
            assert!(h.metadata.omitted());
            assert_ne!(h.source_flags & 8, 0);
            let out = response(&h, &r);
            let Frame::Success {
                preview, losses, ..
            } = decode_response(&out, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap()
            else {
                panic!("masked exact BW must succeed")
            };
            let first = if white { 255 } else { 0 };
            assert_eq!(preview, [first, first, first, 255, 0, 0, 0, 255]);
            assert!(losses.contains(&export::metadata_policy::OMITTED_REASON));
            assert!(losses.contains(&export::filter_bake::BAKED_REASON));
            assert_eq!(pcnd::write(&d).unwrap(), before);
        }
    }
}
