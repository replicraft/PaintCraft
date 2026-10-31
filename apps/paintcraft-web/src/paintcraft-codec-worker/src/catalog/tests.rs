use paintcraft_core::types::{
    Rgba,
    filters::{Descriptor, LevelsRgbV1},
};
use paintcraft_document::{FilterInstance, FilterStack};
use paintcraft_formats::{
    codec_worker::Owner,
    dds::{self, web_export as d},
    export::{self, filter_bake, metadata_policy},
    msp::web_export as m,
    registry::{Family, Operation},
};
fn source() -> paintcraft_document::Document {
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            doc.document_id,
            true,
            Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, 200, 0, 255).unwrap()),
        )])
        .unwrap();
    doc
}
fn policies(doc: &paintcraft_document::Document, family: Family) -> export::CopyPolicies {
    let target = metadata_policy::Target::new(family, Operation::ExportFlat);
    export::CopyPolicies {
        source_metadata: Default::default(),
        filter_bake: filter_bake::Policy::capture(doc, target, filter_bake::Intent::Bake, &|| {
            Ok(())
        })
        .unwrap(),
    }
}
fn owner(doc: &paintcraft_document::Document) -> Owner {
    Owner {
        session: doc.document_id,
        state: doc.state_id,
        epoch: 0,
        frontend: 3,
    }
}
#[test]
fn actual_dds_catalog_entry_eagerly_prepares_exact_white_gamma_endpoint() {
    let doc = source();
    let options = dds::TextureOptions {
        encoding: dds::Encoding::Rgba8,
        transfer: dds::Transfer::Srgb,
        alpha: dds::SourceAlpha::Straight,
        mips: dds::MipCount::Levels(1),
        filter: dds::MipFilter::Nearest,
        quality: dds::TextureQuality::Normal,
    };
    let plan =
        d::SourcePlan::capture_with_policies(&doc, options, policies(&doc, Family::Dds)).unwrap();
    let source = plan.snapshot(&doc).unwrap();
    let header = d::v6::Header::capture(&plan, &source, owner(&doc), doc.document_id).unwrap();
    let mut request = header.encode().to_vec();
    request.extend(source);
    let response = super::dds(&request).unwrap();
    let d::v6::Frame::Success {
        encoded, preview, ..
    } = d::v6::decode_response(&response, &header, header.owner, env!("CARGO_PKG_VERSION"))
        .unwrap()
    else {
        panic!("actual catalog entry refused")
    };
    assert_eq!(preview, [255; 8]);
    assert_eq!(
        dds::decode_top_mip_checked(encoded, 2, &|| Ok(()))
            .unwrap()
            .rgba,
        [255; 8]
    );
}
#[test]
fn actual_msp_catalog_entry_eagerly_prepares_exact_white_gamma_endpoint() {
    let doc = source();
    let plan = m::SourcePlan::capture_with_policies(&doc, policies(&doc, Family::Msp)).unwrap();
    let source = plan.snapshot(&doc).unwrap();
    let header = m::v6::Header::capture(&plan, &source, owner(&doc), doc.document_id).unwrap();
    let mut request = header.encode().to_vec();
    request.extend(source);
    let response = super::msp(&request).unwrap();
    let m::v6::Frame::Success {
        encoded, preview, ..
    } = m::v6::decode_response(&response, &header, header.owner, env!("CARGO_PKG_VERSION"))
        .unwrap()
    else {
        panic!("actual catalog entry refused")
    };
    assert_eq!(preview, [255; 8]);
    assert_eq!(
        paintcraft_formats::msp::decode(encoded, &|| Ok(()))
            .unwrap()
            .rgba(&|| Ok(()))
            .unwrap(),
        [255; 8]
    );
}
