//! Browser-specific DDS entry; it neither changes nor falls back to decoder2.
use paintcraft_core::types::Result;
use paintcraft_formats::dds::web_export;
pub fn execute(bytes: &[u8]) -> Result<Vec<u8>> {
    web_export::execute(bytes, env!("CARGO_PKG_VERSION"), &|| Ok(()))
}
#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_formats::{
        codec_worker::Owner,
        dds::{
            Encoding, MipCount, MipFilter, SourceAlpha, TextureOptions, TextureQuality, Transfer,
        },
    };
    #[test]
    fn actual_worker_entry_completes_linear_and_keeps_decoder2_separate() {
        let doc = paintcraft_document::new_document(
            1,
            1,
            Some(paintcraft_core::types::Rgba([128, 188, 255, 127])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        let options = TextureOptions {
            encoding: Encoding::Rgba8,
            transfer: Transfer::Linear,
            alpha: SourceAlpha::Straight,
            mips: MipCount::Levels(1),
            filter: MipFilter::Nearest,
            quality: TextureQuality::Normal,
        };
        let plan = web_export::SourcePlan::capture(&doc, options).unwrap();
        let source = plan.snapshot(&doc).unwrap();
        let owner = Owner {
            session: doc.document_id,
            state: doc.state_id,
            epoch: 0,
            frontend: 3,
        };
        let header = web_export::Header::capture(&plan, &source, owner, doc.document_id).unwrap();
        let mut request = header.encode().to_vec();
        request.extend(source);
        let response = execute(&request).unwrap();
        match web_export::decode_response(&response, &header, owner, env!("CARGO_PKG_VERSION"))
            .unwrap()
        {
            web_export::Frame::Success { preview, .. } => assert_eq!(preview, [55, 128, 255, 127]),
            web_export::Frame::Refused(error) => panic!("{error:?}"),
        }
        request[..8].copy_from_slice(b"PCDEC002");
        assert!(execute(&request).is_err());
    }
}

#[cfg(test)]
mod filter_tests {
    use super::*;
    use paintcraft_core::types::json;
    use paintcraft_formats::{
        codec_worker::Owner,
        export::{self, filter_bake, metadata_policy},
        pcnd,
    };
    #[test]
    fn actual_entry_qualifies_independent_enabled_disabled_and_metadata_pcnd3() {
        let root = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../crates/services/paintcraft-formats/tests/browser-filter-export-fixtures/"
        );
        let oracle = json::parse(&std::fs::read(format!("{root}oracle.json")).unwrap()).unwrap();
        for row in oracle.get("fixtures").unwrap().as_array().unwrap() {
            let source = std::fs::read(format!(
                "{root}{}",
                row.get("file").unwrap().as_str().unwrap()
            ))
            .unwrap();
            let doc = pcnd::read(&source).unwrap();
            let target = metadata_policy::Target::new(
                paintcraft_formats::registry::Family::Dds,
                paintcraft_formats::registry::Operation::ExportFlat,
            );
            let policies = export::CopyPolicies {
                source_metadata: metadata_policy::Policy::capture(
                    &doc,
                    target,
                    metadata_policy::Intent::Omit,
                )
                .unwrap(),
                filter_bake: filter_bake::Policy::capture(
                    &doc,
                    target,
                    filter_bake::Intent::Bake,
                    &|| Ok(()),
                )
                .unwrap(),
            };
            let options = paintcraft_formats::dds::TextureOptions {
                encoding: paintcraft_formats::dds::Encoding::Rgba8,
                transfer: paintcraft_formats::dds::Transfer::Srgb,
                alpha: paintcraft_formats::dds::SourceAlpha::Straight,
                mips: paintcraft_formats::dds::MipCount::Levels(1),
                filter: paintcraft_formats::dds::MipFilter::Nearest,
                quality: paintcraft_formats::dds::TextureQuality::Normal,
            };
            let plan =
                web_export::SourcePlan::capture_with_policies(&doc, options, policies).unwrap();
            assert_eq!(plan.snapshot(&doc).unwrap(), source);
            let inventory = plan
                .filter_digest()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(
                inventory,
                row.get("filter_inventory_sha256")
                    .unwrap()
                    .as_str()
                    .unwrap()
            );
            let owner = Owner {
                session: doc.document_id,
                state: doc.state_id,
                epoch: 0,
                frontend: 3,
            };
            let header =
                web_export::Header::capture(&plan, &source, owner, doc.document_id).unwrap();
            let mut request = header.encode().to_vec();
            request.extend_from_slice(&source);
            let response = execute(&request).unwrap();
            let web_export::Frame::Success {
                preview, losses, ..
            } = web_export::decode_response(&response, &header, owner, env!("CARGO_PKG_VERSION"))
                .unwrap()
            else {
                panic!("independent fixture refused")
            };
            let expected = row
                .get("expected_rgba")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_i64().unwrap() as u8)
                .collect::<Vec<_>>();
            assert_eq!(preview, expected);
            assert!(losses.contains(&filter_bake::BAKED_REASON));
            assert_eq!(
                losses.contains(&metadata_policy::OMITTED_REASON),
                doc.source_metadata.is_some()
            );
            let mut refused_header = header.clone();
            refused_header.bake_intent = 0;
            let mut refused = refused_header.encode().to_vec();
            refused.extend_from_slice(&source);
            let response = execute(&refused).unwrap();
            let web_export::Frame::Refused(error) = web_export::decode_response(
                &response,
                &refused_header,
                owner,
                env!("CARGO_PKG_VERSION"),
            )
            .unwrap() else {
                panic!("metadata omission granted bake")
            };
            assert_eq!(
                error.code,
                paintcraft_core::types::ErrorCode::CapabilityUnsupported
            );
            assert_eq!(pcnd::write(&doc).unwrap(), source);
        }
    }
}
