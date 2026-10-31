use paintcraft_core::types::filters::{Descriptor, ExposureSrgbV1};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_formats::{
    codec_worker::Owner,
    dds::{self, web_export as d},
    export::{CopyPolicies, filter_bake, metadata_policy},
    msp::web_export as m,
    registry::{Family, Operation},
};
use uuid::Uuid;
fn source(
    rgb: [u8; 3],
    stops: i16,
    family: Family,
) -> (paintcraft_document::Document, CopyPolicies, Owner) {
    let mut doc = paintcraft_document::import_image(
        1,
        1,
        &[rgb[0], rgb[1], rgb[2], 255],
        "actual prepared entry",
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(31),
            true,
            Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(stops).unwrap()),
        )])
        .unwrap();
    let target = metadata_policy::Target::new(family, Operation::ExportFlat);
    let policies = CopyPolicies {
        source_metadata: metadata_policy::Policy::default(),
        filter_bake: filter_bake::Policy::capture(&doc, target, filter_bake::Intent::Bake, &|| {
            Ok(())
        })
        .unwrap(),
    };
    let owner = Owner {
        session: doc.document_id,
        state: doc.state_id,
        epoch: 3,
        frontend: 3,
    };
    (doc, policies, owner)
}
#[test]
fn actual_prepared_dds_entry_returns_causal_linear_light_literal() {
    let (doc, policies, owner) = source([64, 16, 128], 100, Family::Dds);
    let options = dds::TextureOptions {
        encoding: dds::Encoding::Rgba8,
        transfer: dds::Transfer::Srgb,
        alpha: dds::SourceAlpha::Straight,
        mips: dds::MipCount::Levels(1),
        filter: dds::MipFilter::Nearest,
        quality: dds::TextureQuality::Normal,
    };
    let plan = d::SourcePlan::capture_with_policies(&doc, options, policies).unwrap();
    let bytes = plan.snapshot(&doc).unwrap();
    let header = d::v7::Header::capture(&plan, &bytes, owner, doc.document_id).unwrap();
    let mut packet = header.encode().to_vec();
    packet.extend(bytes);
    let response = super::dds(&packet).unwrap();
    match d::v7::decode_response(&response, &header, owner, env!("CARGO_PKG_VERSION")).unwrap() {
        d::v7::Frame::Success { preview, .. } => assert_eq!(preview, [90, 26, 176, 255]),
        _ => panic!("actual prepared DDS refused"),
    }
}
#[test]
fn actual_prepared_msp_entry_requires_evaluated_black_not_input_quantization() {
    let (doc, policies, owner) = source([1; 3], -800, Family::Msp);
    let plan = m::SourcePlan::capture_with_policies(&doc, policies).unwrap();
    let bytes = plan.snapshot(&doc).unwrap();
    let header = m::v7::Header::capture(&plan, &bytes, owner, doc.document_id).unwrap();
    let mut packet = header.encode().to_vec();
    packet.extend(bytes);
    let response = super::msp(&packet).unwrap();
    match m::v7::decode_response(&response, &header, owner, env!("CARGO_PKG_VERSION")).unwrap() {
        m::v7::Frame::Success { preview, .. } => assert_eq!(preview, [0, 0, 0, 255]),
        _ => panic!("actual prepared MSP refused"),
    }
}
