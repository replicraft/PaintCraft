//! Actual closed Worker6 entries; these quote assertions are same-native-ABI only.
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    filters::{Descriptor, LevelsRgbV1},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{Document, FilterInstance, FilterStack};
use paintcraft_formats::{
    codec_worker::Owner,
    dds::{self, web_export as d},
    export::{self, filter_bake, metadata_policy},
    msp::web_export as m,
    pcnd,
    registry::{Family, Operation},
};
use std::cell::Cell;
use uuid::Uuid;
#[derive(Clone, Copy)]
enum Codec {
    Dds,
    Msp,
}
fn options() -> dds::TextureOptions {
    dds::TextureOptions {
        encoding: dds::Encoding::Rgba8,
        transfer: dds::Transfer::Srgb,
        alpha: dds::SourceAlpha::Straight,
        mips: dds::MipCount::Levels(1),
        filter: dds::MipFilter::Nearest,
        quality: dds::TextureQuality::Normal,
    }
}
fn policies(doc: &Document, codec: Codec) -> export::CopyPolicies {
    let target = metadata_policy::Target::new(
        match codec {
            Codec::Dds => Family::Dds,
            Codec::Msp => Family::Msp,
        },
        Operation::ExportFlat,
    );
    export::CopyPolicies {
        source_metadata: metadata_policy::Policy::capture(
            doc,
            target,
            metadata_policy::Intent::Omit,
        )
        .unwrap(),
        filter_bake: filter_bake::Policy::capture(doc, target, filter_bake::Intent::Bake, &|| {
            Ok(())
        })
        .unwrap(),
    }
}
fn source(width: u32, height: u32, enabled: bool, gamma: i16) -> Document {
    let mut doc = paintcraft_document::new_document(
        width,
        height,
        Some(paintcraft_core::types::Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(17),
            enabled,
            Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, gamma, 0, 255).unwrap()),
        )])
        .unwrap();
    doc
}
fn owner(doc: &Document) -> Owner {
    Owner {
        session: doc.document_id,
        state: doc.state_id,
        epoch: 3,
        frontend: 3,
    }
}
fn packet(doc: &Document, codec: Codec) -> Vec<u8> {
    let owner = owner(doc);
    let mut request = match codec {
        Codec::Dds => {
            let plan =
                d::SourcePlan::capture_with_policies(doc, options(), policies(doc, codec)).unwrap();
            assert_eq!(plan.protocol_version(), 6);
            let source = plan.snapshot(doc).unwrap();
            assert_eq!(source[4], 6);
            assert!(d::Header::capture(&plan, &source, owner, doc.document_id).is_err());
            assert!(d::v5::Header::capture(&plan, &source, owner, doc.document_id).is_err());
            let exact = plan.spatial_context(source.len()).unwrap();
            let plus = plan.spatial_context(source.len() + 1).unwrap();
            assert_eq!(
                u64::from_be_bytes(plus[40..48].try_into().unwrap())
                    - u64::from_be_bytes(exact[40..48].try_into().unwrap()),
                1
            );
            let mut bytes = d::v6::Header::capture(&plan, &source, owner, doc.document_id)
                .unwrap()
                .encode()
                .to_vec();
            bytes.extend(source);
            bytes
        }
        Codec::Msp => {
            let plan = m::SourcePlan::capture_with_policies(doc, policies(doc, codec)).unwrap();
            assert_eq!(plan.protocol_version(), 6);
            let source = plan.snapshot(doc).unwrap();
            assert_eq!(source[4], 6);
            assert!(m::Header::capture(&plan, &source, owner, doc.document_id).is_err());
            assert!(m::v5::Header::capture(&plan, &source, owner, doc.document_id).is_err());
            let exact = plan.spatial_context(source.len()).unwrap();
            let plus = plan.spatial_context(source.len() + 1).unwrap();
            assert_eq!(
                u64::from_be_bytes(plus[40..48].try_into().unwrap())
                    - u64::from_be_bytes(exact[40..48].try_into().unwrap()),
                1
            );
            let mut bytes = m::v6::Header::capture(&plan, &source, owner, doc.document_id)
                .unwrap()
                .encode()
                .to_vec();
            bytes.extend(source);
            bytes
        }
    };
    assert!(request.len() > 384);
    request.shrink_to_fit();
    request
}
enum Outcome {
    Success(Vec<u8>, Vec<u8>, Vec<String>),
    Refused(Error),
}
fn execute(request: &[u8], codec: Codec, check: &dyn Fn() -> Result<()>) -> Result<Outcome> {
    match codec {
        Codec::Dds => {
            let header = d::v6::Header::decode(&request[..384])?;
            let result = d::v6::execute(request, "test", check)?;
            match d::v6::decode_response(&result, &header, header.owner, "test")? {
                d::v6::Frame::Success {
                    encoded,
                    preview,
                    losses,
                    ..
                } => Ok(Outcome::Success(
                    encoded.to_vec(),
                    preview.to_vec(),
                    losses.iter().map(|s| s.to_string()).collect(),
                )),
                d::v6::Frame::Refused(e) => Ok(Outcome::Refused(e)),
            }
        }
        Codec::Msp => {
            let header = m::v6::Header::decode(&request[..384])?;
            let result = m::v6::execute(request, "test", check)?;
            match m::v6::decode_response(&result, &header, header.owner, "test")? {
                m::v6::Frame::Success {
                    encoded,
                    preview,
                    losses,
                    ..
                } => Ok(Outcome::Success(
                    encoded.to_vec(),
                    preview.to_vec(),
                    losses.iter().map(|s| s.to_string()).collect(),
                )),
                m::v6::Frame::Refused(e) => Ok(Outcome::Refused(e)),
            }
        }
    }
}
fn error(result: Result<Outcome>) -> Error {
    match result {
        Err(e) | Ok(Outcome::Refused(e)) => e,
        _ => panic!("unexpected success"),
    }
}
#[test]
fn actual_catalog_entries_preserve_literal_pixels_disabled_intent_and_exact_finalization() {
    for codec in [Codec::Dds, Codec::Msp] {
        for enabled in [false, true] {
            let doc = source(2, 1, enabled, 200);
            let original = pcnd::write(&doc).unwrap();
            let request = packet(&doc, codec);
            let Outcome::Success(encoded, preview, losses) =
                execute(&request, codec, &|| Ok(())).unwrap()
            else {
                panic!("valid catalog refused")
            };
            assert_eq!(preview, [255; 8]);
            assert!(losses.iter().any(|l| l == filter_bake::BAKED_REASON));
            assert!(!losses.iter().any(|l| l == metadata_policy::OMITTED_REASON));
            let decoded = match codec {
                Codec::Dds => {
                    dds::decode_top_mip_checked(&encoded, 2, &|| Ok(()))
                        .unwrap()
                        .rgba
                }
                Codec::Msp => paintcraft_formats::msp::decode(&encoded, &|| Ok(()))
                    .unwrap()
                    .rgba(&|| Ok(()))
                    .unwrap(),
            };
            assert_eq!(decoded, [255; 8]);
            assert_eq!(pcnd::write(&doc).unwrap(), original);
        }
    }
}
#[test]
fn catalog_preparation_is_once_per_frame_beyond_chunk_boundary_and_reuses_parent_domain() {
    for codec in [Codec::Dds, Codec::Msp] {
        let doc = source(257, 257, true, 997);
        let request = packet(&doc, codec);
        let compile = u64::from_be_bytes(request[336..344].try_into().unwrap());
        let combined = u64::from_be_bytes(request[368..376].try_into().unwrap());
        assert!(
            compile > combined - compile,
            "test requires compile to dominate region work"
        );
        let budget = SharedBudget::new(64 << 20);
        let parent = paintcraft_render::spatial::WorkspaceAdmission::new(
            budget.scope(AllocationClass::SharedBudget),
            64 << 20,
            export::SPATIAL_WORK_MAX,
        );
        let result = paintcraft_render::spatial::with_workspace(parent.clone(), || {
            execute(&request, codec, &|| Ok(()))
        })
        .unwrap();
        let Outcome::Success(_, preview, _) = result else {
            panic!("catalog large frame refused")
        };
        assert!(preview.iter().all(|v| *v == 255));
        assert!(parent.work_used() >= compile);
        assert!(
            parent.work_used() < 2 * compile,
            "program recompiled across chunks"
        );
        assert_eq!(budget.usage().reserved, 0);
        let empty = SharedBudget::new(0);
        let parent = paintcraft_render::spatial::WorkspaceAdmission::new(
            empty.scope(AllocationClass::SharedBudget),
            0,
            export::SPATIAL_WORK_MAX,
        );
        let failure = paintcraft_render::spatial::with_workspace(parent, || {
            execute(&request, codec, &|| Ok(()))
        });
        assert_eq!(error(failure).code, ErrorCode::ResourceLimit);
        assert_eq!(empty.usage().reserved, 0);
    }
}
#[test]
fn catalog_context_tampering_and_wrong_profiles_never_produce_outputs() {
    for codec in [Codec::Dds, Codec::Msp] {
        let doc = source(2, 1, true, 100);
        let original = packet(&doc, codec);
        for at in [
            256, 258, 268, 304, 320, 322, 332, 336, 344, 352, 360, 368, 376,
        ] {
            let mut bytes = original.clone();
            bytes[at] ^= 1;
            assert!(matches!(
                error(execute(&bytes, codec, &|| Ok(()))).code,
                ErrorCode::InputSchema | ErrorCode::CodecFailed | ErrorCode::ResourceLimit
            ));
        }
        let mut bytes = original.clone();
        bytes[384 + 4] = 5;
        assert_eq!(
            error(execute(&bytes, codec, &|| Ok(()))).code,
            ErrorCode::CodecFailed
        );
        // Authenticated profile6 containing a structurally valid future requirement
        // must hit this consumer's explicit initial-eight closure before PCND decode.
        let mut bytes = original.clone();
        let from = b"levels-rgb-v1";
        let to = b"future-rgb-v1";
        let source = &mut bytes[384..];
        let at = source.windows(from.len()).position(|w| w == from).unwrap();
        source[at..at + from.len()].copy_from_slice(to);
        let end = source.len() - 32;
        use sha2::{Digest, Sha256};
        let outer = Sha256::digest(&source[..end]);
        source[end..].copy_from_slice(&outer);
        let sha = Sha256::digest(source);
        bytes[81..113].copy_from_slice(&sha);
        let e = error(execute(&bytes, codec, &|| Ok(())));
        assert_eq!(e.code, ErrorCode::CapabilityUnsupported);
        assert_eq!(
            e.message,
            "Export Worker6 admits only initial eight profiles/params-v1"
        );
    }
}
#[test]
fn catalog_cancellation_at_every_checkpoint_preserves_source_and_releases_parent_scope() {
    for codec in [Codec::Dds, Codec::Msp] {
        let doc = source(2, 1, true, 100);
        let request = packet(&doc, codec);
        let before = pcnd::write(&doc).unwrap();
        let visits = Cell::new(0);
        let result = execute(&request, codec, &|| {
            visits.set(visits.get() + 1);
            Ok(())
        })
        .unwrap();
        assert!(matches!(result, Outcome::Success(..)));
        for stop in 1..=visits.get() {
            let budget = SharedBudget::new(64 << 20);
            let parent = paintcraft_render::spatial::WorkspaceAdmission::new(
                budget.scope(AllocationClass::SharedBudget),
                64 << 20,
                export::SPATIAL_WORK_MAX,
            );
            let at = Cell::new(0);
            let result = paintcraft_render::spatial::with_workspace(parent, || {
                execute(&request, codec, &|| {
                    at.set(at.get() + 1);
                    if at.get() == stop {
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "cancel catalog export",
                        ))
                    } else {
                        Ok(())
                    }
                })
            });
            assert_eq!(
                error(result).code,
                ErrorCode::OperationCancelled,
                "checkpoint{stop}"
            );
            assert_eq!(budget.usage().reserved, 0);
            assert_eq!(pcnd::write(&doc).unwrap(), before);
        }
    }
}

