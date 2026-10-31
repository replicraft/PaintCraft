//! Separate actual codec Worker5 entries; legacy4 entry points stay closed.
use paintcraft_core::types::Result;
use paintcraft_formats::{dds::web_export as dds_wire, msp::web_export as msp_wire};
pub fn dds(bytes: &[u8]) -> Result<Vec<u8>> {
    dds_wire::v5::execute(bytes, env!("CARGO_PKG_VERSION"), &|| Ok(()))
}
pub fn msp(bytes: &[u8]) -> Result<Vec<u8>> {
    msp_wire::v5::execute(bytes, env!("CARGO_PKG_VERSION"), &|| Ok(()))
}
#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::{
        Rgba,
        filters::{BoxBlurV1, Descriptor},
    };
    use paintcraft_document::{FilterInstance, FilterStack, POLICY};
    use paintcraft_formats::{
        codec_worker::Owner,
        export::{self, filter_bake, metadata_policy},
        registry,
    };
    fn source(enabled: bool, radius: i16) -> paintcraft_document::Document {
        let mut doc =
            paintcraft_document::new_document(2, 1, Some(Rgba([255; 4])), 4, POLICY).unwrap();
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                doc.document_id,
                enabled,
                Descriptor::BoxBlurV1(BoxBlurV1::new(radius).unwrap()),
            )])
            .unwrap();
        doc
    }
    fn policies(
        doc: &paintcraft_document::Document,
        family: registry::Family,
    ) -> export::CopyPolicies {
        let target = metadata_policy::Target::new(family, registry::Operation::ExportFlat);
        export::CopyPolicies {
            source_metadata: metadata_policy::Policy::default(),
            filter_bake: filter_bake::Policy::capture(
                doc,
                target,
                filter_bake::Intent::Bake,
                &|| Ok(()),
            )
            .unwrap(),
        }
    }
    #[test]
    fn actual_dds_spatial_entry_preserves_disabled_and_positive_radius_white_literals() {
        for (enabled, radius) in [(false, 32), (true, 0), (true, 1)] {
            let doc = source(enabled, radius);
            let plan = dds_wire::SourcePlan::capture_with_policies(
                &doc,
                paintcraft_formats::dds::TextureOptions {
                    encoding: paintcraft_formats::dds::Encoding::Rgba8,
                    transfer: paintcraft_formats::dds::Transfer::Srgb,
                    alpha: paintcraft_formats::dds::SourceAlpha::Straight,
                    mips: paintcraft_formats::dds::MipCount::Levels(1),
                    filter: paintcraft_formats::dds::MipFilter::Nearest,
                    quality: paintcraft_formats::dds::TextureQuality::Normal,
                },
                policies(&doc, registry::Family::Dds),
            )
            .unwrap();
            let source = plan.snapshot(&doc).unwrap();
            let owner = Owner {
                session: doc.document_id,
                state: doc.state_id,
                epoch: 0,
                frontend: 3,
            };
            let header =
                dds_wire::v5::Header::capture(&plan, &source, owner, doc.document_id).unwrap();
            let mut request = header.encode().to_vec();
            request.extend(source);
            let response = dds(&request).unwrap();
            let dds_wire::v5::Frame::Success { preview, .. } =
                dds_wire::v5::decode_response(&response, &header, owner, env!("CARGO_PKG_VERSION"))
                    .unwrap()
            else {
                panic!("spatial entry refused")
            };
            assert_eq!(preview, [255; 8]);
            request[7] = b'4';
            assert!(dds(&request).is_err());
        }
    }
    #[test]
    fn actual_msp_spatial_entry_preserves_disabled_and_positive_radius_white_literals() {
        for (enabled, radius) in [(false, 32), (true, 0), (true, 1)] {
            let doc = source(enabled, radius);
            let plan = msp_wire::SourcePlan::capture_with_policies(
                &doc,
                policies(&doc, registry::Family::Msp),
            )
            .unwrap();
            let source = plan.snapshot(&doc).unwrap();
            let owner = Owner {
                session: doc.document_id,
                state: doc.state_id,
                epoch: 0,
                frontend: 3,
            };
            let header =
                msp_wire::v5::Header::capture(&plan, &source, owner, doc.document_id).unwrap();
            let mut request = header.encode().to_vec();
            request.extend(source);
            let response = msp(&request).unwrap();
            let msp_wire::v5::Frame::Success { preview, .. } =
                msp_wire::v5::decode_response(&response, &header, owner, env!("CARGO_PKG_VERSION"))
                    .unwrap()
            else {
                panic!("spatial entry refused")
            };
            assert_eq!(preview, [255; 8]);
            request[7] = b'4';
            assert!(msp(&request).is_err());
        }
    }
}
