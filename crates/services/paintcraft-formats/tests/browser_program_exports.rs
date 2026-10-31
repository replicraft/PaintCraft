//! Actual generic Worker7, using this producer/consumer native ABI only.
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    filters::{Descriptor, ExposureSrgbV1},
    resources::SharedBudget,
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
fn source(codec: Codec, enabled: bool, stops: i16) -> Document {
    let rgba = match codec {
        Codec::Dds => [64, 16, 128, 255],
        Codec::Msp => [1, 1, 1, 255],
    };
    let mut doc = paintcraft_document::import_image(1, 1, &rgba, "literal Exposure input").unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(17),
            enabled,
            Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(stops).unwrap()),
        )])
        .unwrap();
    doc
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
fn owner(doc: &Document) -> Owner {
    Owner {
        session: doc.document_id,
        state: doc.state_id,
        epoch: 3,
        frontend: 3,
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
fn packet(doc: &Document, codec: Codec) -> Vec<u8> {
    let source = pcnd::write_compact(doc).unwrap();
    assert_eq!(source[4], 6);
    let mut packet = match codec {
        Codec::Dds => {
            let plan =
                d::SourcePlan::capture_with_policies(doc, options(), policies(doc, codec)).unwrap();
            assert_eq!(plan.protocol_version(), 7);
            assert_eq!(plan.snapshot(doc).unwrap(), source);
            assert!(d::v6::Header::capture(&plan, &source, owner(doc), doc.document_id).is_err());
            let header =
                d::v7::Header::capture(&plan, &source, owner(doc), doc.document_id).unwrap();
            let described = plan.spatial_context(source.len()).unwrap();
            let extra = plan.spatial_context(source.len() + 1).unwrap();
            assert_eq!(
                u64::from_be_bytes(extra[40..48].try_into().unwrap())
                    - u64::from_be_bytes(described[40..48].try_into().unwrap()),
                1
            );
            let actual = plan.snapshot_contexts(&source, &|| Ok(())).unwrap();
            assert_eq!(header.spatial.encode_programs(), actual.region);
            assert_eq!(header.programs.encode(), actual.programs);
            assert_eq!(header.source_sha256, actual.source_sha256);
            assert_eq!(
                plan.program_context(source.len()).unwrap(),
                header.programs.encode()
            );
            header.encode().to_vec()
        }
        Codec::Msp => {
            let plan = m::SourcePlan::capture_with_policies(doc, policies(doc, codec)).unwrap();
            assert_eq!(plan.protocol_version(), 7);
            assert_eq!(plan.snapshot(doc).unwrap(), source);
            assert!(m::v6::Header::capture(&plan, &source, owner(doc), doc.document_id).is_err());
            let header =
                m::v7::Header::capture(&plan, &source, owner(doc), doc.document_id).unwrap();
            let described = plan.spatial_context(source.len()).unwrap();
            let extra = plan.spatial_context(source.len() + 1).unwrap();
            assert_eq!(
                u64::from_be_bytes(extra[40..48].try_into().unwrap())
                    - u64::from_be_bytes(described[40..48].try_into().unwrap()),
                1
            );
            let actual = plan.snapshot_contexts(&source, &|| Ok(())).unwrap();
            assert_eq!(header.spatial.encode_programs(), actual.region);
            assert_eq!(header.programs.encode(), actual.programs);
            assert_eq!(header.source_sha256, actual.source_sha256);
            assert_eq!(
                plan.program_context(source.len()).unwrap(),
                header.programs.encode()
            );
            header.encode().to_vec()
        }
    };
    assert_eq!(packet.len(), 448);
    packet.extend(source);
    packet
}
enum Outcome {
    Success(Vec<u8>, Vec<u8>, Vec<String>),
    Refused(Error),
}
fn execute(packet: &[u8], codec: Codec, check: &dyn Fn() -> Result<()>) -> Result<Outcome> {
    match codec {
        Codec::Dds => {
            let header = d::v7::Header::decode(&packet[..448])?;
            let response = d::v7::execute(packet, "test", check)?;
            match d::v7::decode_response(&response, &header, header.owner, "test")? {
                d::v7::Frame::Success {
                    encoded,
                    preview,
                    losses,
                    ..
                } => Ok(Outcome::Success(
                    encoded.to_vec(),
                    preview.to_vec(),
                    losses.iter().map(|s| s.to_string()).collect(),
                )),
                d::v7::Frame::Refused(error) => Ok(Outcome::Refused(error)),
            }
        }
        Codec::Msp => {
            let header = m::v7::Header::decode(&packet[..448])?;
            let response = m::v7::execute(packet, "test", check)?;
            match m::v7::decode_response(&response, &header, header.owner, "test")? {
                m::v7::Frame::Success {
                    encoded,
                    preview,
                    losses,
                    ..
                } => Ok(Outcome::Success(
                    encoded.to_vec(),
                    preview.to_vec(),
                    losses.iter().map(|s| s.to_string()).collect(),
                )),
                m::v7::Frame::Refused(error) => Ok(Outcome::Refused(error)),
            }
        }
    }
}
fn error(outcome: Result<Outcome>) -> Error {
    match outcome {
        Err(error) | Ok(Outcome::Refused(error)) => error,
        _ => panic!("unexpected output"),
    }
}
#[test]
fn actual_program_entries_render_causal_literals_and_preserve_disabled_and_privacy_intent() {
    for codec in [Codec::Dds, Codec::Msp] {
        for enabled in [false, true] {
            // EV+1 linear-light causal color, and EV-8 where opaque byte1 ->0.
            // Disabled MSP uses white, since arbitrary grey must NOT be quantized.
            let stops = match codec {
                Codec::Dds => 100,
                Codec::Msp => -800,
            };
            let mut doc = source(codec, enabled, stops);
            if !enabled && matches!(codec, Codec::Msp) {
                doc = paintcraft_document::import_image(1, 1, &[255; 4], "disabled MSP white")
                    .unwrap();
                doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
                    FilterStack::new(vec![FilterInstance::new(
                        Uuid::from_u128(17),
                        false,
                        Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(stops).unwrap()),
                    )])
                    .unwrap();
            }
            let original = pcnd::write(&doc).unwrap();
            let request = packet(&doc, codec);
            let Outcome::Success(encoded, preview, losses) =
                execute(&request, codec, &|| Ok(())).unwrap()
            else {
                panic!("valid request refused")
            };
            let expected = match (codec, enabled) {
                (Codec::Dds, true) => [90, 26, 176, 255],
                (Codec::Dds, false) => [64, 16, 128, 255],
                (Codec::Msp, true) => [0, 0, 0, 255],
                (Codec::Msp, false) => [255; 4],
            };
            assert_eq!(preview, expected);
            let decoded = match codec {
                Codec::Dds => {
                    dds::decode_top_mip_checked(&encoded, 1, &|| Ok(()))
                        .unwrap()
                        .rgba
                }
                Codec::Msp => paintcraft_formats::msp::decode(&encoded, &|| Ok(()))
                    .unwrap()
                    .rgba(&|| Ok(()))
                    .unwrap(),
            };
            assert_eq!(decoded, expected);
            assert!(losses.iter().any(|l| l == filter_bake::BAKED_REASON));
            assert!(!losses.iter().any(|l| l == metadata_policy::OMITTED_REASON));
            assert_eq!(pcnd::write(&doc).unwrap(), original);
        }
    }
    // MSP remains appearance-strict: disabled Exposure cannot make byte1 B/W.
    let doc = source(Codec::Msp, false, -800);
    assert_eq!(
        error(execute(&packet(&doc, Codec::Msp), Codec::Msp, &|| Ok(()))).code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn appended_facts_tampering_and_bake_refusal_never_publish() {
    for codec in [Codec::Dds, Codec::Msp] {
        let doc = source(codec, false, 0);
        let original = packet(&doc, codec);
        for at in 256..448 {
            let mut bytes = original.clone();
            bytes[at] ^= 1;
            let failure = error(execute(&bytes, codec, &|| Ok(())));
            assert!(matches!(
                failure.code,
                ErrorCode::InputSchema | ErrorCode::CodecFailed | ErrorCode::ResourceLimit
            ));
        }
        let mut bytes = original;
        bytes[122] = 0;
        assert_eq!(
            error(execute(&bytes, codec, &|| Ok(()))).code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
#[test]
fn worker_program_cancellation_and_parent_quota_preserve_source() {
    for codec in [Codec::Dds, Codec::Msp] {
        let doc = source(
            codec,
            true,
            match codec {
                Codec::Dds => 100,
                Codec::Msp => -800,
            },
        );
        let before = pcnd::write(&doc).unwrap();
        let request = packet(&doc, codec);
        let seen = Cell::new(0);
        let _ = execute(&request, codec, &|| {
            seen.set(seen.get() + 1);
            Ok(())
        })
        .unwrap();
        let total = seen.get();
        assert!(total > 2);
        for stop in [1, total] {
            let at = Cell::new(0);
            let failure = error(execute(&request, codec, &|| {
                at.set(at.get() + 1);
                if at.get() >= stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "caller cancellation",
                    ))
                } else {
                    Ok(())
                }
            }));
            assert_eq!(failure.code, ErrorCode::OperationCancelled);
            assert_eq!(pcnd::write(&doc).unwrap(), before);
        }
        let empty = SharedBudget::new(0);
        let scope = empty.scope(paintcraft_core::types::resources::AllocationClass::SharedBudget);
        let admission = paintcraft_render::spatial::WorkspaceAdmission::new(
            scope,
            64 << 20,
            export::SPATIAL_WORK_MAX,
        );
        let failure = paintcraft_render::spatial::with_workspace(admission, || {
            Ok(error(execute(&request, codec, &|| Ok(()))))
        })
        .unwrap();
        assert_eq!(failure.code, ErrorCode::ResourceLimit);
        assert_eq!(empty.usage().reserved, 0);
    }
}

#[test]
fn registry_and_inner_integrity_refuse_before_generic_adoption_on_authenticated_requests() {
    use sha2::{Digest, Sha256};
    for codec in [Codec::Dds, Codec::Msp] {
        let doc = source(codec, false, 0);
        let original = packet(&doc, codec);
        let mut unknown = original.clone();
        let source = &mut unknown[448..];
        let old = b"exposure-srgb-v1";
        let new = b"futurexx-srgb-v1";
        assert_eq!(old.len(), new.len());
        let at = source
            .windows(old.len())
            .position(|bytes| bytes == old)
            .unwrap();
        source[at..at + old.len()].copy_from_slice(new);
        let end = source.len() - 32;
        let digest = Sha256::digest(&source[..end]);
        source[end..].copy_from_slice(&digest);
        let digest = Sha256::digest(source);
        unknown[81..113].copy_from_slice(&digest);
        let failure = error(execute(&unknown, codec, &|| Ok(())));
        assert_eq!(failure.code, ErrorCode::CapabilityUnsupported);
        assert_eq!(
            failure.message,
            "Prepared worker implementation or pricing is unavailable"
        );
        let mut inner = original;
        *inner.last_mut().unwrap() ^= 1;
        let digest = Sha256::digest(&inner[448..]);
        inner[81..113].copy_from_slice(&digest);
        let failure = error(execute(&inner, codec, &|| Ok(())));
        assert_eq!(failure.code, ErrorCode::CodecFailed);
        assert_eq!(failure.message, "compact PCND SHA-256 mismatch");
    }
}

#[path = "browser_program_exports/curves.rs"]
mod curves;

#[path = "browser_program_exports/vibrance.rs"]
mod vibrance;
