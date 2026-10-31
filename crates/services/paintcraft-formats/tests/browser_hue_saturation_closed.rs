//! Known HSL remains outside the frozen export worker profiles.
use paintcraft_core::types::{
    ErrorCode,
    filters::{BoxBlurV1, Descriptor, LevelsRgbV1},
    json,
};
use paintcraft_document::{Document, FilterStack};
use paintcraft_formats::{
    codec_worker::Owner,
    dds::{self, web_export as d},
    export::{CopyPolicies, filter_bake, metadata_policy},
    msp::web_export as m,
    pcnd,
    registry::{Family, Operation},
};
use uuid::Uuid;
fn policy(doc: &Document, family: Family) -> CopyPolicies {
    let target = metadata_policy::Target::new(family, Operation::ExportFlat);
    CopyPolicies {
        source_metadata: metadata_policy::Policy::capture(
            doc,
            target,
            metadata_policy::Intent::Refuse,
        )
        .unwrap(),
        filter_bake: filter_bake::Policy::capture(doc, target, filter_bake::Intent::Bake, &|| {
            Ok(())
        })
        .unwrap(),
    }
}
fn open(bytes: &[u8]) -> paintcraft_core::types::Result<paintcraft_document::Document> {
    let budget = paintcraft_core::types::resources::SharedBudget::new(4 << 20);
    pcnd::read_with_admission_checked(
        bytes,
        2,
        65_536,
        1,
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
}
#[test]
fn authenticated_worker6_refuses_reader_known_hue_saturation_including_disabled_and_default() {
    let oracle = json::parse(include_bytes!("pcnd-hsl-fixtures/oracle.json")).unwrap();
    let mut count = 0;
    for row in oracle
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row.get("accepted").unwrap().as_bool() == Some(true))
    {
        count += 1;
        let (pairs, tail) = row
            .get("hex")
            .unwrap()
            .as_str()
            .unwrap()
            .as_bytes()
            .as_chunks::<2>();
        assert!(tail.is_empty());
        let source: Vec<u8> = pairs
            .iter()
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect();
        // First prove this exact source is accepted by the current general reader.
        let mut base = open(&source).unwrap();
        let layer = base.layers.get_mut(&base.roots[0]).unwrap();
        let original = &layer.filters.as_slice()[0];
        layer.filters = FilterStack::new(vec![original.with_descriptor(Descriptor::LevelsRgbV1(
            LevelsRgbV1::new(0, 255, 100, 0, 255).unwrap(),
        ))])
        .unwrap();
        let owner = Owner {
            session: base.document_id,
            state: base.state_id,
            epoch: 3,
            frontend: 3,
        };
        let nonce = Uuid::from_u128(91);
        let options = dds::TextureOptions {
            encoding: dds::Encoding::Rgba8,
            transfer: dds::Transfer::Srgb,
            alpha: dds::SourceAlpha::Straight,
            mips: dds::MipCount::Levels(1),
            filter: dds::MipFilter::Nearest,
            quality: dds::TextureQuality::Normal,
        };
        // The exact PCND6 source is authenticated for each frozen old profile.
        // Different captured legacy control documents affect only admissible header facts.
        for old in [4, 5] {
            let mut legacy = base.clone();
            let descriptor = if old == 4 {
                Descriptor::InvertRgbV1
            } else {
                Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap())
            };
            legacy.layers.get_mut(&legacy.roots[0]).unwrap().filters = FilterStack::new(vec![
                base.layers[&base.roots[0]].filters.as_slice()[0]
                    .with_descriptor(descriptor)
                    .with_enabled(false),
            ])
            .unwrap();
            let dp = d::SourcePlan::capture_with_policies(
                &legacy,
                options,
                policy(&legacy, Family::Dds),
            )
            .unwrap();
            let mp = m::SourcePlan::capture_with_policies(&legacy, policy(&legacy, Family::Msp))
                .unwrap();
            if old == 4 {
                let h = d::Header::capture(&dp, &source, owner, nonce).unwrap();
                let mut p = h.encode().to_vec();
                p.extend_from_slice(&source);
                let r = d::execute(&p, "closed", &|| Ok(())).unwrap();
                match d::decode_response(&r, &h, owner, "closed").unwrap() {
                    d::Frame::Refused(e) => assert_eq!(e.code, ErrorCode::CapabilityUnsupported),
                    _ => panic!("Worker4 escaped"),
                }
                let h = m::Header::capture(&mp, &source, owner, nonce).unwrap();
                let mut p = h.encode().to_vec();
                p.extend_from_slice(&source);
                let r = m::execute(&p, "closed", &|| Ok(())).unwrap();
                match m::decode_response(&r, &h, owner, "closed").unwrap() {
                    m::Frame::Refused(e) => assert_eq!(e.code, ErrorCode::CapabilityUnsupported),
                    _ => panic!("Worker4 escaped"),
                }
            } else {
                let h = d::v5::Header::capture(&dp, &source, owner, nonce).unwrap();
                let mut p = h.encode().to_vec();
                p.extend_from_slice(&source);
                let r = d::v5::execute(&p, "closed", &|| Ok(())).unwrap();
                match d::v5::decode_response(&r, &h, owner, "closed").unwrap() {
                    d::v5::Frame::Refused(e) => {
                        assert_eq!(e.code, ErrorCode::InputSchema);
                        assert_eq!(e.message, "Export Worker5 requires PCND5 source");
                    }
                    _ => panic!("Worker5 escaped"),
                }
                let h = m::v5::Header::capture(&mp, &source, owner, nonce).unwrap();
                let mut p = h.encode().to_vec();
                p.extend_from_slice(&source);
                let r = m::v5::execute(&p, "closed", &|| Ok(())).unwrap();
                match m::v5::decode_response(&r, &h, owner, "closed").unwrap() {
                    m::v5::Frame::Refused(e) => {
                        assert_eq!(e.code, ErrorCode::InputSchema);
                        assert_eq!(e.message, "Export Worker5 requires PCND5 source");
                    }
                    _ => panic!("Worker5 escaped"),
                }
            }
        }
        let plan = d::SourcePlan::capture_with_policies(&base, options, policy(&base, Family::Dds))
            .unwrap();
        let header = d::v6::Header::capture(&plan, &source, owner, nonce).unwrap();
        let mut packet = header.encode().to_vec();
        packet.extend_from_slice(&source);
        let reply = d::v6::execute(&packet, "closed-profile", &|| Ok(())).unwrap();
        match d::v6::decode_response(&reply, &header, owner, "closed-profile").unwrap() {
            d::v6::Frame::Refused(error) => {
                assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
                assert_eq!(
                    error.message,
                    "Export Worker6 admits only initial eight profiles/params-v1"
                );
            }
            _ => panic!("Hue/Saturation escaped frozen DDS capability"),
        }
        let plan = m::SourcePlan::capture_with_policies(&base, policy(&base, Family::Msp)).unwrap();
        let header = m::v6::Header::capture(&plan, &source, owner, nonce).unwrap();
        let mut packet = header.encode().to_vec();
        packet.extend_from_slice(&source);
        let reply = m::v6::execute(&packet, "closed-profile", &|| Ok(())).unwrap();
        match m::v6::decode_response(&reply, &header, owner, "closed-profile").unwrap() {
            m::v6::Frame::Refused(error) => {
                assert_eq!(error.code, ErrorCode::CapabilityUnsupported);
                assert_eq!(
                    error.message,
                    "Export Worker6 admits only initial eight profiles/params-v1"
                );
            }
            _ => panic!("Hue/Saturation escaped frozen MSP capability"),
        }
    }
    assert_eq!(count, 13);
}
