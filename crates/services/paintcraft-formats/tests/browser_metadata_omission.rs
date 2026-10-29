//! Actual isolated preparation uses real PCND2, not a descriptive loss token.
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba, resources::SharedBudget};
use paintcraft_document::{Document, POLICY, new_document};
use paintcraft_formats::{
    codec_worker::Owner,
    dds,
    export::metadata_policy::{Intent, OMITTED_REASON, Policy, Target},
    msp, pcnd,
    registry::{Family, Operation},
};
use std::cell::Cell;
fn document(attached: bool) -> Document {
    let doc = new_document(4, 4, Some(Rgba([255; 4])), 0, POLICY).unwrap();
    if !attached {
        return doc;
    }
    let budget = SharedBudget::new(16 << 10);
    let source = pcnd::read_with_metadata_admission_checked(
        include_bytes!("pcnd-metadata-fixtures/rgb-profile.pcnd"),
        32,
        131072,
        8,
        pcnd::MetadataAdmission {
            bytes: 16 << 10,
            budget: &budget,
        },
        &|| Ok(()),
    )
    .unwrap();
    doc.with_source_metadata_checked(source.source_metadata.clone(), &|| Ok(()))
        .unwrap()
}
fn capture_dds(doc: &Document, policy: Policy) -> Result<dds::web_export::SourcePlan> {
    dds::web_export::SourcePlan::capture_with_metadata_policy(
        doc,
        dds::TextureOptions {
            encoding: dds::Encoding::Rgba8,
            transfer: dds::Transfer::Srgb,
            alpha: dds::SourceAlpha::Straight,
            mips: dds::MipCount::Levels(1),
            filter: dds::MipFilter::Nearest,
            quality: dds::TextureQuality::Normal,
        },
        policy,
    )
}
fn capture_msp(doc: &Document, policy: Policy) -> Result<msp::web_export::SourcePlan> {
    msp::web_export::SourcePlan::capture_with_metadata_policy(doc, policy)
}
macro_rules! qualify {
    ($name:ident,$module:ident,$family:ident,$capture:ident) => {
        #[test]
        fn $name() {
            use paintcraft_formats::$module::web_export as service;
            for attached in [false, true] {
                let doc = document(attached);
                let original = pcnd::write(&doc).unwrap();
                let target = Target::new(Family::$family, Operation::ExportFlat);
                if attached {
                    assert_eq!(
                        $capture(&doc, Policy::default()).unwrap_err().code,
                        ErrorCode::CapabilityUnsupported
                    );
                }
                let policy = Policy::capture(&doc, target, Intent::Omit).unwrap();
                let plan = $capture(&doc, policy).unwrap();
                let source = plan.snapshot(&doc).unwrap();
                assert_eq!(
                    source[4],
                    if attached { 2 } else { 1 },
                    "Snapshot preserves real attachment"
                );
                let owner = Owner {
                    session: uuid::Uuid::new_v4(),
                    state: doc.state_id,
                    epoch: 7,
                    frontend: 3,
                };
                let header =
                    service::Header::capture(&plan, &source, owner, uuid::Uuid::new_v4()).unwrap();
                assert_eq!(header.metadata.omit_intent(), true);
                assert_eq!(header.metadata.omitted(), attached);
                let request = |h: &service::Header| {
                    let mut bytes = h.encode().to_vec();
                    bytes.extend_from_slice(&source);
                    bytes
                };
                let bytes = request(&header);
                let response =
                    service::execute(&bytes, env!("CARGO_PKG_VERSION"), &|| Ok(())).unwrap();
                let service::Frame::Success {
                    preview, losses, ..
                } = service::decode_response(&response, &header, owner, env!("CARGO_PKG_VERSION"))
                    .unwrap()
                else {
                    panic!("Expected actual copy")
                };
                assert_eq!(preview, vec![255; 64]);
                assert_eq!(
                    losses,
                    if attached {
                        vec![OMITTED_REASON]
                    } else {
                        vec![]
                    }
                );
                for at in [288, 304, 336, 337, 338, 339, 340, 344] {
                    let mut altered = response.clone();
                    altered[at] ^= 1;
                    assert_eq!(
                        service::decode_response(
                            &altered,
                            &header,
                            owner,
                            env!("CARGO_PKG_VERSION")
                        )
                        .unwrap_err()
                        .code,
                        ErrorCode::CodecFailed
                    );
                }
                let mut old = bytes.clone();
                old[7] = b'1';
                old[8..10].copy_from_slice(&1u16.to_be_bytes());
                assert_eq!(
                    service::execute(&old, env!("CARGO_PKG_VERSION"), &|| Ok(()))
                        .unwrap_err()
                        .code,
                    ErrorCode::InputSchema
                );
                let mut stale = doc.clone();
                stale.state_id = uuid::Uuid::new_v4();
                assert_eq!(
                    plan.snapshot(&stale).unwrap_err().code,
                    ErrorCode::ConflictStaleBase
                );
                if attached {
                    let mut changed = doc.clone();
                    changed.source_metadata = None;
                    assert_eq!(
                        plan.snapshot(&changed).unwrap_err().code,
                        ErrorCode::ConflictStaleBase
                    );
                    let mut bad = header.clone();
                    let mut context = bad.metadata.encode();
                    context[16] ^= 1;
                    bad.metadata =
                        paintcraft_formats::export::SourceMetadataContext::decode(&context)
                            .unwrap();
                    let refusal =
                        service::execute(&request(&bad), env!("CARGO_PKG_VERSION"), &|| Ok(()))
                            .unwrap();
                    assert!(matches!(
                        service::decode_response(&refusal, &bad, owner, env!("CARGO_PKG_VERSION"))
                            .unwrap(),
                        service::Frame::Refused(_)
                    ));
                    let mut bad = header.clone();
                    let mut context = bad.metadata.encode();
                    context[56..64]
                        .copy_from_slice(&u64::from(bad.metadata.wire_bytes()).to_be_bytes());
                    bad.metadata =
                        paintcraft_formats::export::SourceMetadataContext::decode(&context)
                            .unwrap();
                    let refusal =
                        service::execute(&request(&bad), env!("CARGO_PKG_VERSION"), &|| Ok(()))
                            .unwrap();
                    let service::Frame::Refused(error) =
                        service::decode_response(&refusal, &bad, owner, env!("CARGO_PKG_VERSION"))
                            .unwrap()
                    else {
                        panic!("Understatement must refuse")
                    };
                    assert_eq!(error.code, ErrorCode::ResourceLimit);
                    let mut wrong = header.clone();
                    let mut context = wrong.metadata.encode();
                    context[49] = 3 - context[49];
                    wrong.metadata =
                        paintcraft_formats::export::SourceMetadataContext::decode(&context)
                            .unwrap();
                    assert_eq!(
                        service::execute(&request(&wrong), env!("CARGO_PKG_VERSION"), &|| Ok(()))
                            .unwrap_err()
                            .code,
                        ErrorCode::InputSchema
                    );
                }
                let count = Cell::new(0);
                service::execute(&bytes, env!("CARGO_PKG_VERSION"), &|| {
                    count.set(count.get() + 1);
                    Ok(())
                })
                .unwrap();
                for sticky in [false, true] {
                for stop in 1..=count.get() {
                    let at = Cell::new(0);
                    let outcome = service::execute(&bytes, env!("CARGO_PKG_VERSION"), &|| {
                        at.set(at.get() + 1);
                        if at.get() == stop || (sticky && at.get() > stop) {
                            Err(Error::new(
                                ErrorCode::OperationCancelled,
                                "cancel exact metadata copy",
                            ))
                        } else {
                            Ok(())
                        }
                    });
                    match outcome {
                        Err(error) => assert_eq!(error.code, ErrorCode::OperationCancelled),
                        Ok(reply) => {
                            let service::Frame::Refused(error) = service::decode_response(
                                &reply,
                                &header,
                                owner,
                                env!("CARGO_PKG_VERSION"),
                            )
                            .unwrap() else {
                                panic!("Cancelled copy cannot carry output: stop={stop}, actual={}, successful={}, attached={attached}",at.get(),count.get())
                            };
                            assert_eq!(error.code, ErrorCode::OperationCancelled);
                        }
                    }
                }
                }
                assert_eq!(
                    pcnd::write(&doc).unwrap(),
                    original,
                    "Every failure leaves retained source facts and pixels unchanged"
                );
            }
        }
    };
}
qualify!(
    dds_pcnd2_policy_and_preallocation_are_bound,
    dds,
    Dds,
    capture_dds
);
qualify!(
    msp_pcnd2_policy_and_preallocation_are_bound,
    msp,
    Msp,
    capture_msp
);
