//! Real Worker5 paths, independent finite-source literals, and closed legacy4.
use paintcraft_core::types::{
    Error, ErrorCode,
    filters::{BoxBlurV1, Descriptor},
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_document::{Document, FilterInstance, FilterStack};
use paintcraft_formats::{
    codec_worker::Owner,
    dds::{self, web_export as d},
    export::{self, filter_bake, metadata_policy},
    msp::web_export as m,
    pcnd,
};
const VERSION: &str = "0.1.0";
#[derive(Clone, Copy)]
enum Codec {
    Dds,
    Msp,
}
fn source(codec: Codec, enabled: bool, radius: i16) -> Document {
    let raw = match codec {
        Codec::Dds => [255, 0, 0, 255, 0, 255, 0, 255],
        Codec::Msp if enabled && radius > 0 => [0, 0, 0, 255, 0, 0, 0, 255],
        Codec::Msp => [0, 0, 0, 255, 255, 255, 255, 255],
    };
    let mut doc = paintcraft_document::import_image(2, 1, &raw, &"12".repeat(32)).unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            uuid::Uuid::from_u128(89),
            enabled,
            Descriptor::BoxBlurV1(BoxBlurV1::new(radius).unwrap()),
        )])
        .unwrap();
    doc
}
fn policies(codec: Codec, doc: &Document) -> export::CopyPolicies {
    let family = match codec {
        Codec::Dds => paintcraft_formats::registry::Family::Dds,
        Codec::Msp => paintcraft_formats::registry::Family::Msp,
    };
    let target =
        metadata_policy::Target::new(family, paintcraft_formats::registry::Operation::ExportFlat);
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
fn owner(doc: &Document) -> Owner {
    Owner {
        session: doc.document_id,
        state: doc.state_id,
        epoch: 0,
        frontend: 3,
    }
}
fn packet(codec: Codec, doc: &Document) -> Vec<u8> {
    let source = pcnd::write_compact(doc).unwrap();
    let owner = owner(doc);
    let mut bytes = match codec {
        Codec::Dds => {
            let plan =
                d::SourcePlan::capture_with_policies(doc, options(), policies(codec, doc)).unwrap();
            assert_eq!(plan.protocol_version(), 5);
            assert!(d::Header::capture(&plan, &source, owner, doc.document_id).is_err());
            let exact = plan.spatial_context(source.len()).unwrap();
            let plus_one = plan.spatial_context(source.len() + 1).unwrap();
            assert_eq!(
                u64::from_be_bytes(plus_one[40..48].try_into().unwrap())
                    - u64::from_be_bytes(exact[40..48].try_into().unwrap()),
                1
            );
            d::v5::Header::capture(&plan, &source, owner, doc.document_id)
                .unwrap()
                .encode()
                .to_vec()
        }
        Codec::Msp => {
            let plan = m::SourcePlan::capture_with_policies(doc, policies(codec, doc)).unwrap();
            assert_eq!(plan.protocol_version(), 5);
            assert!(m::Header::capture(&plan, &source, owner, doc.document_id).is_err());
            let exact = plan.spatial_context(source.len()).unwrap();
            let plus_one = plan.spatial_context(source.len() + 1).unwrap();
            assert_eq!(
                u64::from_be_bytes(plus_one[40..48].try_into().unwrap())
                    - u64::from_be_bytes(exact[40..48].try_into().unwrap()),
                1
            );
            m::v5::Header::capture(&plan, &source, owner, doc.document_id)
                .unwrap()
                .encode()
                .to_vec()
        }
    };
    assert_eq!(bytes.len(), 320);
    bytes.extend(source);
    bytes
}
enum Outcome {
    Success(Vec<u8>, Vec<u8>),
    Refused(Error),
}
fn execute(
    codec: Codec,
    bytes: &[u8],
    check: &dyn Fn() -> paintcraft_core::types::Result<()>,
) -> paintcraft_core::types::Result<Outcome> {
    match codec {
        Codec::Dds => {
            let header = d::v5::Header::decode(&bytes[..320])?;
            let response = d::v5::execute(bytes, VERSION, check)?;
            match d::v5::decode_response(&response, &header, header.owner, VERSION)? {
                d::v5::Frame::Success {
                    encoded, preview, ..
                } => Ok(Outcome::Success(encoded.to_vec(), preview.to_vec())),
                d::v5::Frame::Refused(e) => Ok(Outcome::Refused(e)),
            }
        }
        Codec::Msp => {
            let header = m::v5::Header::decode(&bytes[..320])?;
            let response = m::v5::execute(bytes, VERSION, check)?;
            match m::v5::decode_response(&response, &header, header.owner, VERSION)? {
                m::v5::Frame::Success {
                    encoded, preview, ..
                } => Ok(Outcome::Success(encoded.to_vec(), preview.to_vec())),
                m::v5::Frame::Refused(e) => Ok(Outcome::Refused(e)),
            }
        }
    }
}
fn error(result: paintcraft_core::types::Result<Outcome>) -> Error {
    match result {
        Err(e) | Ok(Outcome::Refused(e)) => e,
        Ok(Outcome::Success(..)) => panic!("expected refusal"),
    }
}
#[test]
fn worker5_real_dds_clamp_and_msp_disabled_radius_zero_keep_source_and_legacy4_closed() {
    for (codec, enabled, radius, expected) in [
        (Codec::Dds, true, 1, [170, 85, 0, 255, 85, 170, 0, 255]),
        (Codec::Dds, false, 32, [255, 0, 0, 255, 0, 255, 0, 255]),
        (Codec::Msp, true, 0, [0, 0, 0, 255, 255, 255, 255, 255]),
        (Codec::Msp, false, 32, [0, 0, 0, 255, 255, 255, 255, 255]),
        (Codec::Msp, true, 1, [0, 0, 0, 255, 0, 0, 0, 255]),
    ] {
        let doc = source(codec, enabled, radius);
        let raw = doc.layers[&doc.roots[0]].raster.clone();
        let bytes = packet(codec, &doc);
        if !enabled {
            assert_eq!(u64::from_be_bytes(bytes[280..288].try_into().unwrap()), 0);
            assert!(u64::from_be_bytes(bytes[272..280].try_into().unwrap()) > 0);
            assert!(u64::from_be_bytes(bytes[288..296].try_into().unwrap()) > 0);
        }

        let Outcome::Success(encoded, preview) = execute(codec, &bytes, &|| Ok(())).unwrap() else {
            panic!("valid literal refused")
        };
        assert_eq!(preview, expected);
        assert!(!encoded.is_empty());
        assert_eq!(doc.layers[&doc.roots[0]].raster, raw);
        // Reframe to genuine4 header while retaining exact PCND5 and original source hash.
        let mut legacy = bytes[..256].to_vec();
        legacy[7] = b'4';
        legacy[8..10].copy_from_slice(&4u16.to_be_bytes());
        legacy.extend_from_slice(&bytes[320..]);
        let budget = SharedBudget::new(256 << 20);
        let admission = paintcraft_render::spatial::WorkspaceAdmission::new(
            budget.scope(AllocationClass::SharedBudget),
            256 << 20,
            u64::MAX,
        );
        paintcraft_render::spatial::with_workspace(admission, || {
            let refused = match codec {
                Codec::Dds => {
                    let h = d::Header::decode(&legacy[..256])?;
                    let response = d::execute(&legacy, VERSION, &|| Ok(()))?;
                    matches!(
                        d::decode_response(&response, &h, h.owner, VERSION)?,
                        d::Frame::Refused(error) if error.code==ErrorCode::CapabilityUnsupported
                    )
                }
                Codec::Msp => {
                    let h = m::Header::decode(&legacy[..256])?;
                    let response = m::execute(&legacy, VERSION, &|| Ok(()))?;
                    matches!(
                        m::decode_response(&response, &h, h.owner, VERSION)?,
                        m::Frame::Refused(error) if error.code==ErrorCode::CapabilityUnsupported
                    )
                }
            };
            assert!(refused);
            Ok(())
        })
        .unwrap();
    }
}
#[test]
fn worker5_context_tamper_and_tiny_parent_never_create_alternate_workspace() {
    for codec in [Codec::Dds, Codec::Msp] {
        let doc = source(codec, true, 0);
        let bytes = packet(codec, &doc);
        for at in [263usize, 271, 279, 287, 295, 303] {
            let mut bad = bytes.clone();
            bad[at] ^= 1;
            assert!(matches!(
                error(execute(codec, &bad, &|| Ok(()))).code,
                ErrorCode::InputSchema | ErrorCode::ResourceLimit | ErrorCode::CodecFailed
            ));
        }
        for at in [268usize, 304] {
            let mut bad = bytes.clone();
            bad[at] = 1;
            assert_eq!(
                error(execute(codec, &bad, &|| Ok(()))).code,
                ErrorCode::InputSchema
            );
        }
        let budget = SharedBudget::new(1);
        let parent = paintcraft_render::spatial::WorkspaceAdmission::new(
            budget.scope(AllocationClass::SharedBudget),
            1,
            1_000_000_000,
        );
        paintcraft_render::spatial::with_workspace(parent, || {
            assert_eq!(
                error(execute(codec, &bytes, &|| Ok(()))).code,
                ErrorCode::ResourceLimit
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.usage().reserved, 0);
    }
}
#[test]
fn worker5_cancellation_at_every_successful_checkpoint_is_never_swallowed() {
    use std::cell::Cell;
    for codec in [Codec::Dds, Codec::Msp] {
        let bytes = packet(codec, &source(codec, true, 0));
        let count = Cell::new(0);
        execute(codec, &bytes, &|| {
            count.set(count.get() + 1);
            Ok(())
        })
        .unwrap();
        assert!(count.get() > 10);
        for stop in 1..=count.get() {
            let visits = Cell::new(0);
            let result = execute(codec, &bytes, &|| {
                visits.set(visits.get() + 1);
                if visits.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "one-shot test cancellation",
                    ))
                } else {
                    Ok(())
                }
            });
            assert_eq!(
                error(result).code,
                ErrorCode::OperationCancelled,
                "checkpoint {stop}"
            );
        }
    }
}

