use super::*;
use sha2::{Digest, Sha256};

#[test]
fn authenticated_worker4_sources_are_closed_before_the_general_reader() {
    let doc = document(&fixture("constant.pcnd"));
    for family in [Family::Dds, Family::Msp] {
        let wire = Wire::capture(&doc, family);
        let original = wire.request();
        let success = wire.execute(&original).unwrap();
        assert!(wire.outcome(&original, &success, false).is_ok());
        for version in [0, 5, 6, 255] {
            let mut request = original.clone();
            request[256 + 4] = version;
            let end = request.len() - 32;
            let outer = Sha256::digest(&request[256..end]);
            request[end..].copy_from_slice(&outer);
            let authenticated = Sha256::digest(&request[256..]);
            request[81..113].copy_from_slice(&authenticated);
            let response = wire.execute(&request).unwrap();
            let error = match family {
                Family::Dds => {
                    let header = dds::web_export::Header::decode(&request[..256]).unwrap();
                    match dds::web_export::decode_response(&response, &header, header.owner, "test")
                        .unwrap()
                    {
                        dds::web_export::Frame::Refused(error) => error,
                        _ => panic!("future source was exported"),
                    }
                }
                Family::Msp => {
                    let header = msp::web_export::Header::decode(&request[..256]).unwrap();
                    match msp::web_export::decode_response(&response, &header, header.owner, "test")
                        .unwrap()
                    {
                        msp::web_export::Frame::Refused(error) => error,
                        _ => panic!("future source was exported"),
                    }
                }
                _ => unreachable!(),
            };
            assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
            assert_eq!(
                error.message,
                if version == 5 {
                    "Export Worker4 does not admit PCND5 spatial source"
                } else {
                    "Export Worker4 source profile admits only PCND1 through PCND4"
                }
            );
            // Without updating the request hash this cannot reach the version guard.
            request[256 + 4] ^= 1;
            let response = wire.execute(&request).unwrap();
            assert_eq!(
                wire.outcome(&request, &response, false),
                Err(ErrorCode::CodecFailed)
            );
        }
    }
}

#[test]
fn worker4_keeps_all_four_supported_source_versions() {
    let v1 = paintcraft_document::import_image(2, 1, &[0, 0, 0, 255, 255, 255, 255, 255], "legacy")
        .unwrap();
    let mut v2 = v1.clone();
    v2.source_metadata = pcnd::read(include_bytes!(
        "../pcnd-metadata-fixtures/resolution-only.pcnd"
    ))
    .unwrap()
    .source_metadata;
    let v3 = pcnd::read(include_bytes!(
        "../browser-filter-export-fixtures/invert.pcnd"
    ))
    .unwrap();
    let v4 = document(&fixture("constant.pcnd"));
    for (index, doc) in [v1, v2, v3, v4].iter().enumerate() {
        let source = pcnd::write_compact(doc).unwrap();
        assert_eq!(usize::from(source[4]), index + 1);
        for family in [Family::Dds, Family::Msp] {
            let owner = Owner {
                session: Uuid::from_u128(7),
                state: doc.state_id,
                epoch: 4,
                frontend: 3,
            };
            let nonce = Uuid::from_u128(8);
            let policies = policies(doc, family);
            let wire = if family == Family::Dds {
                let options = dds::TextureOptions {
                    encoding: dds::Encoding::Rgba8,
                    transfer: dds::Transfer::Srgb,
                    alpha: dds::SourceAlpha::Straight,
                    mips: dds::MipCount::Levels(1),
                    filter: dds::MipFilter::Nearest,
                    quality: dds::TextureQuality::Normal,
                };
                let plan =
                    dds::web_export::SourcePlan::capture_with_policies(doc, options, policies)
                        .unwrap();
                Wire::Dds(
                    dds::web_export::Header::capture(&plan, &source, owner, nonce).unwrap(),
                    source.clone(),
                )
            } else {
                let plan =
                    msp::web_export::SourcePlan::capture_with_policies(doc, policies).unwrap();
                Wire::Msp(
                    msp::web_export::Header::capture(&plan, &source, owner, nonce).unwrap(),
                    source.clone(),
                )
            };
            let request = wire.request();
            let response = wire.execute(&request).unwrap();
            assert!(
                wire.outcome(&request, &response, false).is_ok(),
                "PCND{} {family:?}",
                index + 1
            );
        }
    }
}
