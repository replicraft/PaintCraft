//! Actual protocol4 Rust Worker entrypoints with independently authored PCND4.
use paintcraft_core::types::{
    ErrorCode, Result,
    json::{self, Value},
    resources::SharedBudget,
};
use paintcraft_document::Document;
use paintcraft_formats::{
    codec_worker::Owner,
    dds, export, msp, pcnd,
    registry::{Family, Operation},
};
use std::path::Path;
use uuid::Uuid;
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/browser-mask-export-fixtures")
            .join(name),
    )
    .unwrap()
}
fn document(bytes: &[u8]) -> Document {
    let budget = SharedBudget::new(1 << 20);
    pcnd::read_with_admission_checked(
        bytes,
        2,
        65536,
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
    .unwrap()
}
fn policies(doc: &Document, family: Family) -> export::CopyPolicies {
    let target = export::metadata_policy::Target::new(family, Operation::ExportFlat);
    export::CopyPolicies {
        source_metadata: export::metadata_policy::Policy::capture(
            doc,
            target,
            if doc.source_metadata.is_some() {
                export::metadata_policy::Intent::Omit
            } else {
                export::metadata_policy::Intent::Refuse
            },
        )
        .unwrap(),
        filter_bake: export::filter_bake::Policy::capture(
            doc,
            target,
            export::filter_bake::Intent::Bake,
            &|| Ok(()),
        )
        .unwrap(),
    }
}
enum Wire {
    Dds(dds::web_export::Header, Vec<u8>),
    Msp(msp::web_export::Header, Vec<u8>),
}
impl Wire {
    fn capture(doc: &Document, family: Family) -> Self {
        let owner = Owner {
            session: Uuid::from_u128(7),
            state: doc.state_id,
            epoch: 4,
            frontend: 3,
        };
        let nonce = Uuid::from_u128(8);
        let policies = policies(doc, family);
        if family == Family::Dds {
            let options = dds::TextureOptions {
                encoding: dds::Encoding::Rgba8,
                transfer: dds::Transfer::Srgb,
                alpha: dds::SourceAlpha::Straight,
                mips: dds::MipCount::Levels(1),
                filter: dds::MipFilter::Nearest,
                quality: dds::TextureQuality::Normal,
            };
            assert_eq!(
                dds::web_export::SourcePlan::capture(doc, options)
                    .unwrap_err()
                    .code,
                ErrorCode::CapabilityUnsupported
            );
            let plan =
                dds::web_export::SourcePlan::capture_with_policies(doc, options, policies).unwrap();
            let source = plan.snapshot(doc).unwrap();
            Self::Dds(
                dds::web_export::Header::capture(&plan, &source, owner, nonce).unwrap(),
                source,
            )
        } else {
            assert_eq!(
                msp::web_export::SourcePlan::capture(doc).unwrap_err().code,
                ErrorCode::CapabilityUnsupported
            );
            let plan = msp::web_export::SourcePlan::capture_with_policies(doc, policies).unwrap();
            let source = plan.snapshot(doc).unwrap();
            Self::Msp(
                msp::web_export::Header::capture(&plan, &source, owner, nonce).unwrap(),
                source,
            )
        }
    }
    fn request(&self) -> Vec<u8> {
        match self {
            Self::Dds(header, source) => {
                let mut b = header.encode().to_vec();
                b.extend(source);
                b
            }
            Self::Msp(header, source) => {
                let mut b = header.encode().to_vec();
                b.extend(source);
                b
            }
        }
    }
    fn execute(&self, request: &[u8]) -> Result<Vec<u8>> {
        match self {
            Self::Dds(..) => dds::web_export::execute(request, "test", &|| Ok(())),
            Self::Msp(..) => msp::web_export::execute(request, "test", &|| Ok(())),
        }
    }
    fn outcome(
        &self,
        request: &[u8],
        response: &[u8],
        revoke: bool,
    ) -> std::result::Result<(Vec<u8>, Vec<&'static str>), ErrorCode> {
        match self {
            Self::Dds(..) => {
                let header = dds::web_export::Header::decode(&request[..256]).unwrap();
                let mut owner = header.owner;
                if revoke {
                    owner.epoch += 1;
                }
                match dds::web_export::decode_response(response, &header, owner, "test")
                    .map_err(|e| e.code)?
                {
                    dds::web_export::Frame::Success {
                        preview, losses, ..
                    } => Ok((preview.to_vec(), losses)),
                    dds::web_export::Frame::Refused(error) => Err(error.code),
                }
            }
            Self::Msp(..) => {
                let header = msp::web_export::Header::decode(&request[..256]).unwrap();
                let mut owner = header.owner;
                if revoke {
                    owner.epoch += 1;
                }
                match msp::web_export::decode_response(response, &header, owner, "test")
                    .map_err(|e| e.code)?
                {
                    msp::web_export::Frame::Success {
                        preview, losses, ..
                    } => Ok((preview.to_vec(), losses)),
                    msp::web_export::Frame::Refused(error) => Err(error.code),
                }
            }
        }
    }
}
fn rows() -> Vec<Value> {
    json::parse(&fixture("oracle.json"))
        .unwrap()
        .get("fixtures")
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec()
}
#[test]
fn protocol4_entries_preserve_independent_masked_pixels_and_separate_copy_authorities() {
    for row in rows() {
        let name = row.get("file").unwrap().as_str().unwrap();
        let bytes = fixture(name);
        let doc = document(&bytes);
        assert_eq!(
            paintcraft_document::hex(
                &export::filter_bake::inspect(&doc, &|| Ok(()))
                    .unwrap()
                    .digest()
            ),
            row.get("filter_inventory_sha256")
                .unwrap()
                .as_str()
                .unwrap(),
            "{name}"
        );
        for family in [Family::Dds, Family::Msp] {
            let wire = Wire::capture(&doc, family);
            let request = wire.request();
            let response = wire.execute(&request).unwrap();
            let outcome = wire.outcome(&request, &response, false);
            if family == Family::Msp && !row.get("msp_supported").unwrap().as_bool().unwrap() {
                assert_eq!(outcome.unwrap_err(), ErrorCode::CapabilityUnsupported);
            } else {
                let (preview, losses) = outcome.unwrap();
                let expected: Vec<_> = row
                    .get("expected_rgba")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_i64().unwrap() as u8)
                    .collect();
                assert_eq!(preview, expected, "{family:?} {name}");
                assert!(losses.contains(&export::filter_bake::BAKED_REASON));
                assert_eq!(
                    losses.contains(&export::metadata_policy::OMITTED_REASON),
                    row.get("metadata").unwrap().as_bool().unwrap()
                );
            }
            assert_eq!(
                wire.outcome(&request, &response, true).unwrap_err(),
                ErrorCode::ConflictStaleBase
            );
            let mut unchecked = request.clone();
            unchecked[122] = 0;
            let refused = wire.execute(&unchecked).unwrap();
            assert_eq!(
                wire.outcome(&unchecked, &refused, false).unwrap_err(),
                ErrorCode::CapabilityUnsupported,
                "metadata Omit cannot grant Bake"
            );
            assert_eq!(&response[384..416], &request[224..256]);
        }
        assert_eq!(
            pcnd::write_compact(&doc).unwrap(),
            bytes,
            "source must remain exact: {name}"
        );
    }
}
#[test]
fn protocol4_understated_decode_admission_and_old3_future5_refuse() {
    let bytes = fixture("fractional.pcnd");
    let doc = document(&bytes);
    for family in [Family::Dds, Family::Msp] {
        let wire = Wire::capture(&doc, family);
        let mut request = wire.request();
        request[248..256].copy_from_slice(&1u64.to_le_bytes());
        let response = wire.execute(&request).unwrap();
        assert_eq!(
            wire.outcome(&request, &response, false).unwrap_err(),
            ErrorCode::ResourceLimit
        );
        for version in [3, 5] {
            let mut old = request.clone();
            old[7] = b'0' + version;
            old[8..10].copy_from_slice(&u16::from(version).to_be_bytes());
            assert!(wire.execute(&old).is_err());
        }
        let mut old = request.clone();
        old[224..256].fill(0);
        assert!(
            wire.execute(&old).is_err(),
            "stored-mask flag needs matching facts"
        );
    }
}

#[path = "browser_mask_export/version_guards.rs"]
mod version_guards;