#[test]
fn worker5_masked_clamp_and_retained_source_context_remain_independent_authorities() {
    use paintcraft_core::types::Rect;
    use paintcraft_document::FilterMask;
    use paintcraft_raster::coverage_mask::{A8Mask, Limits};
    let mut doc = source(Codec::Dds, true, 1);
    let coverage = A8Mask::uniform(
        Rect::new(0, 0, 2, 1).unwrap(),
        128,
        &SharedBudget::new(8192),
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.filters = FilterStack::new(vec![
        layer.filters.as_slice()[0]
            .clone()
            .with_mask(Some(FilterMask::new(true, coverage))),
    ])
    .unwrap();
    let attachment = pcnd::read(include_bytes!("pcnd-metadata-fixtures/rgb-profile.pcnd"))
        .unwrap()
        .source_metadata;
    doc = doc
        .with_source_metadata_checked(attachment, &|| Ok(()))
        .unwrap();
    let before = pcnd::write_compact(&doc).unwrap();
    let request = packet(Codec::Dds, &doc);
    let Outcome::Success(_, preview) = execute(Codec::Dds, &request, &|| Ok(())).unwrap() else {
        panic!("mask/profile copy refused")
    };
    assert_eq!(preview, [212, 43, 0, 255, 43, 212, 0, 255]);
    assert_eq!(pcnd::write_compact(&doc).unwrap(), before);
    for at in [122usize, 208, 224, 256 + 7] {
        let mut bad = request.clone();
        bad[at] ^= 1;
        assert!(matches!(
            error(execute(Codec::Dds, &bad, &|| Ok(()))).code,
            ErrorCode::CapabilityUnsupported
                | ErrorCode::CodecFailed
                | ErrorCode::InputSchema
                | ErrorCode::ResourceLimit
                | ErrorCode::ConflictStaleBase
        ));
    }
}

#[test]
fn msp5_exact_phase_peak_uses_larger_workspace_or_encode_stage_without_summing_both() {
    use paintcraft_core::types::Rgba;
    for (size, count, radius, workspace_dominates) in [(2, 1, 0, false), (256, 3, 32, true)] {
        let mut doc = paintcraft_document::new_document(
            size,
            size,
            Some(Rgba([255; 4])),
            112,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
        assert_eq!(layer.raster.resident_tiles(), 0);
        layer.filters = FilterStack::new(
            (0..count)
                .map(|n| {
                    FilterInstance::new(
                        uuid::Uuid::from_u128(100 + n),
                        true,
                        Descriptor::BoxBlurV1(BoxBlurV1::new(radius).unwrap()),
                    )
                })
                .collect(),
        )
        .unwrap();
        let plan = m::SourcePlan::capture_with_policies(&doc, policies(Codec::Msp, &doc)).unwrap();
        let source = plan.snapshot(&doc).unwrap();
        let context = plan.spatial_context(source.len()).unwrap();
        let workspace = u64::from_be_bytes(context[24..32].try_into().unwrap());
        let payload = u64::from_be_bytes(context[40..48].try_into().unwrap());
        let layout = paintcraft_formats::msp::RgbaLayout::new(
            paintcraft_formats::msp::Version::RowRleV2,
            size,
            size,
        )
        .unwrap();
        let encode = layout.packed_bytes() as u64 + m::ENCODED_BYTES_MAX as u64;
        assert_eq!(workspace > encode, workspace_dominates);
        let common = export::msp::render_reservation_bytes(layout).unwrap()
            + source.len() as u64
            + export::filter_bake::inspect(&doc, &|| Ok(()))
                .unwrap()
                .filter_storage_bytes()
            + (8 << 20)
            + (1 << 20)
            + m::ENCODED_BYTES_MAX as u64
            + 2 * m::PREVIEW_BYTES_MAX as u64;
        assert_eq!(payload, common + workspace.max(encode));
        assert!(payload < common + workspace + encode);
        let Outcome::Success(_, preview) =
            execute(Codec::Msp, &packet(Codec::Msp, &doc), &|| Ok(())).unwrap()
        else {
            panic!("admitted two-phase source refused")
        };
        assert!(preview.iter().all(|b| *b == 255));
    }
}