#[test]
fn authenticated_pcnd6_never_inherits_closed_worker4_or_worker5_admission() {
    use paintcraft_core::types::filters::BoxBlurV1;
    let doc = source(2, 1, true, 100);
    let bytes = pcnd::write(&doc).unwrap();
    for codec in [Codec::Dds, Codec::Msp] {
        let mut legacy = doc.clone();
        legacy.layers.get_mut(&legacy.roots[0]).unwrap().filters = Default::default();
        let mut box_doc = legacy.clone();
        box_doc.layers.get_mut(&box_doc.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                Uuid::from_u128(99),
                false,
                Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap()),
            )])
            .unwrap();
        match codec {
            Codec::Dds => {
                let plan = d::SourcePlan::capture_with_policies(
                    &legacy,
                    options(),
                    policies(&legacy, codec),
                )
                .unwrap();
                let header =
                    d::Header::capture(&plan, &bytes, owner(&legacy), legacy.document_id).unwrap();
                let mut request = header.encode().to_vec();
                request.extend(&bytes);
                let response = d::execute(&request, "test", &|| Ok(())).unwrap();
                let d::Frame::Refused(e) =
                    d::decode_response(&response, &header, header.owner, "test").unwrap()
                else {
                    panic!("worker4 adopted6")
                };
                assert_eq!(
                    e.message,
                    "Export Worker4 source profile admits only PCND1 through PCND4"
                );
                let plan = d::SourcePlan::capture_with_policies(
                    &box_doc,
                    options(),
                    policies(&box_doc, codec),
                )
                .unwrap();
                let header =
                    d::v5::Header::capture(&plan, &bytes, owner(&box_doc), box_doc.document_id)
                        .unwrap();
                let mut request = header.encode().to_vec();
                request.extend(&bytes);
                let response = d::v5::execute(&request, "test", &|| Ok(())).unwrap();
                let d::v5::Frame::Refused(e) =
                    d::v5::decode_response(&response, &header, header.owner, "test").unwrap()
                else {
                    panic!("worker5 adopted6")
                };
                assert_eq!(e.message, "Export Worker5 requires PCND5 source");
            }
            Codec::Msp => {
                let plan = m::SourcePlan::capture_with_policies(&legacy, policies(&legacy, codec))
                    .unwrap();
                let header =
                    m::Header::capture(&plan, &bytes, owner(&legacy), legacy.document_id).unwrap();
                let mut request = header.encode().to_vec();
                request.extend(&bytes);
                let response = m::execute(&request, "test", &|| Ok(())).unwrap();
                let m::Frame::Refused(e) =
                    m::decode_response(&response, &header, header.owner, "test").unwrap()
                else {
                    panic!("worker4 adopted6")
                };
                assert_eq!(
                    e.message,
                    "Export Worker4 source profile admits only PCND1 through PCND4"
                );
                let plan =
                    m::SourcePlan::capture_with_policies(&box_doc, policies(&box_doc, codec))
                        .unwrap();
                let header =
                    m::v5::Header::capture(&plan, &bytes, owner(&box_doc), box_doc.document_id)
                        .unwrap();
                let mut request = header.encode().to_vec();
                request.extend(&bytes);
                let response = m::v5::execute(&request, "test", &|| Ok(())).unwrap();
                let m::v5::Frame::Refused(e) =
                    m::v5::decode_response(&response, &header, header.owner, "test").unwrap()
                else {
                    panic!("worker5 adopted6")
                };
                assert_eq!(e.message, "Export Worker5 requires PCND5 source");
            }
        }
    }
}
#[test]
fn catalog_bake_and_source_omission_are_independent_original_snapshot_witnesses() {
    let mut doc = source(2, 1, true, 100);
    doc.source_metadata = pcnd::read(include_bytes!("pcnd-metadata-fixtures/rgb-profile.pcnd"))
        .unwrap()
        .source_metadata;
    for codec in [Codec::Dds, Codec::Msp] {
        let target = metadata_policy::Target::new(
            match codec {
                Codec::Dds => Family::Dds,
                Codec::Msp => Family::Msp,
            },
            Operation::ExportFlat,
        );
        let capture = |p| match codec {
            Codec::Dds => d::SourcePlan::capture_with_policies(&doc, options(), p).map(|_| ()),
            Codec::Msp => m::SourcePlan::capture_with_policies(&doc, p).map(|_| ()),
        };
        let full = policies(&doc, codec);
        assert_eq!(
            capture(export::CopyPolicies {
                filter_bake: Default::default(),
                ..full
            })
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(
            capture(export::CopyPolicies {
                source_metadata: Default::default(),
                ..full
            })
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        let original = pcnd::write(&doc).unwrap();
        let request = packet(&doc, codec);
        let Outcome::Success(_, _, losses) = execute(&request, codec, &|| Ok(())).unwrap() else {
            panic!("explicit catalog copy refused")
        };
        assert!(losses.iter().any(|l| l == metadata_policy::OMITTED_REASON));
        assert!(losses.iter().any(|l| l == filter_bake::BAKED_REASON));
        assert_eq!(pcnd::write(&doc).unwrap(), original);
        let mut changed = doc.clone();
        changed.state_id = Uuid::new_v4();
        assert_eq!(
            full.source_metadata
                .validate(&changed, target)
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
        assert_eq!(
            full.filter_bake
                .validate(&changed, target, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ConflictStaleBase
        );
    }
}

#[test]
fn catalog_completed_responses_still_bind_current_owner_and_original_program_context() {
    let doc = source(2, 1, true, 100);
    for codec in [Codec::Dds, Codec::Msp] {
        let request = packet(&doc, codec);
        match codec {
            Codec::Dds => {
                let header = d::v6::Header::decode(&request[..384]).unwrap();
                let mut response = d::v6::execute(&request, "test", &|| Ok(())).unwrap();
                let mut stale = header.owner;
                stale.epoch += 1;
                assert_eq!(
                    d::v6::decode_response(&response, &header, stale, "test")
                        .unwrap_err()
                        .code,
                    ErrorCode::ConflictStaleBase
                );
                response[480 + 24] ^= 1;
                assert!(d::v6::decode_response(&response, &header, header.owner, "test").is_err());
            }
            Codec::Msp => {
                let header = m::v6::Header::decode(&request[..384]).unwrap();
                let mut response = m::v6::execute(&request, "test", &|| Ok(())).unwrap();
                let mut stale = header.owner;
                stale.epoch += 1;
                assert_eq!(
                    m::v6::decode_response(&response, &header, stale, "test")
                        .unwrap_err()
                        .code,
                    ErrorCode::ConflictStaleBase
                );
                response[480 + 24] ^= 1;
                assert!(m::v6::decode_response(&response, &header, header.owner, "test").is_err());
            }
        }
    }
}

#[test]
fn catalog_dds_gamma_and_single_fractional_mask_mix_match_independent_literal_tuples() {
    // gamma2 full endpoints gives round(sqrt(channel*255)); expected bytes were
    // checked independently with integer-square comparisons, then rational A8.
    const RAW: [u8; 8] = [64, 255, 16, 255, 16, 64, 255, 255];
    for enabled in [false, true] {
        let mut doc = paintcraft_document::import_image(2, 1, &RAW, &"12".repeat(32)).unwrap();
        let budget = SharedBudget::new(1 << 20);
        let mask = paintcraft_raster::coverage_mask::A8Mask::uniform(
            paintcraft_core::types::Rect::new(0, 0, 2, 1).unwrap(),
            128,
            &budget,
            Default::default(),
            &|| Ok(()),
        )
        .unwrap();
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters = FilterStack::new(vec![
            FilterInstance::new(
                Uuid::from_u128(18),
                true,
                Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, 200, 0, 255).unwrap()),
            )
            .with_mask(Some(paintcraft_document::FilterMask::new(enabled, mask))),
        ])
        .unwrap();
        let original = pcnd::write(&doc).unwrap();
        let request = packet(&doc, Codec::Dds);
        let Outcome::Success(encoded, preview, _) =
            execute(&request, Codec::Dds, &|| Ok(())).unwrap()
        else {
            panic!("masked catalog refused")
        };
        let expected = if enabled {
            [96, 255, 40, 255, 40, 96, 255, 255]
        } else {
            [128, 255, 64, 255, 64, 128, 255, 255]
        };
        assert_eq!(preview, expected);
        assert_eq!(
            dds::decode_top_mip_checked(&encoded, 2, &|| Ok(()))
                .unwrap()
                .rgba,
            expected
        );
        assert_eq!(pcnd::write(&doc).unwrap(), original);
    }
}
