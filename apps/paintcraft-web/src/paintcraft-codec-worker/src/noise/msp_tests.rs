//! Authored actual thin endpoints; source-only until combined graph qualification.
use paintcraft_core::types::{
    ErrorCode,
    filters::{AddNoiseRgbV1, Descriptor, NoiseDistribution},
};
use paintcraft_document::{Document, FilterInstance, FilterStack};
use paintcraft_formats::{
    export,
    msp::{
        self,
        web_export::{self as legacy, v8},
    },
};
fn document(mode: NoiseDistribution, amount: i16, enabled: bool, pixel: [u8; 4]) -> Document {
    let mut d = paintcraft_document::import_image(1, 1, &pixel, &"13".repeat(32)).unwrap();
    d.layers.get_mut(&d.roots[0]).unwrap().filters = FilterStack::new(vec![FilterInstance::new(
        uuid::Uuid::from_u128(531),
        enabled,
        Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::new(amount, 1, true, mode).unwrap()),
    )])
    .unwrap();
    d
}
fn request(d: &Document, version: msp::Version) -> (v8::Header, Vec<u8>) {
    let target = export::metadata_policy::Target::new(
        paintcraft_formats::registry::Family::Msp,
        paintcraft_formats::registry::Operation::ExportFlat,
    );
    let bake =
        export::filter_bake::Policy::capture(d, target, export::filter_bake::Intent::Bake, &|| {
            Ok(())
        })
        .unwrap();
    let p = v8::NoiseSourcePlan::capture_with_policies_checked(
        d,
        version,
        export::CopyPolicies {
            source_metadata: Default::default(),
            filter_bake: bake,
        },
        v8::default_workload().unwrap(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(p.version(), version);
    let raw = p.snapshot(d).unwrap();
    let owner = paintcraft_formats::codec_worker::Owner {
        session: d.document_id,
        state: d.state_id,
        epoch: 3,
        frontend: 3,
    };
    let h = v8::Header::capture(&p, &raw, owner, d.document_id).unwrap();
    let mut bytes = h.encode().unwrap().to_vec();
    bytes.extend(raw);
    (h, bytes)
}
#[test]
fn actual_msp1_and_msp2_noise_entry_executes_both_modes_with_exact_bw_and_echo() {
    for version in [msp::Version::UncompressedV1, msp::Version::RowRleV2] {
        for mode in [NoiseDistribution::Uniform, NoiseDistribution::Gaussian] {
            // Frozen independent coordinate0/seed1 mono draws are positive. White clamps
            // exactly255 under both modes; no threshold/alpha/color conversion is allowed.
            let d = document(mode, 10, true, [255; 4]);
            let before = paintcraft_formats::pcnd::write(&d).unwrap();
            let (h, bytes) = request(&d, version);
            assert_eq!(h.version, version);
            assert!(h.programs.work().noise > 0);
            let response = super::msp(&bytes).unwrap();
            let v8::Frame::Success {
                encoded, preview, ..
            } = v8::decode_response(&response, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap()
            else {
                panic!("real MSP8 success")
            };
            assert_eq!(preview, [255; 4]);
            assert_eq!(
                msp::inspect(encoded, &|| Ok(())).unwrap().version(),
                version
            );
            assert_eq!(paintcraft_formats::pcnd::write(&d).unwrap(), before);
        }
    }
}
#[test]
fn actual_msp8_refuses_non_bw_and_nonopaque_without_thresholding_or_matte() {
    for version in [msp::Version::UncompressedV1, msp::Version::RowRleV2] {
        for pixel in [[64, 64, 64, 255], [255, 255, 255, 127]] {
            let d = document(NoiseDistribution::Gaussian, 0, true, pixel);
            let (h, bytes) = request(&d, version);
            let response = super::msp(&bytes).unwrap();
            let v8::Frame::Refused(e) =
                v8::decode_response(&response, &h, h.owner, env!("CARGO_PKG_VERSION")).unwrap()
            else {
                panic!("must not manufacture BW")
            };
            assert_eq!(e.code, ErrorCode::CapabilityUnsupported);
        }
    }
}
#[test]
fn stored_disabled_identity_noise_cannot_upgrade_legacy_msp_and_v8_framing_is_exact() {
    for version in [msp::Version::UncompressedV1, msp::Version::RowRleV2] {
        let d = document(NoiseDistribution::Uniform, 0, false, [255; 4]);
        assert_eq!(
            legacy::SourcePlan::capture(&d).unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        let (h, bytes) = request(&d, version);
        assert_eq!(&bytes[..8], b"PCMSPQ08");
        assert_eq!(
            bytes[113],
            if version == msp::Version::UncompressedV1 {
                1
            } else {
                2
            }
        );
        assert_eq!(bytes[121], h.source_flags);
        assert_eq!(&bytes[114..121], &[0; 7]);
        assert!(legacy::v7::Header::decode(&bytes[..v8::REQUEST_HEADER]).is_err());
    }
}
